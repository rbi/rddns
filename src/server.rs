use std::collections::HashMap;
use futures;
use futures::future::Future;
use hyper;
use hyper::StatusCode;
use hyper::server::{Http, Request, Response, Service, NewService};

pub struct Server<T: Clone + 'static> {
    update_callback: fn(&T, &HashMap<String, String>) -> Result<(), String>,
    user_data: T,
}

impl<T: Clone + 'static> Server<T> {
    pub fn new(update_callback: fn(&T, &HashMap<String, String>) -> Result<(), String>, user_data: T)
               -> Server<T> {
        Server {
            update_callback,
            user_data,
        }
    }

    pub fn start_server(&self) {
        let addr = "[::]:3000".parse().unwrap();
        let service_creator: ServiceCreator<T> = ServiceCreator {
            update_callback: self.update_callback,
            user_data: self.user_data.clone(),
        };
        let server = Http::new().bind(&addr, service_creator).unwrap();
        server.run().unwrap();
    }

    pub fn http_port(&self) -> u16 {
        3000
    }
}

struct ServiceCreator<T> {
    update_callback: fn(&T, &HashMap<String, String>) -> Result<(), String>,
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
            user_data: self.user_data.clone(),
        })
    }
}

struct RequestHandler<T> {
    update_callback: fn(&T, &HashMap<String, String>) -> Result<(), String>,
    user_data: T,
}

impl<T> Service for RequestHandler<T> {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<Future<Item=Self::Response, Error=Self::Error>>;

    fn call(&self, req: Request) -> Self::Future {
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
        Box::new(futures::future::ok(
            Response::new()
                .with_status(return_code)
                .with_body(message)))
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
    let mut parts = param.split("=");
    let key_opt = parts.next();
    let value_opt = parts.next();

    if key_opt.is_none() || value_opt.is_none() {
        return None;
    }

    let key = key_opt.unwrap().to_string();

    let mut key_parts = key.split(".");
    let prefix_opt = key_parts.next();
    let key_name_opt = key_parts.next();
    if prefix_opt.is_none() || key_name_opt.is_none() {
        return None;
    }

    let key_name = key_name_opt.unwrap();
    if prefix_opt.unwrap() != "ip" || key_name.is_empty() {
        return None;
    }

    Some((key_name.to_string(), value_opt.unwrap().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn extract_address_parameters_correctly() {
        let mut expected = HashMap::new();
        expected.insert("first".to_string(), "2001:DB8:123:abcd::1".to_string());
        expected.insert("other".to_string(), "203.0.113.85".to_string());

        let query = Some("ip.first=2001:DB8:123:abcd::1&abitrary_param=abc&ip.other=203.0.113.85\
&broken_param&ip.=broken&ip=broken_too");
        let actual = extract_address_parameters(&query);

        assert_eq!(actual, expected);
    }

    #[test]
    fn extract_address_parameters_not_failing_when_empty_query() {
        let actual = extract_address_parameters(&None);

        assert!(actual.is_empty());
    }
}
