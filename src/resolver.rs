use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use regex::Regex;

use config::{Config, IpAddress, DdnsEntry};

#[derive(Clone, PartialEq, Debug)]
pub struct ResolvedDdnsEntry {
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct ResolveFailed {
    pub template_url: String,
    pub message: String,
}

impl Display for ResolvedDdnsEntry {
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        write!(f, "{}", self.url)
    }
}

impl Display for ResolveFailed {
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        write!(f, "{}", self.template_url)
    }
}

pub fn resolve_config(config: &Config, addresses: &HashMap<String, IpAddr>) -> Vec<Result<ResolvedDdnsEntry, ResolveFailed>> {
    resolve(&config.ddns_entries, &config.ip_addresses, addresses)
}

pub fn resolve(entries: &Vec<DdnsEntry>, address_defs: &HashMap<String, IpAddress>,
               address_actual: &HashMap<String, IpAddr>) -> Vec<Result<ResolvedDdnsEntry, ResolveFailed>> {
    let resolved_addresses = resolve_addresses(address_defs, address_actual);

    entries.iter()
        .map(|entry| resolve_entry(entry, &resolved_addresses))
        .collect()
}

fn resolve_entry(entry: &DdnsEntry, resolved_addresses: &HashMap<&String, IpAddr>) -> Result<ResolvedDdnsEntry, ResolveFailed> {
    let mut resolved_url = entry.url.clone();
    for (addr_key, addr_value) in resolved_addresses.iter() {
        let placeholder = format!("{{{}}}", addr_key);
        if resolved_url.contains(&placeholder) {
            resolved_url = resolved_url.replace(&placeholder, &addr_value.to_string());
        }
    }

    lazy_static! {
        static ref PLACEHOLDER: Regex = Regex::new(r"\{[^\}]*\}").unwrap();
    }

    if PLACEHOLDER.is_match(&resolved_url) {
        Err(ResolveFailed {
            template_url: entry.url.clone(),
            message: "Some placeholders for IP addresses could not be resolved to actual addresses.".to_string(),
        })
    } else {
        Ok(ResolvedDdnsEntry {
            url: resolved_url,
            username: entry.username.clone(),
            password: entry.password.clone(),
        })
    }
}

fn resolve_addresses<'a>(address_defs: &'a HashMap<String, IpAddress>,
                         address_actual: &HashMap<String, IpAddr>) -> HashMap<&'a String, IpAddr> {
    let mut resolved = HashMap::new();

    for (name, def) in address_defs {
        match match def {
            &IpAddress::Static { ref address } => Some(address.clone()),
            &IpAddress::FromParameter { ref parameter } => address_actual.get(parameter).cloned(),
            &IpAddress::Derived { .. } => None
        } {
            Some(address) => resolved.insert(name, address),
            _ => None
        };
    }

    // Derived addresses need to be resolved in a second phase after all potentintal source
    // addresses have been resolved.
    for (name, def) in address_defs {
        match match def {
            &IpAddress::Derived { subnet_bits, ref host_address, ref subnet_entry } =>
                resolve_derived(resolved.get(subnet_entry), host_address, subnet_bits),
            _ => None
        } {
            Some(address) => resolved.insert(name, address),
            _ => None
        };
    }

    resolved
}

fn resolve_derived(net_address: Option<&IpAddr>, host_address: &IpAddr, subnet_bits: u8) -> Option<IpAddr> {
    net_address.and_then(|addr| match addr {
        &IpAddr::V4(ref net_addr) => {
            match host_address {
                &IpAddr::V4(ref host_addr) => resolve_derived_ipv4(net_addr, host_addr, subnet_bits),
                &IpAddr::V6(ref host_addr) => {
                    warn!("Failed to resolve a derived IP address for host_address \"{}\" and net_address \"{}\". \
                           The first is an IPv6 address and the second an IPv4 address.", host_addr, net_addr);
                    None
                }
            }
        }
        &IpAddr::V6(ref net_addr) => {
            match host_address {
                &IpAddr::V6(ref host_addr) => resolve_derived_ipv6(net_addr, host_addr, subnet_bits),
                &IpAddr::V4(ref host_addr) => {
                    warn!("Failed to resolve a derived IP address for host_address \"{}\" and net_address \"{}\". \
                           The first is an IPv4 address and the second an IPv6 address.", host_addr, net_addr);
                    None
                }
            }
        }
    })
}

