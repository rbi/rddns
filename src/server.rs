use futures;
use futures::future::Future;
use hyper;
use hyper::StatusCode;
use hyper::server::{Http, Request, Response, Service};

pub struct Server;

impl Server {
    pub fn start_server(&self, update_callback: fn() -> Result<(), String>) {
        let addr = "[::]:3000".parse().unwrap();
        let server = Http::new().bind(&addr, move || Ok(RequestHandler { update_callback })).unwrap();
        println!("Listening on port 3000");
        server.run().unwrap();
    }
}

struct RequestHandler {
    update_callback: fn() -> Result<(), String>
}

impl Service for RequestHandler {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<Future<Item=Self::Response, Error=Self::Error>>;

    fn call(&self, _req: Request) -> Self::Future {
        let update_result = (self.update_callback)();
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
