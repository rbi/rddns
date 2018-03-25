extern crate tokio_core;
#[macro_use]
extern crate hyper;
extern crate hyper_tls;
extern crate futures;

#[macro_use]
extern crate serde_derive;
extern crate toml;
extern crate regex;
#[macro_use]
extern crate lazy_static;

#[macro_use] extern crate log;
extern crate simplelog;

mod server;
mod config;
mod resolver;
mod updater;

use std::env;
use std::path::PathBuf;
use std::collections::HashMap;
use std::fmt::Display;
use std::net::IpAddr;

use simplelog::{TermLogger, LevelFilter, Config as SimpleLogConfig};

use config::Config;
use updater::DdnsUpdater;

fn main() {
    let logger = TermLogger::init(LevelFilter::Info, SimpleLogConfig::default());
    if logger.is_err() {
        eprintln!("{}", logger.unwrap_err());
        return;
    }

    let config_file = get_config_file();
    if config_file.is_err() {
        return;
    }

    let config_or_error = config::read_config(&config_file.unwrap());
    if config_or_error.is_err() {
        error!("{}", config_or_error.unwrap_err());
        return;
    }
    let config = config_or_error.unwrap();

    let s = server::Server::new(do_update, config.server.clone(), config);
    info!("Listening on port {}", s.http_port());
    s.start_server();
}

fn get_config_file() -> Result<PathBuf, ()> {
    let mut args = env::args();
    let executable = args.next().unwrap_or("rddns".to_string());

    let usage = format!("Usage: {} [config-file]", executable);
    let path_string = args.next();
    if path_string.is_none() {
        error!("No configuration file was specified.");
        info!("{}", usage);
        return Err(());
    }
    let path = PathBuf::from(path_string.unwrap());
    if !path.is_file() {
        error!("\"{}\" is not a valid path to a config file.", path.to_str().unwrap());
        info!("{}", usage);
        return Err(());
    }

    Ok(path)
}

fn do_update(config: &Config, addresses: &HashMap<String, IpAddr>) -> Result<(), String> {
    info!("updating DDNS entries");

    let resolved_entries = resolver::resolve_config(config, addresses);

    let mut updater = DdnsUpdater::new();
    let mut error = String::new();

    for entry in resolved_entries {
        match entry {
            Ok(ref resolved) => {
                let result = updater.update_dns(resolved);
                match result {
                    Ok(_) => info!("Successfully updated DDNS entry {}", resolved),
                    Err(e) => handle_error_while_updating(&mut error, resolved, & e)
                }
            }
            Err(ref e) => handle_error_while_updating(&mut error, e, & e.message)
        }
    }
    if error.is_empty() {
        Ok(())
    } else {
        Err(error.to_string())
    }
}

fn handle_error_while_updating(error: &mut String, entity: &Display, message: &String) {
    let error_text = format!("Updating DDNS \"{}\" failed. Reason: {}", entity, message);
    error!("{}", error_text);
    error.push_str(&error_text);
    error.push('\n');
}