fn resolve_derived_ipv4(net_address: &Ipv4Addr, host_address: &Ipv4Addr, subnet_bits: u8) -> Option<IpAddr> {
    if subnet_bits > 32 {
        warn!("Failed to resolve a derived IP address. The subnet_bits for an IPv4 address must be between 0 and 32 but was {}.", subnet_bits);
        return None;
    }

    let numbers_net = net_address.octets();
    let numbers_host = host_address.octets();
    let mut number_derived: [u8; 4] = [0; 4];
    for i in 0..4 {
        let mut shift = subnet_bits as i16 - (i * 8);
        let netmask = if shift >= 8 { 0xFF } else if shift <= 0 { 0x00 } else { 0xFF << shift };
        let hostmask = if shift >= 8 { 0x00 } else if shift <= 0 { 0xFF } else { 0xFF >> (8 - shift) };
        number_derived[i as usize] = (numbers_net[i as usize] & netmask) | (numbers_host[i as usize] & hostmask);
    }
    Some(IpAddr::V4(number_derived.into()))
}

fn resolve_derived_ipv6(net_address: &Ipv6Addr, host_address: &Ipv6Addr, subnet_bits: u8) -> Option<IpAddr> {
    if subnet_bits > 128 {
        warn!("Failed to resolve a derived IP address. The subnet_bits for an IPv6 address must be between 0 and 128 but was {}.", subnet_bits);
        return None;
    }

    let numbers_net = net_address.octets();
    let numbers_host = host_address.octets();
    let mut number_derived: [u8; 16] = [0; 16];
    for i in 0..16 {
        let mut shift = subnet_bits as i16 - (i * 8);
        let netmask = if shift >= 8 { 0xFF } else if shift <= 0 { 0x00 } else { 0xFF << shift };
        let hostmask = if shift >= 8 { 0x00 } else if shift <= 0 { 0xFF } else { 0xFF >> (8 - shift) };
        number_derived[i as usize] = (numbers_net[i as usize] & netmask) | (numbers_host[i as usize] & hostmask);
    }
    Some(IpAddr::V6(number_derived.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn some_entries() -> Vec<DdnsEntry> {
        return vec![DdnsEntry {
            url: "http://someHost/path/{ip1}?update={other_ip}".to_string(),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
        }, DdnsEntry {
            url: "http://otherHost?ip={other_ip}".to_string(),
            username: None,
            password: None,
        }];
    }

    #[test]
    fn resolve_handles_static_addresses() {
        let mut address_defs = HashMap::new();
        address_defs.insert("ip1".to_string(), IpAddress::Static {
            address: "2001:DB8:123:beef::42".parse().unwrap()
        });
        address_defs.insert("other_ip".to_string(), IpAddress::Static {
            address: "203.0.113.25".parse().unwrap()
        });
        let mut address_values = HashMap::new();
        // ip1 should be statically resolved and not taken from the map
        address_values.insert("ip1".to_string(), "203.0.113.92".parse().unwrap());

        let expected = vec![Ok(ResolvedDdnsEntry {
            url: "http://someHost/path/2001:db8:123:beef::42?update=203.0.113.25".to_string(),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
        }), Ok(ResolvedDdnsEntry {
            url: "http://otherHost?ip=203.0.113.25".to_string(),
            username: None,
            password: None,
        })];

        let actual = resolve(&some_entries(), &address_defs, &address_values);

        assert_eq!(actual, expected);
    }

    #[test]
    fn resolve_handles_parametrized_addresses() {
        let mut address_defs = HashMap::new();
        address_defs.insert("ip1".to_string(), IpAddress::FromParameter {
            parameter: "ip1".to_string()
        });
        address_defs.insert("other_ip".to_string(), IpAddress::FromParameter {
            parameter: "different_parameter".to_string()
        });
        let mut address_values = HashMap::new();
        address_values.insert("ip1".to_string(), "203.0.113.39".parse().unwrap());
        address_values.insert("different_parameter".to_string(), "2001:DB8:a2f3::29".parse().unwrap());

        let expected = vec![Ok(ResolvedDdnsEntry {
            url: "http://someHost/path/203.0.113.39?update=2001:db8:a2f3::29".to_string(),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
        }), Ok(ResolvedDdnsEntry {
            url: "http://otherHost?ip=2001:db8:a2f3::29".to_string(),
            username: None,
            password: None,
        })];

        let actual = resolve(&some_entries(), &address_defs, &address_values);

        assert_eq!(actual, expected)
    }

    #[test]
    fn resolve_handles_derived_addresses() {
        let mut address_defs = HashMap::new();
        address_defs.insert("source_ip1".to_string(), IpAddress::Static {
            address: "203.0.113.25".parse().unwrap()
        });
        address_defs.insert("source_ip2".to_string(), IpAddress::FromParameter {
            parameter: "source_ip2_parameter".to_string()
        });
        address_defs.insert("ip1".to_string(), IpAddress::Derived {
            subnet_bits: 24,
            subnet_entry: "source_ip1".to_string(),
            host_address: "0.0.0.42".parse().unwrap(),
        });
        address_defs.insert("other_ip".to_string(), IpAddress::Derived {
            subnet_bits: 56,
            subnet_entry: "source_ip2".to_string(),
            host_address: "::4bcf:78ff:feac:8bd9".parse().unwrap(),
        });

        let mut address_values = HashMap::new();
        address_values.insert("source_ip2_parameter".to_string(), "2001:DB8:a2f3:aaaa::29".parse().unwrap());

        let expected = vec![Ok(ResolvedDdnsEntry {
            url: "http://someHost/path/203.0.113.42?update=2001:db8:a2f3:aa00:4bcf:78ff:feac:8bd9".to_string(),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
        }), Ok(ResolvedDdnsEntry {
            url: "http://otherHost?ip=2001:db8:a2f3:aa00:4bcf:78ff:feac:8bd9".to_string(),
            username: None,
            password: None,
        })];

        let actual = resolve(&some_entries(), &address_defs, &address_values);

        assert_eq!(actual, expected);
    }

    #[test]
    fn resolve_handles_derived_addresses_with_maximal_possible_subnet() {
        let mut address_defs = HashMap::new();
        address_defs.insert("source_ip1".to_string(), IpAddress::Static {
            address: "203.0.113.25".parse().unwrap()
        });
        address_defs.insert("source_ip2".to_string(), IpAddress::Static {
            address: "2001:DB8:a2f3::29".parse().unwrap()
        });
        address_defs.insert("ip1".to_string(), IpAddress::Derived {
            subnet_bits: 32,
            subnet_entry: "source_ip1".to_string(),
            host_address: "0.0.0.42".parse().unwrap(),
        });
        address_defs.insert("other_ip".to_string(), IpAddress::Derived {
            subnet_bits: 128,
            subnet_entry: "source_ip2".to_string(),
            host_address: "::4bcf:78ff:feac:8bd9".parse().unwrap(),
        });

        let expected = vec![Ok(ResolvedDdnsEntry {
            url: "http://someHost/path/203.0.113.25?update=2001:db8:a2f3::29".to_string(),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
        }), Ok(ResolvedDdnsEntry {
            url: "http://otherHost?ip=2001:db8:a2f3::29".to_string(),
            username: None,
            password: None,
        })];

        let actual = resolve(&some_entries(), &address_defs, &HashMap::new());

        assert_eq!(actual, expected);
    }

    #[test]
    fn resolve_handles_derived_addresses_with_minimal_possible_subnet() {
        let mut address_defs = HashMap::new();
        address_defs.insert("source_ip1".to_string(), IpAddress::Static {
            address: "203.0.113.25".parse().unwrap()
        });
        address_defs.insert("source_ip2".to_string(), IpAddress::Static {
            address: "2001:DB8:a2f3::29".parse().unwrap()
        });
        address_defs.insert("ip1".to_string(), IpAddress::Derived {
            subnet_bits: 0,
            subnet_entry: "source_ip1".to_string(),
            host_address: "0.0.0.42".parse().unwrap(),
        });
        address_defs.insert("other_ip".to_string(), IpAddress::Derived {
            subnet_bits: 0,
            subnet_entry: "source_ip2".to_string(),
            host_address: "::4bcf:78ff:feac:8bd9".parse().unwrap(),
        });

        let expected = vec![Ok(ResolvedDdnsEntry {
            url: "http://someHost/path/0.0.0.42?update=::4bcf:78ff:feac:8bd9".to_string(),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
        }), Ok(ResolvedDdnsEntry {
            url: "http://otherHost?ip=::4bcf:78ff:feac:8bd9".to_string(),
            username: None,
            password: None,
        })];

        let actual = resolve(&some_entries(), &address_defs, &HashMap::new());

        assert_eq!(actual, expected);
    }

    #[test]
    fn resolve_does_not_fail_on_invalid_subnets() {
        let mut address_defs = HashMap::new();
        address_defs.insert("source_ip1".to_string(), IpAddress::Static {
            address: "203.0.113.25".parse().unwrap()
        });
        address_defs.insert("source_ip2".to_string(), IpAddress::Static {
            address: "2001:DB8:a2f3::29".parse().unwrap()
        });
        address_defs.insert("ip1".to_string(), IpAddress::Derived {
            subnet_bits: 64,
            subnet_entry: "source_ip1".to_string(),
            host_address: "0.0.0.42".parse().unwrap(),
        });
        address_defs.insert("other_ip".to_string(), IpAddress::Derived {
            subnet_bits: 129,
            subnet_entry: "source_ip2".to_string(),
            host_address: "::4bcf:78ff:feac:8bd9".parse().unwrap(),
        });

        let actual = resolve(&some_entries(), &address_defs, &HashMap::new());

        assert_eq!(actual.len(), 2);
        assert!(actual[0].is_err());
        assert!(actual[1].is_err());
    }

    #[test]
    fn resolve_does_not_fail_on_net_and_host_having_different_ip_versions() {
        let mut address_defs = HashMap::new();
        address_defs.insert("source_ip1".to_string(), IpAddress::Static {
            address: "2001:DB8:a2f3::29".parse().unwrap()
        });
        address_defs.insert("ip1".to_string(), IpAddress::Derived {
            subnet_bits: 24,
            subnet_entry: "source_ip1".to_string(),
            host_address: "0.0.0.42".parse().unwrap(),
        });
        address_defs.insert("other_ip".to_string(), IpAddress::Derived {
            subnet_bits: 56,
            subnet_entry: "source_ip1".to_string(),
            host_address: "::4bcf:78ff:feac:8bd9".parse().unwrap(),
        });

        let actual = resolve(&some_entries(), &address_defs, &HashMap::new());

        assert_eq!(actual.len(), 2);
        assert!(actual[0].is_err());
        let template_url = &actual[0].as_ref().unwrap_err().template_url;
        assert_eq!(template_url, "http://someHost/path/{ip1}?update={other_ip}");

        assert!(actual[1].is_ok());
    }

    #[test]
    fn resolve_produces_failed_entry_when_no_address_def_for_placeholder_is_available() {
        let mut address_defs = HashMap::new();
        address_defs.insert("other_ip".to_string(), IpAddress::FromParameter {
            parameter: "different_parameter".to_string()
        });
        let mut address_values = HashMap::new();
        address_values.insert("different_parameter".to_string(), "2001:DB8:a2f3::29".parse().unwrap());

        let actual = resolve(&some_entries(), &address_defs, &address_values);

        assert_eq!(actual.len(), 2);
        assert!(actual[0].is_err());
        let template_url = &actual[0].as_ref().unwrap_err().template_url;
        assert_eq!(template_url, "http://someHost/path/{ip1}?update={other_ip}");
        assert!(actual[1].is_ok());
    }

    #[test]
    fn resolve_produces_failed_entry_when_no_address_for_address_def_is_available() {
        let mut address_defs = HashMap::new();
        address_defs.insert("ip1".to_string(), IpAddress::FromParameter {
            parameter: "ip1".to_string()
        });
        address_defs.insert("other_ip".to_string(), IpAddress::FromParameter {
            parameter: "different_parameter".to_string()
        });
        let mut address_values = HashMap::new();
        address_values.insert("ip1".to_string(), "203.0.113.39".parse().unwrap());

        let actual = resolve(&some_entries(), &address_defs, &address_values);

        assert_eq!(actual.len(), 2);
        assert!(actual[0].is_err());
        let template_url0 = &actual[0].as_ref().unwrap_err().template_url;
        assert_eq!(template_url0, "http://someHost/path/{ip1}?update={other_ip}");
        assert!(actual[1].is_err());
        let template_url1 = &actual[1].as_ref().unwrap_err().template_url;
        assert_eq!(template_url1, "http://otherHost?ip={other_ip}");
    }
}