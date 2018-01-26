use tokio_core::reactor::Core;
use hyper::{Body, Client, Request, Method};
use hyper::client::HttpConnector;
use hyper::header::{Authorization, Basic};
use hyper_tls::HttpsConnector;

use resolver::ResolvedDdnsEntry;

pub struct DdnsUpdater {
    core: Core,
    client: Client<HttpsConnector<HttpConnector>, Body>,
}

impl DdnsUpdater {
    pub fn new() -> DdnsUpdater {
        let core = Core::new().unwrap();
        let handle = core.handle();
        let client = Client::configure()
            .connector(HttpsConnector::new(4, &handle).unwrap())
            .build(&handle);
        DdnsUpdater {
            core,
            client,
        }
    }

    pub fn update_dns(& mut self, ddns_entry: &ResolvedDdnsEntry) -> Result<(), String> {
        let uri = ddns_entry.url.parse().unwrap();

        let mut request = Request::new(Method::Get, uri);
        ddns_entry.username.clone().map(|username| {
            let auth = Authorization(
                Basic {
                    username: username,
                    password: ddns_entry.password.clone(),
                }
            );
            request.headers_mut().set(auth);
        });

        let work = self.client.request(request);
        self.core.run(work)
            .map_err(|err| err.to_string())
            .and_then(|result| {
                let result_code = result.status().as_u16();
                if result_code < 300 {
                    Ok(())
                } else {
                    Err(format!("Failed to update DDNS entry. HTTP return code was {}.", result_code))
                }
            })
    }
}