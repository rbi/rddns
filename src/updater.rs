use tokio_core::reactor::Core;
use hyper::{Client, Request, Method};
use hyper::header::{Authorization, Basic};

pub struct DdnsEntry {
    pub url: String,
    pub username: String,
    pub password: String,
}

pub struct DdnsUpdater {
    core: Core,
    client: Client<::hyper::client::HttpConnector, ::hyper::Body>,
}

impl DdnsUpdater {
    pub fn new() -> DdnsUpdater {
        let core = Core::new().unwrap();
        let client = Client::new(&core.handle());
        DdnsUpdater {
            core,
            client,
        }
    }

    pub fn update_dns(& mut self, ddns_entry: DdnsEntry) -> Result<(), String> {
        let uri = ddns_entry.url.parse().unwrap();

        let mut request = Request::new(Method::Get, uri);
        let auth = Authorization(
            Basic {
                username: ddns_entry.username,
                password: Some(ddns_entry.password),
            }
        );
        request.headers_mut().set(auth);

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
