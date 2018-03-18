use std::collections::HashMap;
use std::fmt::{Display, Formatter};
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

pub fn resolve_config(config: &Config, addresses: &HashMap<String, String>) -> Vec<Result<ResolvedDdnsEntry, ResolveFailed>> {
    resolve(&config.ddns_entries, &config.ip_addresses, addresses)
}

pub fn resolve(entries: &Vec<DdnsEntry>, address_defs: &HashMap<String, IpAddress>,
               address_actual: &HashMap<String, String>) -> Vec<Result<ResolvedDdnsEntry, ResolveFailed>> {
    let resolved_addresses = resolve_addresses(address_defs, address_actual);

    entries.iter()
        .map(|entry| resolve_entry(entry, &resolved_addresses))
        .collect()
}

fn resolve_entry(entry: &DdnsEntry, resolved_addresses: &HashMap<String, &String>) -> Result<ResolvedDdnsEntry, ResolveFailed> {
    let mut resolved_url = entry.url.clone();
    for (addr_key, addr_value) in resolved_addresses.iter() {
        if resolved_url.contains(addr_key) {
            resolved_url = resolved_url.replace(addr_key, addr_value);
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
                         address_actual: &'a HashMap<String, String>) -> HashMap<String, &'a String> {
    let mut resolved = HashMap::new();

    for (name, def) in address_defs {
        match match def {
            &IpAddress::Static { ref address } => Some(address),
            &IpAddress::FromParameter { ref parameter } => address_actual.get(parameter)
        } {
            Some(address) => resolved.insert(format!("{{{}}}", name), address),
            _ => None
        };
    }
    resolved
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
            address: "2001:DB8:123:beef::42".to_string()
        });
        address_defs.insert("other_ip".to_string(), IpAddress::Static {
            address: "203.0.113.25".to_string()
        });
        let mut address_values = HashMap::new();
        // ip1 should be statically resolved and not taken from the map
        address_values.insert("ip1".to_string(), "203.0.113.92".to_string());

        let expected = vec![Ok(ResolvedDdnsEntry {
            url: "http://someHost/path/2001:DB8:123:beef::42?update=203.0.113.25".to_string(),
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
        address_values.insert("ip1".to_string(), "203.0.113.39".to_string());
        address_values.insert("different_parameter".to_string(), "2001:DB8:a2f3::29".to_string());

        let expected = vec![Ok(ResolvedDdnsEntry {
            url: "http://someHost/path/203.0.113.39?update=2001:DB8:a2f3::29".to_string(),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
        }), Ok(ResolvedDdnsEntry {
            url: "http://otherHost?ip=2001:DB8:a2f3::29".to_string(),
            username: None,
            password: None,
        })];

        let actual = resolve(&some_entries(), &address_defs, &address_values);

        assert_eq!(actual, expected)
    }

    #[test]
    fn resolve_produces_failed_entry_when_no_address_def_for_placeholder_is_available() {
        let mut address_defs = HashMap::new();
        address_defs.insert("other_ip".to_string(), IpAddress::FromParameter {
            parameter: "different_parameter".to_string()
        });
        let mut address_values = HashMap::new();
        address_values.insert("different_parameter".to_string(), "2001:DB8:a2f3::29".to_string());

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
        address_values.insert("ip1".to_string(), "203.0.113.39".to_string());

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