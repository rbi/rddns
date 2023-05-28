use std::collections::HashMap;
use futures::future::{ok, err, result, Future, FutureResult};
use hyper;
use hyper::StatusCode;
use hyper::{Request, Response, Body};
use hyper::service::Service;
use hyper::header::{HeaderMap, WWW_AUTHENTICATE, AUTHORIZATION};
use regex::Regex;
use std::net::{AddrParseError, IpAddr};
use std::convert::TryFrom;

use config::TriggerHttp;
use basic_auth_header::BasicAuth;

pub fn create_server<T, F>(update_callback: fn(&T, &HashMap<String, IpAddr>) -> F,
                           server_config: TriggerHttp, user_data: T) -> Box<dyn Future<Item=(), Error=String> + Send>
    where T: Clone + Send + Sync + 'static,
          F: Future<Item=(), Error=String> + Send + 'static {
    let port = server_config.port;
    match format!("[::]:{}", port).parse().map_err(|err: AddrParseError| err.to_string()) {
        Ok(addr) => {
            let service_creator = move || {
                let user_data = user_data.clone();
                let server_config = server_config.clone();
                let service: FutureResult<RequestHandler<T, F>, hyper::Error> = ok(RequestHandler {
                    update_callback,
                    server_config: server_config.clone(),
                    user_data: user_data.clone(),
                });
                service
            };

            info!("Listening on port {}", port);
            Box::new(hyper::Server::bind(&addr).serve(service_creator).map_err(|err| err.to_string()))
        }
        Err(_) => Box::new(err("Failed to parse address.".to_owned()))
    }
}

struct RequestHandler<T, F> {
    update_callback: fn(&T, &HashMap<String, IpAddr>) -> F,
    server_config: TriggerHttp,
    user_data: T,
}

impl<T, F> Service for RequestHandler<T, F>
    where T: Clone + Send + Sync + 'static,
          F: Future<Item=(), Error=String> + Send + 'static {
    type ReqBody = hyper::Body;
    type ResBody = hyper::Body;
    type Error = hyper::http::Error;
    type Future = Box<dyn Future<Item=Response<Body>, Error=Self::Error> + Send>;

    fn call(&mut self, req: Request<Self::ReqBody>) -> Self::Future {
        let authorized = check_authorisation(req.headers(), &self.server_config);
        match authorized {
            Ok(_) => {
                let ip_parameters = extract_address_parameters(&req.uri().query());
                Box::new((self.update_callback)(&self.user_data, &ip_parameters).then(move |update_result| {
                    let return_code = match update_result {
                        Ok(_) => StatusCode::OK,
                        Err(_) => StatusCode::BAD_GATEWAY
                    };
                    let message = match update_result {
                        Ok(_) => "success".to_string(),
                        Err(err) => err
                    };

                    result(Response::builder().status(return_code).body(Body::from(message)))
                }))
            }
            Err(_) => Box::new(result(Response::builder().status(StatusCode::UNAUTHORIZED).header(WWW_AUTHENTICATE, "Basic realm=\"rddns\"")
                .body(Body::empty())))
        }
    }
}

fn check_authorisation(headers: &HeaderMap, config: &TriggerHttp) -> Result<(), ()> {
    match config.username {
        Some(ref username) => {
            headers.get(AUTHORIZATION)
                .ok_or(())
                .and_then(|value| value.to_str().map_err(|_| ()))
                .and_then(|auth| BasicAuth::try_from(auth).map_err(|err| {
                    debug!("{}", err);
                    ()
                }))
                .and_then(|auth|
                    if auth.username.eq(username) && match config.password {
                        Some(ref config_password) => match auth.password {
                            Some(ref auth_password) => config_password.eq(auth_password),
                            None => false
                        }
                        None => true
                    } {
                        Ok(())
                    } else {
                        Err(())
                    })
        }
        None => Ok(())
    }
}

fn extract_address_parameters(query: &Option<&str>) -> HashMap<String, IpAddr> {
    let mut map: HashMap<String, IpAddr> = HashMap::new();
    let iter = query.map(|q| q.split("&"));
    match iter {
        Some(params) => for param in params {
            let address_param = to_address_param(param);
            match address_param {
                Some((key, value)) => match value.parse() {
                    Ok(addr) => map.insert(key, addr),
                    Err(_) => {
                        warn!("Value passed for IP address parameter \"{}\" is not a valid IP address. Ignoring it.", key);
                        None
                    }
                },
                _ => None
            };
        },
        _ => ()
    }
    map
}

