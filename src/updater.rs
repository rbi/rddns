use hyper::{Body, Client, Request, Uri};
use hyper::header::AUTHORIZATION;
use futures::future::Future;
use hyper_rustls::HttpsConnector;

use resolver::ResolvedDdnsEntry;
use basic_auth_header::{to_auth_header_value, to_auth_header_value_no_password};


pub fn update_dns(ddns_entry: &ResolvedDdnsEntry) -> impl Future<Item=(), Error=String> {
    let uri: Uri = ddns_entry.url.parse().unwrap();
    let https_connector = HttpsConnector::new(4);
    let client = Client::builder().build(https_connector);

    let mut request = Request::builder();
    request.uri(uri);


    ddns_entry.original.username.as_ref().map(|username| {
        let header_value = ddns_entry.original.password.as_ref()
            .map_or(to_auth_header_value_no_password(username), |ref password| {
                to_auth_header_value(username, password)
            });
        request.header(AUTHORIZATION, header_value);
    });

    client.request(request.body(Body::empty()).unwrap())
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
