extern crate tokio;
extern crate hyper;
extern crate hyper_rustls;
extern crate futures;
extern crate base64;
extern crate pnet;
extern crate ipnetwork;

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
mod resolver_derived;
mod resolver_interface;
mod updater;
mod basic_auth_header;

use std::collections::HashMap;
use std::net::IpAddr;

use tokio::runtime::Runtime;
use futures::future::{Future, ok, err, join_all};

use simplelog::{SimpleLogger, TermLogger, CombinedLogger, LevelFilter, Config as SimpleLogConfig};

use config::Config;
use updater::update_dns;
use command_line::{ExecutionMode, parse_command_line};
use futures::future::result;

fn main() -> Result<(), String> {
    init_logging();

    let cmd_args = parse_command_line();

    let config = config::read_config(&cmd_args.config_file).map_err(|err| err.to_string())?;

    let mut rt = Runtime::new().unwrap();
    match cmd_args.execution_mode {
        ExecutionMode::SERVER => rt.block_on(server::create_server(do_update, config.server.clone(), config)),
        ExecutionMode::UPDATE => rt.block_on(do_update(&config, &cmd_args.addresses))
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

fn do_update(config: &Config, addresses: &HashMap<String, IpAddr>) -> impl Future<Item=(), Error=String> + Send {
    info!("updating DDNS entries");

    let resolved_entries = resolver::resolve_config(config, addresses);
    let work = resolved_entries.iter()
        .map(|resolved| {
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
                    })
                )
        })
        .collect::<Vec<_>>();
    join_all(work).and_then(|results| {
        let error = results.join("\n");

        if error.is_empty() || error == "\n" {
            ok(())
        } else {
            err(error.to_string())
        }
    })
}
