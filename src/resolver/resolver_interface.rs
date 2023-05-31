use std::net::IpAddr;
use pnet::datalink::{interfaces, NetworkInterface};
use ipnetwork::IpNetwork;
use crate::config::IpAddressInterface;

pub fn resolve_interface(config: &IpAddressInterface) -> Option<IpAddr> {
    config.network.parse()
        .map_err(|_| warn!("The configured string \"{}\" is not a valid IP network.", config.network))
        .ok().and_then(|network|
                  get_interface(&config.interface)
                  .and_then(|iface| get_ip_address(&iface, &network)))
}

fn get_interface(name: &str) -> Option<NetworkInterface> {
    interfaces().into_iter()
        .filter(|iface| iface.name == name)
        .next()
}

fn get_ip_address(iface: &NetworkInterface, expected_network: &IpNetwork) -> Option<IpAddr> {
    iface.ips.iter()
        .map(|network| network.ip())
        .filter(|ip| expected_network.contains(*ip))
        .map(|addr| addr.clone())
        .next()
}
