use futures;
use futures::future::Future;
use hyper;
use hyper::StatusCode;
use hyper::server::{Http, Request, Response, Service, NewService};

pub struct Server<T: Clone + 'static> {
    update_callback: fn(&T) -> Result<(), String>,
    user_data: T,
}

impl<T: Clone + 'static> Server<T> {
    pub fn new(update_callback: fn(&T) -> Result<(), String>, user_data: T) -> Server<T> {
        Server {
            update_callback,
            user_data,
        }
    }

    pub fn start_server(&self) {
        let addr = "[::]:3000".parse().unwrap();
        let service_creator: ServiceCreator<T> = ServiceCreator {
            update_callback: self.update_callback,
            user_data: self.user_data.clone()
        };
        let server = Http::new().bind(&addr, service_creator ).unwrap();
        server.run().unwrap();
    }

    pub fn http_port(&self) -> u16 {
        3000
    }
}

struct ServiceCreator<T> {
    update_callback: fn(&T) -> Result<(), String>,
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
    update_callback: fn(&T) -> Result<(), String>,
    user_data: T,
}

impl<T> Service for RequestHandler<T> {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<Future<Item=Self::Response, Error=Self::Error>>;

    fn call(&self, _req: Request) -> Self::Future {
        let update_result = (self.update_callback)(&self.user_data);
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
