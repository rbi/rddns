use hyper::client::HttpConnector;
use hyper::header::AUTHORIZATION;
use hyper::{Body, Client, Request, Uri};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use rustls::{ClientConfig, OwnedTrustAnchor, RootCertStore};
use webpki_roots::TLS_SERVER_ROOTS;

use super::basic_auth_header::{to_auth_header_value, to_auth_header_value_no_password};
use super::config::{DdnsEntry, DdnsEntryFile, DdnsEntryHttp};
use super::resolver::ResolvedDdnsEntry;
use tokio::fs::write;

#[derive(Clone, Debug)]
pub struct UpdateExecutor {
    client: Client<HttpsConnector<HttpConnector>>,
}

impl UpdateExecutor {
    pub fn new() -> Self {
        let mut root_store = RootCertStore::empty();
        root_store.add_server_trust_anchors(TLS_SERVER_ROOTS.0.iter().map(|ta| {
            OwnedTrustAnchor::from_subject_spki_name_constraints(
                ta.subject,
                ta.spki,
                ta.name_constraints,
            )
        }));
        let config = ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        let https_connector = HttpsConnectorBuilder::new()
            .with_tls_config(config)
            .https_or_http()
            .enable_http1()
            .build();
        let client = Client::builder().build(https_connector);

        UpdateExecutor { client }
    }

    pub async fn update_dns(&self, ddns_entry: &ResolvedDdnsEntry) -> Result<(), String> {
        match &ddns_entry.resolved {
            DdnsEntry::HTTP(http) => update_via_http(self.client.clone(), http).await,
            DdnsEntry::FILE(file) => update_file(file).await,
        }
    }
}

async fn update_via_http(
    client: Client<HttpsConnector<HttpConnector>>,
    ddns_entry: &DdnsEntryHttp,
) -> Result<(), String> {
    let uri: Uri = ddns_entry.url.parse().unwrap();

    let mut request = Request::builder();
    request = request.uri(uri);

    let auth_header_value = ddns_entry.username.as_ref().map(|username| {
        ddns_entry.password.as_ref().map_or(
            to_auth_header_value_no_password(username),
            |ref password| to_auth_header_value(username, password),
        )
    });

    request = match auth_header_value {
        Some(value) => request.header(AUTHORIZATION, value),
        None => request,
    };

    for (header, value) in &ddns_entry.headers {
        request = request.header(header, value);
    }

    let body = match &ddns_entry.body {
        Some(body) => Body::from(body.clone()),
        None => Body::empty(),
    };

    let result = client
        .request(request.body(body).unwrap())
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

async fn update_file(file: &DdnsEntryFile) -> Result<(), String> {
    write(file.file.clone(), file.replace.clone())
        .await
        .map_err(|err| err.to_string())
}
