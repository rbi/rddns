extern crate tokio_core;
extern crate hyper;
extern crate hyper_tls;
extern crate futures;

#[macro_use]
extern crate serde_derive;
extern crate toml;

mod server;
mod updater;
mod config;

use config::DdnsEntry;
use updater::DdnsUpdater;

fn main() {
    let s = server::Server {};
    s.start_server(do_update);
}

fn do_update() -> Result<(), String> {
    println!("updating DDNS entries");

    let entry = DdnsEntry {
        url: "http://dummy".to_string(),
        username: "dummy".to_string(),
        password: "dummy".to_string(),
    };

    let mut updater = DdnsUpdater::new();

    let result = updater.update_dns(entry);
    if result.is_ok() {
        println!("updating DDNS entries succeed");
    } else {
        println!("Updating DDNS entries failed. Reason: {}", result.clone().unwrap_err());
    }
    result
}