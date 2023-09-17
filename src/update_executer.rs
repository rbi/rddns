use std::cmp::min;
use std::collections::HashMap;

use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc, Mutex};

use hyper::body::HttpBody;
use hyper::client::HttpConnector;
use hyper::header::AUTHORIZATION;
use hyper::{Body, Client, Request, Response, Uri};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use rustls::client::ServerCertVerifier;
use rustls::{Certificate, ClientConfig, OwnedTrustAnchor, RootCertStore};
use rustls_native_certs::load_native_certs;
use webpki_roots::TLS_SERVER_ROOTS;

use crate::config::ServerCertValidation;

use super::basic_auth_header::{to_auth_header_value, to_auth_header_value_no_password};
use super::config::{DdnsEntry, DdnsEntryFile, DdnsEntryHttp};
use super::resolver::ResolvedDdnsEntry;
use tokio::fs::write;

#[derive(Clone, Debug)]
pub struct UpdateExecutor {
    clients: Arc<Mutex<HashMap<ServerCertValidation, Client<HttpsConnector<HttpConnector>>>>>,
}

impl UpdateExecutor {
    pub fn new() -> Self {
        UpdateExecutor {
            clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn update_dns(&self, ddns_entry: &ResolvedDdnsEntry) -> Result<(), String> {
        match &ddns_entry.resolved {
            DdnsEntry::HTTP(http) => update_via_http(self.get_client(http)?, http).await,
            DdnsEntry::FILE(file) => update_file(file).await,
        }
    }

    fn get_client(
        &self,
        http: &DdnsEntryHttp,
    ) -> Result<Client<HttpsConnector<HttpConnector>>, String> {
        let mut clients = self.clients.lock().unwrap();
        match clients.get(&http.server_cert_validation) {
            Some(client) => Ok(client.clone()),
            None => {
                let client = create_client(&http.server_cert_validation)?;
                clients.insert(http.server_cert_validation.clone(), client.clone());
                Ok(client)
            }
        }
    }
}

fn create_client(
    server_cert_validation: &ServerCertValidation,
) -> Result<Client<HttpsConnector<HttpConnector>>, String> {
    let config: Result<ClientConfig, String> = match server_cert_validation {
        ServerCertValidation::MOZILLA => {
            let mut root_store = RootCertStore::empty();
            root_store.add_server_trust_anchors(TLS_SERVER_ROOTS.iter().map(|ta| {
                OwnedTrustAnchor::from_subject_spki_name_constraints(
                    ta.subject,
                    ta.spki,
                    ta.name_constraints,
                )
            }));
            Ok(ClientConfig::builder()
                .with_safe_defaults()
                .with_root_certificates(root_store)
                .with_no_client_auth())
        }
        ServerCertValidation::SYSTEM => {
            let mut root_store = RootCertStore::empty();
            match load_native_certs() {
                Ok(certs) => {
                    let mut cert_bytes = Vec::with_capacity(certs.len());
                    for cert in certs {
                        cert_bytes.push(cert.0);
                    }
                    root_store.add_parsable_certificates(cert_bytes.as_slice());
                    ()
                }
                Err(err) => warn!("Failed to load system CA certificates: {}", err),
            };
            Ok(ClientConfig::builder()
                .with_safe_defaults()
                .with_root_certificates(root_store)
                .with_no_client_auth())
        }
        ServerCertValidation::CUSTOM(path) => {
            let file = File::open(path.ca.clone()).map_err(|err| {
                format!(
                    "Failed to open server_cert_validation ca file '{}': {}",
                    path.ca.display(),
                    err.to_string()
                )
            })?;

            let mut reader = BufReader::new(file);
            let certs = rustls_pemfile::certs(&mut reader).map_err(|err| {
                format!(
                    "Failed to read server_cert_validation ca file '{}': {}",
                    path.ca.display(),
                    err.to_string()
                )
            })?;

            let mut root_store = RootCertStore::empty();
            for cert in certs.into_iter().map(Certificate) {
                root_store.add(&cert).map_err(|err| {
                    format!(
                        "Failed to read server_cert_validation ca file '{}': {}",
                        path.ca.display(),
                        err.to_string()
                    )
                })?;
            }

            Ok(ClientConfig::builder()
                .with_safe_defaults()
                .with_root_certificates(root_store)
                .with_no_client_auth())
        }
        ServerCertValidation::DISABLED => Ok(ClientConfig::builder()
            .with_safe_defaults()
            .with_custom_certificate_verifier(Arc::new(TrustAllCerts {}))
            .with_no_client_auth()),
    };

    let https_connector = HttpsConnectorBuilder::new()
        .with_tls_config(config?)
        .https_or_http()
        .enable_http1()
        .build();
    Ok(Client::builder().build(https_connector))
}

struct TrustAllCerts {}

impl ServerCertVerifier for TrustAllCerts {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}

async fn update_via_http(
    client: Client<HttpsConnector<HttpConnector>>,
    ddns_entry: &DdnsEntryHttp,
) -> Result<(), String> {
    let uri: Uri = ddns_entry.url.parse().unwrap();

    let mut request = Request::builder();
    request = request.uri(uri);

    request = request.method(ddns_entry.method.to_string().as_str());

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
        .request(request.body(body).map_err(|err| err.to_string())?)
        .await
        .map_err(|err| err.to_string())?;
    let result_code = result.status().as_u16();
    if result_code < 300 {
        Ok(())
    } else {
        let status = result.status().to_string();
        let response = read_start_of_body(997, result).await?;

        // };
        Err(format!(
            "Failed to update DDNS entry. HTTP response was: {}: {}",
            status, response
        ))
    }
}

async fn read_start_of_body(capacity: usize, mut result: Response<Body>) -> Result<String, String> {
    let mut response_buffer = String::with_capacity(capacity + 3); // for three dots at the end
    while let Some(next) = result.data().await {
        let chunk = next.map_err(|err| err.to_string())?;
        let chunk = chunk.escape_ascii().to_string();
        let chunk = &chunk[0..min(chunk.len(), capacity - response_buffer.len())];
        response_buffer.push_str(chunk);

        if response_buffer.len() >= capacity {
            break;
        }
    }
    if response_buffer.len() >= capacity {
        response_buffer.push_str("...");
    }
    Ok(response_buffer)
}

async fn update_file(file: &DdnsEntryFile) -> Result<(), String> {
    write(file.file.clone(), file.replace.clone())
        .await
        .map_err(|err| err.to_string())
}
