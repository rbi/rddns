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
mod update_executer;
mod updater;
mod basic_auth_header;

use tokio::runtime::Runtime;

use simplelog::{SimpleLogger, TermLogger, CombinedLogger, LevelFilter, Config as SimpleLogConfig};

use command_line::{ExecutionMode, parse_command_line};
use updater::do_update;

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