fn to_address_param(param: &str) -> Option<(String, String)> {
    lazy_static! {
        static ref IP_PARAM: Regex = Regex::new(r"ip\[([^\]]+)]=(.+)").unwrap();
        static ref IP_BASE64_PARAM: Regex = Regex::new(r"ip.base64\[([^\]]+)]=(.+)").unwrap();
    }

    let result = IP_PARAM.captures(param).map(|groups| (groups[1].to_string(), groups[2].to_string()));
    if result.is_some() {
        return result;
    }

    IP_BASE64_PARAM.captures(param).and_then(|groups| base64_decode(&groups[2]).map( |resolved| (groups[1].to_string(), resolved)))
}

fn base64_decode(encoded: &str) -> Option<String> {
    match base64::decode(encoded) {
        Ok(decoded) => match String::from_utf8(decoded) {
            Ok(decoded_string) => Some(decoded_string),
            Err(_) => {
                warn!("Could not base64 decode an IP address parameter.");
                None
            }
        },
        Err(_) => {
            warn!("Could not base64 decode an IP address parameter.");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_address_parameters_correctly() {
        let mut expected = HashMap::new();
        expected.insert("first".to_string(), "2001:DB8:123:abcd::1".parse().unwrap());
        expected.insert("other".to_string(), "203.0.113.85".parse().unwrap());
        expected.insert("b64encoded".to_string(), "11.22.33.44".parse().unwrap());

        let query = Some("ip[first]=2001:DB8:123:abcd::1&abitrary_param=abc&ip[other]=203.0.113.85\
&broken_param&ip[=broken&ip=broken_too&ip.base64[b64encoded]=MTEuMjIuMzMuNDQ=");
        let actual = extract_address_parameters(&query);

        assert_eq!(actual, expected);
    }

    #[test]
    fn extract_address_parameters_not_failing_on_invalid_ip_addresses() {
        let mut expected = HashMap::new();
        expected.insert("other".to_string(), "203.0.113.85".parse().unwrap());

        let query = Some("ip[first]=invalid_address&ip[other]=203.0.113.85");
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
        headers_with_auth.append(AUTHORIZATION, "Basic c29tZV91c2VyOnNvbWVfcGFzc3dvcmQ".parse().unwrap());
        assert!(check_authorisation(&headers_with_auth, &conf).is_ok());

        let headers_without_auth = HeaderMap::new();
        assert!(check_authorisation(&headers_without_auth, &conf).is_ok());
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
        headers.append(AUTHORIZATION, "Basic c29tZV91c2VyOnNvbWVfcGFzc3dvcmQ".parse().unwrap());
        assert!(check_authorisation(&headers, &conf).is_ok());
    }

    #[test]
    fn not_authorized_when_credentials_are_required_but_wrong_or_missing() {
        let conf = TriggerHttp {
            username: Some("some_user".to_string()),
            password: Some("some_password".to_string()),
            port: 5678,
        };

        let headers_without_auth = HeaderMap::new();
        assert!(check_authorisation(&headers_without_auth, &conf).is_err());

        let mut headers_with_wrong_pw = HeaderMap::new();
        // some_user:other_password
        headers_with_wrong_pw.append(AUTHORIZATION, "Basic c29tZV91c2VyOm90aGVyX3Bhc3N3b3Jk".parse().unwrap());
        assert!(check_authorisation(&headers_with_wrong_pw, &conf).is_err());

        let mut headers_with_wrong_user = HeaderMap::new();
        // other_user:some_password
        headers_with_wrong_user.append(AUTHORIZATION, "Basic b3RoZXJfdXNlcjpzb21lX3Bhc3N3b3Jk".parse().unwrap());
        assert!(check_authorisation(&headers_with_wrong_user, &conf).is_err());
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
        assert!(check_authorisation(&headers_with_right_user, &conf).is_ok());

        let mut headers_with_wrong_user = HeaderMap::new();
        // other_user
        headers_with_wrong_user.append(AUTHORIZATION, "Basic b3RoZXJfdXNlcg==".parse().unwrap());
        assert!(check_authorisation(&headers_with_wrong_user, &conf).is_err());
    }
}
