use hyper::{Body, Client, Request, Uri};
use hyper::header::AUTHORIZATION;
use futures::future::Future;
use hyper_rustls::HttpsConnector;

use tokio::prelude::{AsyncWrite};
use tokio::fs::File;
use config::{DdnsEntry, DdnsEntryHttp};
use resolver::ResolvedDdnsEntry;
use basic_auth_header::{to_auth_header_value, to_auth_header_value_no_password};


pub fn update_dns(ddns_entry: &ResolvedDdnsEntry) -> Box<dyn Future<Item=(), Error=String> + Send> {
    match &ddns_entry.original {
        DdnsEntry::HTTP(http) => update_via_http(&http, &ddns_entry.resolved),
        DdnsEntry::FILE(file) => update_file(file.file.clone(), ddns_entry.resolved.clone())
    }
}

fn update_via_http(ddns_entry: &DdnsEntryHttp, resolved_url: &String) -> Box<dyn Future<Item=(), Error=String> + Send> {
    let uri: Uri = resolved_url.parse().unwrap();
    let https_connector = HttpsConnector::new(4);
    let client = Client::builder().build(https_connector);

    let mut request = Request::builder();
    request.uri(uri);


    ddns_entry.username.as_ref().map(|username| {
        let header_value = ddns_entry.password.as_ref()
            .map_or(to_auth_header_value_no_password(username), |ref password| {
                to_auth_header_value(username, password)
            });
        request.header(AUTHORIZATION, header_value);
    });

    Box::new(client.request(request.body(Body::empty()).unwrap())
        .map_err(|err| err.to_string())
        .and_then(|result| {
            let result_code = result.status().as_u16();
            if result_code < 300 {
                Ok(())
            } else {
                Err(format!("Failed to update DDNS entry. HTTP return code was {}.", result_code))
            }
        }))

}

fn update_file(file: String, resolved_content: String) -> Box<dyn Future<Item=(), Error=String> + Send> {
    Box::new(File::create(file)
        .and_then(move |mut file| file
            .poll_write(resolved_content.as_bytes()))
        .map(|_| ())
        .map_err(|err| err.to_string()))
}
