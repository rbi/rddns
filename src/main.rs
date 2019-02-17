extern crate tokio;
extern crate hyper;
extern crate hyper_tls;
extern crate futures;
extern crate base64;

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
mod basic_auth_header;

use std::collections::HashMap;
use std::fmt::Display;
use std::net::IpAddr;

use tokio::runtime::Runtime;
use futures::future::{lazy, result};

use simplelog::{SimpleLogger, TermLogger, CombinedLogger, LevelFilter, Config as SimpleLogConfig};

use config::Config;
use updater::update_dns;
use command_line::{ExecutionMode, parse_command_line};

fn main() -> Result<(), String>{
    init_logging();

    let cmd_args = parse_command_line();

    let config = config::read_config(&cmd_args.config_file).map_err(|err| err.to_string())?;

    match cmd_args.execution_mode {
        ExecutionMode::SERVER => {
            server::start_server(do_update, config.server.clone(), config)
        },
        ExecutionMode::UPDATE => {
            let mut rt = Runtime::new().unwrap();
            rt.block_on(lazy(move ||result(
            do_update(&config, &cmd_args.addresses)
                // error was already logged
                .map_err(|_err| String::new()))))
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
    let mut error = String::new();

    for entry in resolved_entries {
        match entry {
            Ok(ref resolved) => {
                let result = update_dns(resolved);
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