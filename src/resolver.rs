use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::net::IpAddr;
use regex::Regex;

use config::{Config, IpAddress, DdnsEntry};
use resolver_derived::resolve_derived;
use resolver_interface::resolve_interface;

#[derive(Clone, PartialEq, Debug)]
pub struct ResolvedDdnsEntry {
    pub url: String,
    pub original: DdnsEntry,
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

fn resolve_entry(entry: &DdnsEntry, resolved_addresses: &HashMap<String, IpAddr>) -> Result<ResolvedDdnsEntry, ResolveFailed> {
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
            original: entry.clone(),
        })
    }
}

fn resolve_addresses<'a>(address_defs: &HashMap<String, IpAddress>,
                         address_actual: &HashMap<String, IpAddr>) -> HashMap<String, IpAddr> {
    let mut resolved = HashMap::new();

    // Derived addresses depend on other addresses to be resolved first. Therefore going through the entries multiple times
    // until no more can be resolved.
    let mut last_size = 0;
    for _i in 1..1000 {
        for (name, def) in address_defs {
            match match def {
                IpAddress::Static(val) => Some(val.address.clone()),
                IpAddress::FromParameter(val) => address_actual.
                    get(val.parameter.as_ref().unwrap_or(&name.to_string())).cloned(),
                IpAddress::Derived(val) => resolve_derived(val, &resolved),
                IpAddress::Interface(val) => resolve_interface(val)
            } {
                Some(address) => resolved.insert(name.to_string(), address),
                _ => None
            };
        }
        // If no more entries could be resolved in this round resolving can be aborted.
        if resolved.len() <= last_size {
            break;
        }
        last_size = resolved.len();
    }
    resolved
}


#[cfg(test)]
mod tests {
    use super::*;
    use config::{IpAddressDerived, IpAddressStatic, IpAddressFromParameter};

    fn some_host_entry() -> DdnsEntry {
        DdnsEntry {
            url: "http://someHost/path/{ip1}?update={other_ip}".to_string(),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
            ignore_error: true,
        }
    }

    fn other_host_entry() -> DdnsEntry {
        DdnsEntry {
            url: "http://otherHost?ip={other_ip}".to_string(),
            username: None,
            password: None,
            ignore_error: false,
        }
    }

    fn some_entries() -> Vec<DdnsEntry> {
        return vec![some_host_entry(), other_host_entry()];
    }

    #[test]
    fn resolve_handles_static_addresses() {
        let mut address_defs = HashMap::new();
        address_defs.insert("ip1".to_string(), IpAddress::Static(IpAddressStatic {
            address: "2001:DB8:123:beef::42".parse().unwrap()
        }));
        address_defs.insert("other_ip".to_string(), IpAddress::Static(IpAddressStatic {
            address: "203.0.113.25".parse().unwrap()
        }));
        let mut address_values = HashMap::new();
        // ip1 should be statically resolved and not taken from the map
        address_values.insert("ip1".to_string(), "203.0.113.92".parse().unwrap());

        let expected = vec![Ok(ResolvedDdnsEntry {
            url: "http://someHost/path/2001:db8:123:beef::42?update=203.0.113.25".to_string(),
            original: some_host_entry(),
        }), Ok(ResolvedDdnsEntry {
            url: "http://otherHost?ip=203.0.113.25".to_string(),
            original: other_host_entry(),
        })];

        let actual = resolve(&some_entries(), &address_defs, &address_values);

        assert_eq!(actual, expected);
    }

    #[test]
    fn resolve_handles_parametrized_addresses() {
        let mut address_defs = HashMap::new();
        address_defs.insert("ip1".to_string(), IpAddress::FromParameter(IpAddressFromParameter {
            parameter: None,
        }));
        address_defs.insert("other_ip".to_string(), IpAddress::FromParameter(IpAddressFromParameter {
            parameter: Some("different_parameter".to_string())
        }));
        let mut address_values = HashMap::new();
        address_values.insert("ip1".to_string(), "203.0.113.39".parse().unwrap());
        address_values.insert("different_parameter".to_string(), "2001:DB8:a2f3::29".parse().unwrap());

        let expected = vec![Ok(ResolvedDdnsEntry {
            url: "http://someHost/path/203.0.113.39?update=2001:db8:a2f3::29".to_string(),
            original: some_host_entry(),
        }), Ok(ResolvedDdnsEntry {
            url: "http://otherHost?ip=2001:db8:a2f3::29".to_string(),
            original: other_host_entry(),
        })];

        let actual = resolve(&some_entries(), &address_defs, &address_values);

        assert_eq!(actual, expected)
    }

    #[test]
    fn resolve_handles_derived_addresses_that_reference_other_derived_addresses() {
        let mut address_defs = HashMap::new();
        address_defs.insert("net_ip1".to_string(), IpAddress::Static(IpAddressStatic {
            address: "203.0.113.25".parse().unwrap()
        }));
        address_defs.insert("host_ip1".to_string(), IpAddress::Static(IpAddressStatic {
            address: "0.0.0.42".parse().unwrap()
        }));
        address_defs.insert("net_ip2".to_string(), IpAddress::Static(IpAddressStatic {
            address: "2001:DB8:a2f3:aaaa::29".parse().unwrap()
        }));
        address_defs.insert("host_ip2".to_string(), IpAddress::Static(IpAddressStatic {
            address: "::4bcf:78ff:feac:8bd9".parse().unwrap()
        }));
        address_defs.insert("subnet_ip".to_string(), IpAddress::Static(IpAddressStatic {
            address: "6666:7777:8888:9999::".parse().unwrap()
        }));
        address_defs.insert("ip1".to_string(), IpAddress::Derived(IpAddressDerived {
            subnet_bits: 24,
            subnet_entry: "net_ip1".to_string(),
            host_entry: "host_ip1".to_string(),
        }));
        address_defs.insert("other_ip".to_string(), IpAddress::Derived(IpAddressDerived {
            subnet_bits: 48,
            subnet_entry: "net_ip2".to_string(),
            host_entry: "zderived1".to_string(),
        }));
        address_defs.insert("zderived1".to_string(), IpAddress::Derived(IpAddressDerived {
            subnet_bits: 64,
            subnet_entry: "subnet_ip".to_string(),
            host_entry: "host_ip2".to_string(),
        }));

        let expected = vec![Ok(ResolvedDdnsEntry {
            url: "http://someHost/path/203.0.113.42?update=2001:db8:a2f3:9999:4bcf:78ff:feac:8bd9".to_string(),
            original: some_host_entry(),
        }), Ok(ResolvedDdnsEntry {
            url: "http://otherHost?ip=2001:db8:a2f3:9999:4bcf:78ff:feac:8bd9".to_string(),
            original: other_host_entry(),
        })];

        let actual = resolve(&some_entries(), &address_defs, &HashMap::new());

        assert_eq!(actual, expected);
    }

    #[test]
    fn resolve_produces_failed_entry_when_no_address_def_for_placeholder_is_available() {
        let mut address_defs = HashMap::new();
        address_defs.insert("other_ip".to_string(), IpAddress::FromParameter(IpAddressFromParameter {
            parameter: Some("different_parameter".to_string())
        }));
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
        address_defs.insert("ip1".to_string(), IpAddress::FromParameter(IpAddressFromParameter {
            parameter: None
        }));
        address_defs.insert("other_ip".to_string(), IpAddress::FromParameter(IpAddressFromParameter {
            parameter: Some("different_parameter".to_string())
        }));
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
