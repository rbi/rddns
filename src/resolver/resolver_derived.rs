use crate::config::IpAddressDerived;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

pub fn resolve_derived(
    config: &IpAddressDerived,
    address_actual: &HashMap<String, IpAddr>,
) -> Option<IpAddr> {
    resolve_derived_ip(
        address_actual.get(&config.subnet_entry),
        address_actual.get(&config.host_entry),
        config.subnet_bits,
    )
}

fn resolve_derived_ip(
    net_address: Option<&IpAddr>,
    host_address: Option<&IpAddr>,
    subnet_bits: u8,
) -> Option<IpAddr> {
    if net_address.is_none() || host_address.is_none() {
        return None;
    }
    match net_address.unwrap() {
        IpAddr::V4(net_addr) => match host_address.unwrap() {
            IpAddr::V4(host_addr) => resolve_derived_ipv4(net_addr, host_addr, subnet_bits),
            IpAddr::V6(host_addr) => {
                warn!("Failed to resolve a derived IP address for host_address \"{}\" and net_address \"{}\". \
                           The first is an IPv6 address and the second an IPv4 address.", host_addr, net_addr);
                None
            }
        },
        IpAddr::V6(net_addr) => match host_address.unwrap() {
            IpAddr::V6(host_addr) => resolve_derived_ipv6(net_addr, host_addr, subnet_bits),
            IpAddr::V4(host_addr) => {
                warn!("Failed to resolve a derived IP address for host_address \"{}\" and net_address \"{}\". \
                           The first is an IPv4 address and the second an IPv6 address.", host_addr, net_addr);
                None
            }
        },
    }
}

fn resolve_derived_ipv4(
    net_address: &Ipv4Addr,
    host_address: &Ipv4Addr,
    subnet_bits: u8,
) -> Option<IpAddr> {
    if subnet_bits > 32 {
        warn!("Failed to resolve a derived IP address. The subnet_bits for an IPv4 address must be between 0 and 32 but was {}.", subnet_bits);
        return None;
    }

    let numbers_net = net_address.octets();
    let numbers_host = host_address.octets();
    let mut number_derived: [u8; 4] = [0; 4];
    for i in 0..4 {
        let shift = subnet_bits as i16 - (i * 8);
        let netmask = if shift >= 8 {
            0xFF
        } else if shift <= 0 {
            0x00
        } else {
            0xFF << shift
        };
        let hostmask = if shift >= 8 {
            0x00
        } else if shift <= 0 {
            0xFF
        } else {
            0xFF >> (8 - shift)
        };
        number_derived[i as usize] =
            (numbers_net[i as usize] & netmask) | (numbers_host[i as usize] & hostmask);
    }
    Some(IpAddr::V4(number_derived.into()))
}

