use futures;
use futures::future::Future;
use hyper;
use hyper::server::{Http, Request, Response, Service};

pub struct Server;

impl Server {
    pub fn start_server(&self, update_callback: fn()) {
        let addr = "[::]:3000".parse().unwrap();
        let server = Http::new().bind(&addr, move || Ok(RequestHandler { update_callback })).unwrap();
        server.run().unwrap();
    }
}

struct RequestHandler {
    update_callback: fn()
}

impl Service for RequestHandler {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<Future<Item=Self::Response, Error=Self::Error>>;

    fn call(&self, _req: Request) -> Self::Future {
        (self.update_callback)();
        Box::new(futures::future::ok(
            Response::new().with_status(hyper::StatusCode::Ok)))
    }
}
