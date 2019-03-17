use std::net::IpAddr;
use pnet::datalink::{interfaces, NetworkInterface};
use config::{IpAddressInterface, IpAddressFamily};

pub fn resolve_interface(config: &IpAddressInterface) -> Option<IpAddr> {
    get_interface(&config.interface)
        .and_then(|iface| get_ip_address(&iface, &config.family))
}

fn get_interface(name: &str) -> Option<NetworkInterface> {
    interfaces().into_iter()
        .filter(|iface| iface.name == name)
        .next()
}

fn get_ip_address(iface: &NetworkInterface, family: &IpAddressFamily) -> Option<IpAddr> {
    iface.ips.iter()
        .map(|network| network.ip())
        .filter(|ip| (*family == IpAddressFamily::V4 && ip.is_ipv4() ) ||
                    (*family == IpAddressFamily::V6 && ip.is_ipv6()))
        .map(|addr| addr.clone())
        .next()
}
