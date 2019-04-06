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

extern crate tokio_timer;

mod command_line;
mod server;
mod config;
mod resolver;
mod update_executer;
mod updater;
mod basic_auth_header;

use std::time::{Instant, Duration};
use tokio::runtime::Runtime;
use tokio_timer::Interval;
use futures::future::{Future, join_all};
use futures::stream::Stream;
use std::collections::HashMap;
use std::net::IpAddr;

use simplelog::{SimpleLogger, TermLogger, CombinedLogger, LevelFilter, Config as SimpleLogConfig};

use config::{read_config, Config, Trigger};
use command_line::{ExecutionMode, parse_command_line};
use updater::Updater;
use server::create_server;

fn main() -> Result<(), String> {
    init_logging();

    let cmd_args = parse_command_line();

    let config = read_config(&cmd_args.config_file).map_err(|err| err.to_string())?;

    let mut rt = Runtime::new().unwrap();

    match cmd_args.execution_mode {
        ExecutionMode::TRIGGER => {
            if config.triggers.is_empty() {
                return Err("In trigger mode at least one trigger must be configured.".to_string())
            }
            let triggers = config.triggers.clone();
            let jobs = triggers.into_iter().map(move |trigger| create_trigger_future(trigger, &config));
            rt.block_on(join_all(jobs).map(|_| ()))
        },
        ExecutionMode::UPDATE => {
            let updater = Updater::new(config.clone());
            rt.block_on(updater.do_update(&cmd_args.addresses))
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


fn create_trigger_future(trigger: Trigger, config: &Config) -> Box<Future<Item=(), Error=String> + Send> {
    lazy_static! {
        static ref EMPTY: HashMap<String, IpAddr> = HashMap::new();
    }
    let updater = Updater::new(config.clone());
    match trigger {
        Trigger::HTTP(server) => Box::new(create_server(|updater, addr| updater.do_update(addr), server.clone(), updater)),
        Trigger::TIMED(timed) => Box::new(Interval::new(Instant::now(), Duration::from_secs(timed.interval as u64))
                                          .map_err(|_| "".to_owned()).for_each(move |_| updater.do_update(&EMPTY)))
    }
}

