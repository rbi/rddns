use clap::{Arg, ArgAction, Command};
use regex::Regex;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct CommandLine {
    pub addresses: HashMap<String, String>,
    pub execution_mode: ExecutionMode,
    pub config_file: PathBuf,
}

pub enum ExecutionMode {
    UPDATE,
    TRIGGER,
}

pub fn parse_command_line() -> CommandLine {
    let matches = command!()
        .subcommand_required(true)
        .arg(Arg::new("config")
            .short('c')
            .long("config")
            .help("The path to the configuration file.")
            .action(ArgAction::Set)
            .required(true))
        .subcommand(Command::new("update")
            .about("Triggers a single update of all DynDNS entries.")
            .arg(Arg::new("ip")
                .long("ip")
                .short('i')
                .help("The current IP addresses for IP address configurations of type \"parameter\".\
They must have the form [name]=[address], e.g. my_parameter=203.0.113.25 .")
                .action(ArgAction::Append)
                .value_parser(parse_ip_parameter)))
        .subcommand(Command::new("trigger")
            .about("Starts and waits for configured triggers for updating DynDNS entries to occure."))
        .get_matches();

    CommandLine {
        addresses: match matches.subcommand_matches("update") {
            Some(update_matches) => update_matches
                .get_many::<(String, String)>("ip")
                .map(|val| val.map(|val| val.clone()).collect())
                .unwrap_or_else(HashMap::new),
            _ => HashMap::new(),
        },
        execution_mode: match matches.subcommand_name() {
            Some("update") => ExecutionMode::UPDATE,
            Some("trigger") => ExecutionMode::TRIGGER,
            _ => panic!("BUG: No or unknown sub command was passed. This should not be possible."),
        },
        config_file: get_config_file(matches.get_one::<String>("config").unwrap()),
    }
}

fn parse_ip_parameter(value: &str) -> Result<(String, String), String> {
    lazy_static! {
        static ref IP_PARAM: Regex = Regex::new(r"([^=]+)=(.+)").unwrap();
    }
    match IP_PARAM
        .captures(&value)
        .map(|groups| (groups[1].to_string(), groups[2].to_string()))
    {
        Some((name, parameter)) => Ok((name, parameter)),
        None => Err(format!(
            "IP parameter must have the format [name]=[address] but got \"{}\".",
            value
        )),
    }
}

fn get_config_file(config_file: &str) -> PathBuf {
    let path = PathBuf::from(config_file);
    if !path.is_file() {
        error!(
            "\"{}\" is not a valid path to a config file.",
            path.to_str().unwrap()
        );
        panic!()
    }
    path
}
