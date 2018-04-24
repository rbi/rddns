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

#[macro_use]
extern crate log;
extern crate simplelog;

#[macro_use]
extern crate clap;

mod command_line;
mod server;
mod config;
mod resolver;
mod updater;

use std::collections::HashMap;
use std::fmt::Display;
use std::net::IpAddr;
use std::process::exit;

use simplelog::{SimpleLogger, TermLogger, CombinedLogger, LevelFilter, Config as SimpleLogConfig};

use config::Config;
use updater::DdnsUpdater;
use command_line::{ExecutionMode, parse_command_line};

fn main() {
    init_logging();

    let cmd_args = parse_command_line();

    let config_or_error = config::read_config(&cmd_args.config_file);
    if config_or_error.is_err() {
        error!("{}", config_or_error.unwrap_err());
        exit(1);
    }
    let config = config_or_error.unwrap();

    match cmd_args.execution_mode {
        ExecutionMode::SERVER => {
            let s = server::Server::new(do_update, config.server.clone(), config);
            s.start_server();
        },
        ExecutionMode::UPDATE => {
            match do_update(&config, &cmd_args.addresses) {
                Ok(_) => exit(0),
                Err(_) => exit(1),
            }
        }
    }

}

fn init_logging() {
    let term_logger = TermLogger::new(LevelFilter::Info, SimpleLogConfig::default());
    let logger = if term_logger.is_some() {
        CombinedLogger::init(vec![term_logger.unwrap()])
    } else {
        SimpleLogger::init(LevelFilter::Info, SimpleLogConfig::default())
    };
    if logger.is_err() {
        eprintln!("Failed to initialize logging framework. Nothing will be logged. Error was: {}", logger.unwrap_err());
    }
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
                    Err(e) => handle_error_while_updating(&mut error, resolved, &e, !resolved.original.ignore_error)
                }
            }
            Err(ref e) => handle_error_while_updating(&mut error, e, &e.message, true)
        }
    }
    if error.is_empty() {
        Ok(())
    } else {
        Err(error.to_string())
    }
}

fn handle_error_while_updating(error: &mut String, entity: &Display, message: &String, return_error: bool) {
    let error_text = format!("Updating DDNS \"{}\" failed. Reason: {}", entity, message);
    error!("{}", error_text);
    if return_error {
        error.push_str(&error_text);
        error.push('\n');
    }
}