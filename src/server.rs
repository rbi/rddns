use std::collections::HashMap;
use futures;
use futures::future::Future;
use hyper;
use hyper::StatusCode;
use hyper::server::{Http, Request, Response, Service, NewService};
use hyper::header::{Authorization, Basic, Headers};
use regex::Regex;

use config::Server as ServerConfig;

header!(
    (WWWAuthenticate, "WWW-Authenticate") => Cow[str]
);

pub struct Server<T: Clone + 'static> {
    update_callback: fn(&T, &HashMap<String, String>) -> Result<(), String>,
    server_config: ServerConfig,
    user_data: T,
    port: u16,
}

impl<T: Clone + 'static> Server<T> {
    pub fn new(update_callback: fn(&T, &HashMap<String, String>) -> Result<(), String>,
               server_config: ServerConfig, user_data: T) -> Server<T> {
        Server {
            update_callback,
            port: server_config.port.unwrap_or(3092),
            server_config,
            user_data,
        }
    }

    pub fn start_server(&self) {
        let addr = format!("[::]:{}", self.port).parse().unwrap();
        let service_creator: ServiceCreator<T> = ServiceCreator {
            update_callback: self.update_callback,
            server_config: self.server_config.clone(),
            user_data: self.user_data.clone(),
        };
        let server = Http::new().bind(&addr, service_creator).unwrap();
        server.run().unwrap();
    }

    pub fn http_port(&self) -> u16 {
        self.port
    }
}

struct ServiceCreator<T> {
    update_callback: fn(&T, &HashMap<String, String>) -> Result<(), String>,
    server_config: ServerConfig,
    user_data: T,
}

impl<T: Clone> NewService for ServiceCreator<T> {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Instance = RequestHandler<T>;

    fn new_service(&self) -> ::std::io::Result<Self::Instance> {
        Ok(RequestHandler {
            update_callback: self.update_callback,
            server_config: self.server_config.clone(),
            user_data: self.user_data.clone(),
        })
    }
}

struct RequestHandler<T> {
    update_callback: fn(&T, &HashMap<String, String>) -> Result<(), String>,
    server_config: ServerConfig,
    user_data: T,
}

impl<T> Service for RequestHandler<T> {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<Future<Item=Self::Response, Error=Self::Error>>;

    fn call(&self, req: Request) -> Self::Future {
        let authorized = check_authorisation(req.headers(), &self.server_config);
        let response = match authorized {
            Ok(_) => {
                let ip_parameters = extract_address_parameters(&req.query());
                let update_result = (self.update_callback)(&self.user_data, &ip_parameters);
                let return_code = match update_result {
                    Ok(_) => StatusCode::Ok,
                    Err(_) => StatusCode::BadGateway
                };
                let message = match update_result {
                    Ok(_) => "success".to_string(),
                    Err(err) => err
                };
                Response::new().with_status(return_code).with_body(message)
            }
            Err(_) => {
                Response::new().with_status(StatusCode::Unauthorized).with_header(
                    WWWAuthenticate::new("Basic realm=\"rddns\""))
            }
        };


        Box::new(futures::future::ok(response))
    }
}

fn check_authorisation(headers: &Headers, config: &ServerConfig) -> Result<(), ()> {
    match config.username {
        Some(ref username) => {
            let auth_header: Option<&Authorization<Basic>> = headers.get();
            match auth_header {
                Some(ref auth) =>
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
                    },
                None => Err(())
            }
        }
        None => Ok(())
    }
}

fn extract_address_parameters(query: &Option<&str>) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    let iter = query.map(|q| q.split("&"));
    match iter {
        Some(params) => for param in params {
            let address_param = to_address_param(param);
            match address_param {
                Some((key, value)) => map.insert(key, value),
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
    }

    IP_PARAM.captures(param).map(|groups| (groups[1].to_string(), groups[2].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_address_parameters_correctly() {
        let mut expected = HashMap::new();
        expected.insert("first".to_string(), "2001:DB8:123:abcd::1".to_string());
        expected.insert("other".to_string(), "203.0.113.85".to_string());

        let query = Some("ip[first]=2001:DB8:123:abcd::1&abitrary_param=abc&ip[other]=203.0.113.85\
&broken_param&ip[=broken&ip=broken_too");
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
        let conf = ServerConfig {
            username: None,
            password: None,
            port: None,
        };

        let mut headers_with_auth = Headers::new();
        headers_with_auth.set(Authorization(
            Basic {
                username: "some_user".to_string(),
                password: Some("some_password".to_string()),
            }
        ));
        assert!(check_authorisation(&headers_with_auth, &conf).is_ok());

        let headers_without_auth = Headers::new();
        assert!(check_authorisation(&headers_without_auth, &conf).is_ok());
    }

    #[test]
    fn autorized_when_correct_credentials_are_passed() {
        let conf = ServerConfig {
            username: Some("some_user".to_string()),
            password: Some("some_password".to_string()),
            port: None,
        };


        let mut headers = Headers::new();
        headers.set(Authorization(
            Basic {
                username: "some_user".to_string(),
                password: Some("some_password".to_string()),
            }
        ));
        assert!(check_authorisation(&headers, &conf).is_ok());
    }

    #[test]
    fn not_authorized_when_credentials_are_required_but_wrong_or_missing() {
        let conf = ServerConfig {
            username: Some("some_user".to_string()),
            password: Some("some_password".to_string()),
            port: None,
        };

        let headers_without_auth = Headers::new();
        assert!(check_authorisation(&headers_without_auth, &conf).is_err());

        let mut headers_with_wrong_pw = Headers::new();
        headers_with_wrong_pw.set(Authorization(
            Basic {
                username: "some_user".to_string(),
                password: Some("other_password".to_string()),
            }
        ));
        assert!(check_authorisation(&headers_with_wrong_pw, &conf).is_err());

        let mut headers_with_wrong_user = Headers::new();
        headers_with_wrong_user.set(Authorization(
            Basic {
                username: "other_user".to_string(),
                password: Some("some_password".to_string()),
            }
        ));
        assert!(check_authorisation(&headers_with_wrong_user, &conf).is_err());
    }

    #[test]
    fn authorization_works_for_username_without_password_config() {
        let conf = ServerConfig {
            username: Some("some_user".to_string()),
            password: None,
            port: None,
        };

        let mut headers_with_right_user = Headers::new();
        headers_with_right_user.set(Authorization(
            Basic {
                username: "some_user".to_string(),
                password: None,
            }
        ));
        assert!(check_authorisation(&headers_with_right_user, &conf).is_ok());

        let mut headers_with_wrong_user = Headers::new();
        headers_with_wrong_user.set(Authorization(
            Basic {
                username: "other_user".to_string(),
                password: None,
            }
        ));
        assert!(check_authorisation(&headers_with_wrong_user, &conf).is_err());
    }
}
