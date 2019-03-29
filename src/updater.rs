use std::collections::HashMap;
use std::net::IpAddr;

use futures::future::{result, Future, ok, err, join_all};

use update_executer::update_dns;
use resolver::{resolve_config, ResolvedDdnsEntry, ResolveFailed};
use config::Config;

pub fn do_update(config: &Config, addresses: &HashMap<String, IpAddr>) -> impl Future<Item=(), Error=String> + Send {
    info!("updating DDNS entries");

    let resolved_entries = resolve_config(config, addresses);
    let work = resolved_entries.iter()
        .map(execute_resolved_dns_entry)
        .collect::<Vec<_>>();

    join_all(work).and_then(combine_errors)
}

fn execute_resolved_dns_entry(resolved: &Result<ResolvedDdnsEntry, ResolveFailed>) -> impl Future<Item=String, Error=String> + Send {
    let resolved = resolved.clone();
    result(resolved)
        .map_err(|e| format!("Updating DDNS \"{}\" failed. Reason: {}", e, e.message))
        .and_then(|ref resolved| {
            let resolved2 = resolved.clone();
            let resolved3 = resolved.clone();
            update_dns(resolved)
                .map(move |_| info!("Successfully updated DDNS entry {}", resolved2))
                .map_err(move |error_msg| format!("Updating DDNS \"{}\" failed. Reason: {}", resolved3, error_msg))
        })
        .then(|result|
            ok(match result {
                Ok(_) => "".to_owned(),
                Err(error_msg) => {
                    error!("{}", error_msg);
                    error_msg
                }
            }))
}

fn combine_errors(results: Vec<String>) -> impl Future<Item=(), Error=String> + Send {
    let error = results.join("\n");

    if error.is_empty() || error == "\n" {
        ok(())
    } else {
        err(error.to_string())
    }
}
