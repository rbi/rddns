use futures::future::Future;
use hyper;
use hyper::header::{HeaderMap, AUTHORIZATION, WWW_AUTHENTICATE};
use hyper::service::{make_service_fn, service_fn};
use hyper::StatusCode;
use hyper::{Body, Request, Response};
use regex::Regex;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::net::AddrParseError;

use crate::basic_auth_header::BasicAuth;
use crate::config::TriggerHttp;
use crate::updater::UpdateResults;

pub async fn create_server<Fut>(
    update_callback: impl Fn(HashMap<String, String>) -> Fut + Send + Sync + Clone + 'static,
    server_config: TriggerHttp,
) -> Result<(), String>
where
    Fut: Future<Output = UpdateResults> + Send + 'static,
{
    let port = server_config.port;
    match format!("[::]:{}", port)
        .parse()
        .map_err(|err: AddrParseError| err.to_string())
    {
        Ok(addr) => {
            let service_creator = make_service_fn(move |_| {
                let server_config = server_config.clone();
                let update_callback = update_callback.clone();
                async move {
                    Ok::<_, hyper::Error>(service_fn(move |req| {
                        call(req, update_callback.clone(), server_config.clone())
                    }))
                }
            });

            info!("Listening on port {}", port);
            hyper::Server::bind(&addr)
                .serve(service_creator)
                .await
                .map_err(|err| err.to_string())
        }
        Err(_) => Err("Failed to parse address.".to_owned()),
    }
}

async fn call<Fut>(
    req: Request<Body>,
    update_callback: impl Fn(HashMap<String, String>) -> Fut,
    server_config: TriggerHttp,
) -> Result<Response<Body>, hyper::http::Error>
where
    Fut: Future<Output = UpdateResults>,
{
    info!("Received request: {}", req.uri());
    let authorized = is_authorized(req.headers(), &server_config);
    if !authorized {
        warn!("Request is not authorized.");
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(WWW_AUTHENTICATE, "Basic realm=\"rddns\"")
            .body(Body::empty());
    }

    let ip_parameters = extract_address_parameters(&req.uri().query());
    let update_result = (update_callback)(ip_parameters).await;

    let return_code = match update_result.errors {
        Some(_) => StatusCode::INTERNAL_SERVER_ERROR,
        None => StatusCode::OK,
    };

    let mut message_parts = Vec::with_capacity(2);
    if let Some(err) = update_result.errors {
        message_parts.push(err);
    }
    if let Some(warn) = update_result.warnings {
        message_parts.push(warn);
    }
    let message = match message_parts.len() {
        0 => "success".to_string(),
        _ => message_parts.join("\n"),
    };

    Response::builder()
        .status(return_code)
        .body(Body::from(message))
}

fn is_authorized(headers: &HeaderMap, config: &TriggerHttp) -> bool {
    match config.username {
        Some(ref username) => headers
            .get(AUTHORIZATION)
            .ok_or(())
            .and_then(|value| value.to_str().map_err(|_| ()))
            .and_then(|auth| {
                BasicAuth::try_from(auth).map_err(|err| {
                    debug!("{}", err);
                    ()
                })
            })
            .map(|auth| {
                auth.username.eq(username)
                    && match config.password {
                        Some(ref config_password) => match auth.password {
                            Some(ref auth_password) => config_password.eq(auth_password),
                            None => false,
                        },
                        None => true,
                    }
            })
            .unwrap_or(false),
        None => true,
    }
}

fn extract_address_parameters(query: &Option<&str>) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    let iter = query.map(|q| q.split("&"));
    match iter {
        Some(params) => {
            for param in params {
                let address_param = to_address_param(param);
                if let Some((key, value)) = address_param {
                    map.insert(key, value);
                }
            }
        }
        _ => (),
    }
    map
}

