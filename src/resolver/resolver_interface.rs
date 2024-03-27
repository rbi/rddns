use crate::config::{IpAddressInterface, TextMatchMode};
use ipnetwork::IpNetwork;
use pnet::datalink::{interfaces, NetworkInterface};
use regex::Regex;
use std::net::IpAddr;

pub fn resolve_interface(config: &IpAddressInterface) -> Option<IpAddr> {
    config
        .network
        .parse()
        .map_err(|_| {
            warn!(
                "The configured string \"{}\" is not a valid IP network.",
                config.network
            )
        })
        .ok()
        .and_then(|network| {
            get_interface(&config.interface, &config.match_mode)
                .and_then(|iface| get_ip_address(&iface, &network))
        })
}

fn get_interface(name: &str, match_mode: &TextMatchMode) -> Option<NetworkInterface> {
    match match_mode {
        TextMatchMode::REGEX => match Regex::new(name) {
            Ok(regex) => interfaces()
                .into_iter()
                .filter(|iface| regex.is_match(&iface.name))
                .next(),
            Err(_err) => {
                warn!("The regex \"{}\" couldn't be compiled.", name);
                None
            }
        },
        TextMatchMode::EXACT => interfaces()
            .into_iter()
            .filter(|iface| iface.name == name)
            .next(),
    }
}

fn get_ip_address(iface: &NetworkInterface, expected_network: &IpNetwork) -> Option<IpAddr> {
    iface
        .ips
        .iter()
        .map(|network| network.ip())
        .filter(|ip| expected_network.contains(*ip))
        .map(|addr| addr.clone())
        .next()
}
