use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use regex::Regex;
use clap::{Arg, App, ArgMatches, AppSettings, SubCommand};

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
            .long("config")
            .help("The path to the configuration file.")
            .takes_value(true)
            .required(true))
        .subcommand(SubCommand::with_name("update")
            .about("Triggers a single update of all DynDNS entries.")
            .arg(Arg::with_name("ip")
                .long("ip")
                .short("i")
                .help("The current IP addresses for IP address configurations of type \"parameter\".\
They must have the form [name]=[address], e.g. my_parameter=203.0.113.25 .")
                .takes_value(true)
                .multiple(true)
                .use_delimiter(true)
                .validator(validate_ip_parameter)))
        .subcommand(SubCommand::with_name("server")
            .about("Starts an HTTP server listening for update requests for DynDNS entries."))
        .get_matches();

    CommandLine {
        addresses: match matches.subcommand_matches("update") {
            Some(update_matches) => parse_ip_parameters(update_matches),
            _ => HashMap::new()
        },
        execution_mode: match matches.subcommand_name() {
            Some("update") => ExecutionMode::UPDATE,
            Some("server") => ExecutionMode::SERVER,
            _ => panic!("BUG: No or unknown sub command was passed. This should not be possible.")
        },
        config_file: get_config_file(matches.value_of("config").unwrap()),
    }
}

fn validate_ip_parameter(value: String) -> Result<(), String> {
    parse_ip_parameter(value).map(|_| ())
}

fn parse_ip_parameters(arguments: &ArgMatches) -> HashMap<String, IpAddr> {
    arguments.values_of("ip")
        .map(|values| values
            .map(|value| parse_ip_parameter(value.to_string()).unwrap())
            .collect())
        .unwrap_or_else(|| HashMap::new())
}

fn parse_ip_parameter(value: String) -> Result<(String, IpAddr), String> {
    lazy_static! {
        static ref IP_PARAM: Regex = Regex::new(r"([^=]+)=(.+)").unwrap();
    }
    match IP_PARAM.captures(&value).map(|groups| (groups[1].to_string(), groups[2].to_string())) {
        Some((name, parameter)) => match parameter.parse() {
            Ok(ip) => Ok((name, ip)),
            Err(_) => Err(format!("Expected a valid IP address but got {}.", parameter))
        },
        None => Err(format!("IP parameter must have the format [name]=[address] but got \"{}\".", value))
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