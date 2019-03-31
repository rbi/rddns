use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use futures::future::{result, Future, ok, err, join_all};

use update_executer::update_dns;
use resolver::{resolve_config, ResolvedDdnsEntry, ResolveFailed};
use config::{Config, DdnsEntry};

#[derive(Clone, Debug)]
pub struct Updater {
    config: Config,
    cache: Arc<Mutex<HashMap<DdnsEntry, ResolvedDdnsEntry>>>
}

impl Updater {
    pub fn new(config: Config) -> Self{
        Updater {
            config,
            cache: Arc::new(Mutex::new(HashMap::new()))
        }
    }

    pub fn do_update(&self, addresses: &HashMap<String, IpAddr>) -> impl Future<Item=(), Error=String> + Send {
        info!("updating DDNS entries");

        let work = resolve_config(&self.config, addresses).iter()
            .filter(|entry| self.filter_unchanged(entry))
            .map(execute_resolved_dns_entry)
            .map(|executed| self.cache_successfull(executed))
            .map(error_to_ok)
            .collect::<Vec<_>>();

        join_all(work).and_then(combine_errors)
    }

    fn filter_unchanged(&self, resolved: &Result<ResolvedDdnsEntry, ResolveFailed>) -> bool {
        if resolved.is_err() {
            return true;
        }
        let cache =self.cache.lock().unwrap();
        let resolved = resolved.as_ref().unwrap();
        let filter = cache.get(&resolved.original)
            .map(|last| last != resolved).unwrap_or(true);

        if !filter {
            debug!("Skip updating DDNS entry because it did not change {}", resolved);
        }

        filter
    }

    fn cache_successfull(&self, executed: impl Future<Item=ResolvedDdnsEntry, Error=String>) -> impl Future<Item=ResolvedDdnsEntry, Error=String> {
        let cache = self.cache.clone();
        executed.inspect(move |resolved| {
            let mut cache = cache.lock().unwrap();
            cache.insert(resolved.original.clone(), resolved.clone());
        })
    }
}

fn execute_resolved_dns_entry(resolved: &Result<ResolvedDdnsEntry, ResolveFailed>) -> impl Future<Item=ResolvedDdnsEntry, Error=String> {
    let resolved = resolved.clone();
    result(resolved)
        .map_err(|e| format!("Updating DDNS \"{}\" failed. Reason: {}", e, e.message))
        .and_then(|ref resolved| {
            let resolved2 = resolved.clone();
            let resolved3 = resolved.clone();
            update_dns(resolved)
                .map(move |_| resolved2)
                .inspect( |resolved| info!("Successfully updated DDNS entry {}", resolved))
                .map_err(move |error_msg| format!("Updating DDNS \"{}\" failed. Reason: {}", resolved3, error_msg))
        })
}


fn error_to_ok<T>(executed: impl Future<Item=T, Error=String>) -> impl Future<Item=String, Error=String> {
    executed
        .then(|result|
              ok(match result {
                  Ok(_) => "".to_owned(),
                  Err(error_msg) => {
                      error!("{}", error_msg);
                      error_msg
                  }}))
}

fn combine_errors(results: Vec<String>) -> impl Future<Item=(), Error=String> {
    let error = results.join("\n");

    if error.is_empty() || error == "\n" {
        ok(())
    } else {
        err(error.to_string())
    }
}
