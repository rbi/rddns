use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use futures_util::stream::FuturesUnordered;
use futures_util::StreamExt;

use crate::resolver::Resolver;

use super::config::{Config, DdnsEntry};
use super::resolver::ResolvedDdnsEntry;
use super::update_executer::UpdateExecutor;

#[derive(Clone, Debug)]
pub struct Updater {
    config: Config,
    cache: Arc<Mutex<HashMap<DdnsEntry, ResolvedDdnsEntry>>>,
    resolver: Resolver,
    update_executor: UpdateExecutor,
}

pub struct UpdateResults {
    pub warnings: Option<String>,
    pub errors: Option<String>,
}

enum UpdateResult {
    Ok,
    Warning(String),
    Error(String),
}

impl Updater {
    pub fn new(config: Config) -> Self {
        Updater {
            config,
            cache: Arc::new(Mutex::new(HashMap::new())),
            resolver: Resolver::new(),
            update_executor: UpdateExecutor::new(),
        }
    }

    pub async fn do_update(&self, addresses: HashMap<String, String>) -> UpdateResults {
        debug!("updating DDNS entries");

        let work = self
            .resolver
            .resolve_config(&self.config, &addresses)
            .await
            .iter()
            .map(|entry| async move {
                match entry {
                    Ok(resolved) => self.handle_resolved(resolved.clone()).await,
                    Err(err) => Some(error_to_update_result(&err.original, err.message.clone())),
                }
            })
            .collect::<FuturesUnordered<_>>()
            .collect::<Vec<_>>()
            .await;

        combine_results(work)
    }

    async fn handle_resolved(&self, resolved: ResolvedDdnsEntry) -> Option<UpdateResult> {
        if !self.has_changed(&resolved) {
            return None;
        }
        let executed = execute_resolved_dns_entry(&self.update_executor, &resolved).await;
        if let UpdateResult::Ok = executed {
            self.cache(resolved);
        }
        Some(executed)
    }

    fn has_changed(&self, resolved: &ResolvedDdnsEntry) -> bool {
        let cache = self.cache.lock().unwrap();
        let changed = cache
            .get(&resolved.original)
            .map(|last| last != resolved)
            .unwrap_or(true);

        if !changed {
            debug!(
                "Skip updating DDNS entry because it did not change {}",
                resolved
            );
        }

        changed
    }

    fn cache(&self, executed: ResolvedDdnsEntry) {
        let mut cache = self.cache.lock().unwrap();
        cache.insert(executed.original.clone(), executed.clone());
    }
}

fn error_to_update_result(entry: &DdnsEntry, error_message: String) -> UpdateResult {
    let allowed_to_fail = match entry {
        DdnsEntry::HTTP(http_entry) => http_entry.ignore_error,
        _ => false,
    };
    if allowed_to_fail {
        info!(
            "Updating DDNS \"{}\" failed but is allowed to fail. Reason: {}",
            entry, error_message
        );
        UpdateResult::Warning(error_message)
    } else {
        warn!(
            "Updating DDNS \"{}\" failed. Reason: {}",
            entry, error_message
        );
        UpdateResult::Error(error_message)
    }
}

async fn execute_resolved_dns_entry(
    update_executor: &UpdateExecutor,
    resolved: &ResolvedDdnsEntry,
) -> UpdateResult {
    let result = update_executor.update_dns(&resolved).await;
    if let Err(error_msg) = result {
        error_to_update_result(&resolved.original, error_msg)
    } else {
        info!("Successfully updated DDNS entry {}", resolved);
        UpdateResult::Ok
    }
}

fn combine_results(results: Vec<Option<UpdateResult>>) -> UpdateResults {
    let sorted = results
        .into_iter()
        .fold((vec![], vec![]), |mut result, element| {
            if let Some(element) = element {
                if let UpdateResult::Warning(warn) = element {
                    result.0.push(warn);
                } else if let UpdateResult::Error(error) = element {
                    result.1.push(error);
                }
            }
            result
        });

    UpdateResults {
        warnings: if sorted.0.is_empty() {
            None
        } else {
            Some(sorted.0.join("\n"))
        },
        errors: if sorted.1.is_empty() {
            None
        } else {
            Some(sorted.1.join("\n"))
        },
    }
}
