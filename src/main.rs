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

use std::env;
use std::path::PathBuf;
use std::collections::HashMap;

use config::Config;
use updater::DdnsUpdater;

fn main() {
    let config_file = get_config_file();
    if config_file.is_err() {
        return;
    }

    let config_or_error = config::read_config(&config_file.unwrap());
    if config_or_error.is_err() {
        eprintln!("{}", config_or_error.unwrap_err());
        return;
    }
    let config = config_or_error.unwrap();

    let s = server::Server::new(do_update, config);
    println!("Listening on port {}", s.http_port());
    s.start_server();
}

fn get_config_file() -> Result<PathBuf, ()> {
    let mut args = env::args();
    let executable = args.next().unwrap_or("rddns".to_string());

    let usage = format!("Usage: {} [config-file]", executable);
    let path_string = args.next();
    if path_string.is_none() {
        eprintln!("No configuration file was specified.");
        println!("{}", usage);
        return Err(());
    }
    let path = PathBuf::from(path_string.unwrap());
    if !path.is_file() {
        eprintln!("\"{}\" is not a valid path to a config file.", path.to_str().unwrap());
        println!("{}", usage);
        return Err(());
    }

    Ok(path)
}

fn do_update(config: &Config, addresses: &HashMap<String, String>) -> Result<(), String> {
    println!("updating DDNS entries");

    let mut updater = DdnsUpdater::new();
    let mut error = String::new();

    for entry in &config.ddns_entries {
        let result = updater.update_dns(&entry);
        match result {
            Ok(_) => println!("Successfully updated DDNS entry {}", entry),
            Err(e) => {
                let error_text = format!("Updating DDNS \"{}\" failed. Reason: {}", entry, e);
                eprintln!("{}", error_text);
                error.push_str(&error_text);
                error.push('\n');
            }
        }
    }
    if error.is_empty() {
        Ok(())
    } else {
        Err(error.to_string())
    }
}