fn resolve_derived_ipv6(
    net_address: &Ipv6Addr,
    host_address: &Ipv6Addr,
    subnet_bits: u8,
) -> Option<IpAddr> {
    if subnet_bits > 128 {
        warn!("Failed to resolve a derived IP address. The subnet_bits for an IPv6 address must be between 0 and 128 but was {}.", subnet_bits);
        return None;
    }

    let numbers_net = net_address.octets();
    let numbers_host = host_address.octets();
    let mut number_derived: [u8; 16] = [0; 16];
    for i in 0..16 {
        let shift = subnet_bits as i16 - (i * 8);
        let netmask = if shift >= 8 {
            0xFF
        } else if shift <= 0 {
            0x00
        } else {
            0xFF << shift
        };
        let hostmask = if shift >= 8 {
            0x00
        } else if shift <= 0 {
            0xFF
        } else {
            0xFF >> (8 - shift)
        };
        number_derived[i as usize] =
            (numbers_net[i as usize] & netmask) | (numbers_host[i as usize] & hostmask);
    }
    Some(IpAddr::V6(number_derived.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn some_addresses() -> HashMap<String, IpAddr> {
        let mut address_values = HashMap::new();
        address_values.insert("net_ip1".to_string(), "203.0.113.25".parse().unwrap());
        address_values.insert("host_ip1".to_string(), "0.0.0.42".parse().unwrap());
        address_values.insert(
            "net_ip2".to_string(),
            "2001:DB8:a2f3:aaaa::29".parse().unwrap(),
        );
        address_values.insert(
            "host_ip2".to_string(),
            "::4bcf:78ff:feac:8bd9".parse().unwrap(),
        );
        address_values
    }

    #[test]
    fn resolve_handles_derived_addresses() {
        let address_values = some_addresses();

        assert_eq!(
            Some("203.0.113.42".parse().unwrap()),
            resolve_derived(
                &IpAddressDerived {
                    subnet_bits: 24,
                    subnet_entry: "net_ip1".to_string(),
                    host_entry: "host_ip1".to_string(),
                },
                &address_values
            )
        );

        assert_eq!(
            Some("2001:db8:a2f3:aa00:4bcf:78ff:feac:8bd9".parse().unwrap()),
            resolve_derived(
                &IpAddressDerived {
                    subnet_bits: 56,
                    subnet_entry: "net_ip2".to_string(),
                    host_entry: "host_ip2".to_string(),
                },
                &address_values
            )
        );
    }

    #[test]
    fn resolve_handles_derived_addresses_with_maximal_possible_subnet() {
        let address_values = some_addresses();

        assert_eq!(
            Some("203.0.113.25".parse().unwrap()),
            resolve_derived(
                &IpAddressDerived {
                    subnet_bits: 32,
                    subnet_entry: "net_ip1".to_string(),
                    host_entry: "host_ip1".to_string(),
                },
                &address_values
            )
        );

        assert_eq!(
            Some("2001:db8:a2f3:aaaa::29".parse().unwrap()),
            resolve_derived(
                &IpAddressDerived {
                    subnet_bits: 128,
                    subnet_entry: "net_ip2".to_string(),
                    host_entry: "host_ip2".to_string(),
                },
                &address_values
            )
        );
    }

    #[test]
    fn resolve_handles_derived_addresses_with_minimal_possible_subnet() {
        let address_values = some_addresses();

        assert_eq!(
            Some("0.0.0.42".parse().unwrap()),
            resolve_derived(
                &IpAddressDerived {
                    subnet_bits: 0,
                    subnet_entry: "net_ip1".to_string(),
                    host_entry: "host_ip1".to_string(),
                },
                &address_values
            )
        );

        assert_eq!(
            Some("::4bcf:78ff:feac:8bd9".parse().unwrap()),
            resolve_derived(
                &IpAddressDerived {
                    subnet_bits: 0,
                    subnet_entry: "net_ip2".to_string(),
                    host_entry: "host_ip2".to_string(),
                },
                &address_values
            )
        );
    }

    #[test]
    fn resolve_does_not_fail_on_invalid_subnets() {
        let address_values = some_addresses();

        assert_eq!(
            None,
            resolve_derived(
                &IpAddressDerived {
                    subnet_bits: 64,
                    subnet_entry: "net_ip1".to_string(),
                    host_entry: "host_ip1".to_string(),
                },
                &address_values
            )
        );

        assert_eq!(
            None,
            resolve_derived(
                &IpAddressDerived {
                    subnet_bits: 129,
                    subnet_entry: "net_ip2".to_string(),
                    host_entry: "host_ip2".to_string(),
                },
                &address_values
            )
        );
    }

    #[test]
    fn resolve_does_not_fail_on_net_and_host_having_different_ip_versions() {
        let address_values = some_addresses();

        assert_eq!(
            None,
            resolve_derived(
                &IpAddressDerived {
                    subnet_bits: 24,
                    subnet_entry: "net_ip1".to_string(),
                    host_entry: "host_ip2".to_string(),
                },
                &address_values
            )
        );

        assert_eq!(
            None,
            resolve_derived(
                &IpAddressDerived {
                    subnet_bits: 24,
                    subnet_entry: "net_ip2".to_string(),
                    host_entry: "host_ip1".to_string(),
                },
                &address_values
            )
        );
    }
}
