extern crate base64;
extern crate futures;
extern crate hyper;
extern crate hyper_rustls;
extern crate ipnetwork;
extern crate pnet;
extern crate tokio;

#[macro_use]
extern crate serde_derive;
extern crate regex;
extern crate toml;
#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;
extern crate simplelog;

#[macro_use]
extern crate clap;

mod basic_auth_header;
mod command_line;
mod config;
mod resolver;
mod server;
mod update_executer;
mod updater;

use futures_util::stream::FuturesUnordered;
use futures_util::StreamExt;
use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::time::interval;

use simplelog::{
    ColorChoice, Config as SimpleLogConfig, LevelFilter, SimpleLogger, TermLogger, TerminalMode,
};

use command_line::{parse_command_line, ExecutionMode};
use config::{read_config, Config, Trigger};
use server::create_server;
use updater::Updater;

fn main() -> Result<(), String> {
    init_logging();

    let cmd_args = parse_command_line();

    let config = read_config(&cmd_args.config_file).map_err(|err| err.to_string())?;

    let rt = Runtime::new().unwrap();

    match cmd_args.execution_mode {
        ExecutionMode::TRIGGER => {
            if config.triggers.is_empty() {
                return Err("In trigger mode at least one trigger must be configured.".to_string());
            }
            let triggers = config.triggers.clone();
            let jobs = triggers
                .into_iter()
                .map(move |trigger| create_trigger_future(trigger, config.clone()))
                .collect::<FuturesUnordered<_>>()
                .collect::<Vec<_>>();
            let result = rt.block_on(jobs);
            combine_errors(result)
        }
        ExecutionMode::UPDATE => {
            let updater = Updater::new(config.clone());
            rt.block_on(updater.do_update(cmd_args.addresses))
        }
    }
}

fn init_logging() {
    let term_logger = TermLogger::init(
        LevelFilter::Info,
        SimpleLogConfig::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    );
    let logger = match term_logger {
        Ok(_) => term_logger,
        Err(_) => SimpleLogger::init(LevelFilter::Info, SimpleLogConfig::default()),
    };
    if logger.is_err() {
        eprintln!(
            "Failed to initialize logging framework. Nothing will be logged. Error was: {}",
            logger.unwrap_err()
        );
    }
}

async fn create_trigger_future(trigger: Trigger, config: Config) -> Result<(), String> {
    lazy_static! {
        static ref EMPTY: HashMap<String, IpAddr> = HashMap::new();
    }
    let updater = Updater::new(config.clone());
    match trigger {
        Trigger::HTTP(server) => {
            create_server(
                move |addr| {
                    let updater = updater.clone();
                    async move { updater.do_update(addr).await }
                },
                server.clone(),
            )
            .await
        }
        Trigger::TIMED(timed) => {
            let mut timer = interval(Duration::from_secs(timed.interval as u64));
            loop {
                timer.tick().await;
                updater.do_update(EMPTY.clone()).await.unwrap();
            }
        }
    }
}

fn combine_errors(results: Vec<Result<(), String>>) -> Result<(), String> {
    let error = results
        .into_iter()
        .filter(|res| res.is_err())
        .map(|res| res.unwrap_err())
        .collect::<Vec<_>>()
        .join("\n");

    if error.is_empty() || error == "\n" {
        Ok(())
    } else {
        Err(error.to_string())
    }
}
