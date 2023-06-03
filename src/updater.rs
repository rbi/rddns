use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use futures::future::Future;
use futures_util::stream::FuturesUnordered;
use futures_util::StreamExt;

use super::config::{Config, DdnsEntry};
use super::resolver::{resolve_config, ResolveFailed, ResolvedDdnsEntry};
use super::update_executer::update_dns;

#[derive(Clone, Debug)]
pub struct Updater {
    config: Config,
    cache: Arc<Mutex<HashMap<DdnsEntry, ResolvedDdnsEntry>>>,
}

impl Updater {
    pub fn new(config: Config) -> Self {
        Updater {
            config,
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn do_update(&self, addresses: HashMap<String, IpAddr>) -> Result<(), String> {
        debug!("updating DDNS entries");

        let work = resolve_config(&self.config, &addresses)
            .iter()
            .filter(|entry| self.filter_unchanged(entry))
            .map(|resolved| execute_resolved_dns_entry(resolved))
            .map(|executed| self.cache_successfull(executed))
            .map(error_to_ok)
            .collect::<FuturesUnordered<_>>()
            .collect()
            .await;

        combine_errors(work)
    }

    fn filter_unchanged(&self, resolved: &Result<ResolvedDdnsEntry, ResolveFailed>) -> bool {
        if resolved.is_err() {
            return true;
        }
        let cache = self.cache.lock().unwrap();
        let resolved = resolved.as_ref().unwrap();
        let filter = cache
            .get(&resolved.original)
            .map(|last| last != resolved)
            .unwrap_or(true);

        if !filter {
            debug!(
                "Skip updating DDNS entry because it did not change {}",
                resolved
            );
        }

        filter
    }

    async fn cache_successfull(
        &self,
        executed: impl Future<Output = Result<ResolvedDdnsEntry, String>>,
    ) -> Result<ResolvedDdnsEntry, String> {
        let resolved = executed.await?;

        let mut cache = self.cache.lock().unwrap();
        cache.insert(resolved.original.clone(), resolved.clone());
        Ok(resolved)
    }
}

async fn execute_resolved_dns_entry(
    resolved: &Result<ResolvedDdnsEntry, ResolveFailed>,
) -> Result<ResolvedDdnsEntry, String> {
    let resolved = resolved
        .clone()
        .map_err(|e| format!("Updating DDNS \"{}\" failed. Reason: {}", e, e.message))?;
    let resolved2 = resolved.clone();
    let resolved3 = resolved.clone();

    update_dns(&resolved).await.map_err(move |error_msg| {
        format!(
            "Updating DDNS \"{}\" failed. Reason: {}",
            resolved2, error_msg
        )
    })?;
    info!("Successfully updated DDNS entry {}", resolved3);
    Ok(resolved)
}

async fn error_to_ok<T>(executed: impl Future<Output = Result<T, String>>) -> String {
    match executed.await {
        Ok(_) => "".to_owned(),
        Err(error_msg) => {
            error!("{}", error_msg);
            error_msg
        }
    }
}

fn combine_errors(results: Vec<String>) -> Result<(), String> {
    let error = results.join("\n");

    if error.is_empty() || error == "\n" {
        Ok(())
    } else {
        Err(error.to_string())
    }
}