fn to_address_param(param: &str) -> Option<(String, String)> {
    lazy_static! {
        static ref IP_PARAM: Regex = Regex::new(r"ip\[([^\]]+)]=(.+)").unwrap();
    }

    IP_PARAM
        .captures(param)
        .map(|groups| (groups[1].to_string(), groups[2].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_address_parameters_correctly() {
        let mut expected = HashMap::new();
        expected.insert("first".to_string(), "2001:DB8:123:abcd::1".to_string());
        expected.insert("other".to_string(), "203.0.113.85".to_string());
        expected.insert("b64encoded".to_string(), "MTEuMjIuMzMuNDQ=".to_string());

        let query = Some(
            "ip[first]=2001:DB8:123:abcd::1&abitrary_param=abc&ip[other]=203.0.113.85\
&broken_param&ip[=broken&ip=broken_too&ip[b64encoded]=MTEuMjIuMzMuNDQ=",
        );
        let actual = extract_address_parameters(&query);

        assert_eq!(actual, expected);
    }

    #[test]
    fn extract_address_parameters_not_failing_when_empty_query() {
        let actual = extract_address_parameters(&None);

        assert!(actual.is_empty());
    }

    #[test]
    fn authorized_when_no_credentials_are_required() {
        let conf = TriggerHttp {
            username: None,
            password: None,
            port: 518,
        };

        let mut headers_with_auth = HeaderMap::new();
        // some_user:some_password
        headers_with_auth.append(
            AUTHORIZATION,
            "Basic c29tZV91c2VyOnNvbWVfcGFzc3dvcmQ=".parse().unwrap(),
        );
        assert!(is_authorized(&headers_with_auth, &conf));

        let headers_without_auth = HeaderMap::new();
        assert!(is_authorized(&headers_without_auth, &conf));
    }

    #[test]
    fn authorized_when_correct_credentials_are_passed() {
        let conf = TriggerHttp {
            username: Some("some_user".to_string()),
            password: Some("some_password".to_string()),
            port: 1234,
        };

        let mut headers = HeaderMap::new();
        // some_user:some_password
        headers.append(
            AUTHORIZATION,
            "Basic c29tZV91c2VyOnNvbWVfcGFzc3dvcmQ=".parse().unwrap(),
        );
        assert!(is_authorized(&headers, &conf));
    }

    #[test]
    fn not_authorized_when_credentials_are_required_but_wrong_or_missing() {
        let conf = TriggerHttp {
            username: Some("some_user".to_string()),
            password: Some("some_password".to_string()),
            port: 5678,
        };

        let headers_without_auth = HeaderMap::new();
        assert!(!is_authorized(&headers_without_auth, &conf));

        let mut headers_with_wrong_pw = HeaderMap::new();
        // some_user:other_password
        headers_with_wrong_pw.append(
            AUTHORIZATION,
            "Basic c29tZV91c2VyOm90aGVyX3Bhc3N3b3Jk".parse().unwrap(),
        );
        assert!(!is_authorized(&headers_with_wrong_pw, &conf));

        let mut headers_with_wrong_user = HeaderMap::new();
        // other_user:some_password
        headers_with_wrong_user.append(
            AUTHORIZATION,
            "Basic b3RoZXJfdXNlcjpzb21lX3Bhc3N3b3Jk".parse().unwrap(),
        );
        assert!(!is_authorized(&headers_with_wrong_user, &conf));
    }

    #[test]
    fn authorization_works_for_username_without_password_config() {
        let conf = TriggerHttp {
            username: Some("some_user".to_string()),
            password: None,
            port: 816,
        };

        let mut headers_with_right_user = HeaderMap::new();
        // some_user
        headers_with_right_user.append(AUTHORIZATION, "Basic c29tZV91c2Vy".parse().unwrap());
        assert!(is_authorized(&headers_with_right_user, &conf));

        let mut headers_with_wrong_user = HeaderMap::new();
        // other_user
        headers_with_wrong_user.append(AUTHORIZATION, "Basic b3RoZXJfdXNlcg==".parse().unwrap());
        assert!(!is_authorized(&headers_with_wrong_user, &conf));
    }
}
