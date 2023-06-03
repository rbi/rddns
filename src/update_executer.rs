use hyper::header::AUTHORIZATION;
use hyper::{Body, Client, Request, Uri};
use hyper_rustls::HttpsConnectorBuilder;

use super::basic_auth_header::{to_auth_header_value, to_auth_header_value_no_password};
use super::config::{DdnsEntry, DdnsEntryHttp};
use super::resolver::ResolvedDdnsEntry;
use tokio::fs::write;

pub async fn update_dns(ddns_entry: &ResolvedDdnsEntry) -> Result<(), String> {
    match &ddns_entry.original {
        DdnsEntry::HTTP(http) => update_via_http(&http, &ddns_entry.resolved).await,
        DdnsEntry::FILE(file) => update_file(file.file.clone(), ddns_entry.resolved.clone()).await,
    }
}

async fn update_via_http(ddns_entry: &DdnsEntryHttp, resolved_url: &String) -> Result<(), String> {
    let uri: Uri = resolved_url.parse().unwrap();
    let https_connector = HttpsConnectorBuilder::new()
        .with_native_roots()
        .https_or_http()
        .enable_http1()
        .build();
    let client = Client::builder().build(https_connector);

    let mut request = Request::builder();
    request = request.uri(uri);

    let header_value = ddns_entry.username.as_ref().map(|username| {
        ddns_entry.password.as_ref().map_or(
            to_auth_header_value_no_password(username),
            |ref password| to_auth_header_value(username, password),
        )
    });

    request = match header_value {
        Some(value) => request.header(AUTHORIZATION, value),
        None => request,
    };

    let result = client
        .request(request.body(Body::empty()).unwrap())
        .await
        .map_err(|err| err.to_string())?;
    let result_code = result.status().as_u16();
    if result_code < 300 {
        Ok(())
    } else {
        Err(format!(
            "Failed to update DDNS entry. HTTP return code was {}.",
            result_code
        ))
    }
}

async fn update_file(file: String, resolved_content: String) -> Result<(), String> {
    write(file, resolved_content)
        .await
        .map_err(|err| err.to_string())
}
