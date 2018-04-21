use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use clap::{Arg, App, AppSettings, SubCommand};

pub struct CommandLine {
    pub addresses: HashMap<String, IpAddr>,
    pub execution_mode: ExecutionMode,
    pub config_file: PathBuf,
}

pub enum ExecutionMode {
    UPDATE,
    SERVER,
}

pub fn parse_command_line() -> CommandLine {
    let matches = App::new("rddns")
        .author(crate_authors!())
        .version(crate_version!())
        .setting(AppSettings::SubcommandRequired)
        .setting(AppSettings::VersionlessSubcommands)
        .arg(Arg::with_name("config")
            .short("c")
            .help("path to the configuration file")
            .takes_value(true)
            .required(true))
        .subcommand(SubCommand::with_name("update")
            .about("triggers a single update of all DynDNS entries"))
        .subcommand(SubCommand::with_name("server")
            .about("starts an HTTP server listening for update requests for DynDNS entries"))
        .get_matches();

    CommandLine {
        addresses: HashMap::new(),
        execution_mode: match matches.subcommand_name() {
            Some("update") => ExecutionMode::UPDATE,
            Some("server") => ExecutionMode::SERVER,
            _ => panic!("BUG: No or unknown sub command was passed. This should not be possible.")
        },
        config_file: get_config_file(matches.value_of("config").unwrap()),
    }
}

fn get_config_file(config_file: &str) -> PathBuf {
    let path = PathBuf::from(config_file);
    if !path.is_file() {
        error!("\"{}\" is not a valid path to a config file.", path.to_str().unwrap());
        panic!()
    }
    path
}