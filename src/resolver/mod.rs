mod resolver_derived;
mod resolver_interface;
mod resolver_parameter;
mod resolver_stun;

use regex::Regex;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::Mutex;
use crate::resolver::resolver_stun::resolve_stun;

use self::resolver_derived::resolve_derived;
use self::resolver_interface::resolve_interface;
use self::resolver_parameter::resolve_parameter;
use super::config::{Config, DdnsEntry, IpAddress};

#[derive(Clone, PartialEq, Debug)]
pub struct ResolvedDdnsEntry {
    pub resolved: DdnsEntry,
    pub original: DdnsEntry,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ResolveFailed {
    pub template: String,
    pub message: String,
    pub original: DdnsEntry,
}

impl Display for ResolvedDdnsEntry {
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        write!(f, "{}", self.resolved)
    }
}

impl Display for ResolveFailed {
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        write!(f, "{}", self.template)
    }
}

#[derive(Clone, Debug)]
pub struct Resolver {
    cache: Arc<Mutex<HashMap<String, String>>>,
}

impl Resolver {
    pub fn new() -> Self {
        Resolver {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn resolve_config(
        &self,
        config: &Config,
        addresses: &HashMap<String, String>,
    ) -> Vec<Result<ResolvedDdnsEntry, ResolveFailed>> {
        let mut cache = self.cache.lock().unwrap();
        let result = resolve(
            &config.ddns_entries,
            &config.ip_addresses,
            addresses,
            &cache,
        );

        for new_address in addresses.into_iter() {
            cache.insert(new_address.0.clone(), new_address.1.clone());
        }

        result
    }
}

fn resolve(
    entries: &Vec<DdnsEntry>,
    address_defs: &HashMap<String, IpAddress>,
    address_actual: &HashMap<String, String>,
    address_cache: &HashMap<String, String>,
) -> Vec<Result<ResolvedDdnsEntry, ResolveFailed>> {
    let resolved_addresses = resolve_addresses(address_defs, address_actual, address_cache);

    entries.clone()
        .into_iter()
        .map(|entry| resolve_entry(&entry, &resolved_addresses))
        .collect()
}

fn resolve_entry(
    entry: &DdnsEntry,
    resolved_addresses: &HashMap<String, IpAddr>,
) -> Result<ResolvedDdnsEntry, ResolveFailed> {
    let resolvables = entry.resolvables();
    let mut all_resolved = Vec::with_capacity(resolvables.len());
    for resolvable in resolvables {
        let mut resolved = resolvable.clone();
        for (addr_key, addr_value) in resolved_addresses.iter() {
            let placeholder = format!("{{{}}}", addr_key);
            if resolved.contains(&placeholder) {
                resolved = resolved.replace(&placeholder, &addr_value.to_string());
            }
        }
        lazy_static! {
            static ref PLACEHOLDER: Regex = Regex::new(r"\{[^\}\s]*\}").unwrap();
        }

        if PLACEHOLDER.is_match(&resolved) {
            return Err(ResolveFailed {
                template: resolvable,
                message:
                    "Some placeholders for IP addresses could not be resolved to actual addresses."
                        .to_string(),
                original: entry.clone(),
            });
        } else {
            all_resolved.push(resolved);
        }
    }

    Ok(ResolvedDdnsEntry {
        resolved: entry.resolve(all_resolved),
        original: entry.clone(),
    })
}

fn resolve_addresses<'a>(
    address_defs: &HashMap<String, IpAddress>,
    address_actual: &HashMap<String, String>,
    address_cache: &HashMap<String, String>,
) -> HashMap<String, IpAddr> {
    let mut resolved = HashMap::new();

    // Derived addresses depend on other addresses to be resolved first. Therefore going through the entries multiple times
    // until no more can be resolved.
    let mut last_size = 0;
    for _i in 1..1000 {
        for (name, def) in address_defs {
            match match def {
                IpAddress::Static(val) => Some(val.address.clone()),
                IpAddress::FromParameter(val) => {
                    let key = val.parameter.as_ref().unwrap_or(name);
                    address_actual
                        .get(key)
                        .or(address_cache.get(key))
                        .and_then(|value| resolve_parameter(val, value))
                }
                IpAddress::Derived(val) => resolve_derived(val, &resolved),
                IpAddress::Interface(val) => resolve_interface(val),
                IpAddress::Stun(val) => resolve_stun(val),
            } {
                Some(address) => resolved.insert(name.to_string(), address),
                _ => None,
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
    use crate::config::{
        DdnsEntryFile, DdnsEntryHttp, HttpMethod, IpAddressDerived, IpAddressFromParameter,
        IpAddressStatic, ServerCertValidation,
    };
    use std::collections::BTreeMap;

    fn some_host_entry() -> DdnsEntry {
        DdnsEntry::HTTP(DdnsEntryHttp {
            url: "http://someHost/path/{ip1}?update={other_ip}".to_string(),
            method: HttpMethod::POST,
            body: None,
            headers: BTreeMap::new(),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
            ignore_error: true,
            server_cert_validation: ServerCertValidation::MOZILLA,
        })
    }

    fn other_host_entry() -> DdnsEntry {
        DdnsEntry::HTTP(DdnsEntryHttp {
            url: "http://otherHost?ip={other_ip}".to_string(),
            method: HttpMethod::GET,
            body: None,
            headers: BTreeMap::new(),
            username: None,
            password: None,
            ignore_error: false,
            server_cert_validation: ServerCertValidation::MOZILLA,
        })
    }

    fn some_entries() -> Vec<DdnsEntry> {
        return vec![some_host_entry(), other_host_entry()];
    }

    #[test]
    fn resolve_handles_static_addresses() {
        let mut address_defs = HashMap::new();
        address_defs.insert(
            "ip1".to_string(),
            IpAddress::Static(IpAddressStatic {
                address: "2001:DB8:123:beef::42".parse().unwrap(),
            }),
        );
        address_defs.insert(
            "other_ip".to_string(),
            IpAddress::Static(IpAddressStatic {
                address: "203.0.113.25".parse().unwrap(),
            }),
        );
        let mut address_values = HashMap::new();
        // ip1 should be statically resolved and not taken from the map
        address_values.insert("ip1".to_string(), "203.0.113.92".parse().unwrap());

        let expected = vec![
            Ok(ResolvedDdnsEntry {
                resolved: DdnsEntry::HTTP(DdnsEntryHttp {
                    url: "http://someHost/path/2001:db8:123:beef::42?update=203.0.113.25"
                        .to_string(),
                    method: HttpMethod::POST,
                    body: None,
                    headers: BTreeMap::new(),
                    username: Some("user".to_string()),
                    password: Some("pass".to_string()),
                    ignore_error: true,
                    server_cert_validation: ServerCertValidation::MOZILLA,
                }),
                original: some_host_entry(),
            }),
            Ok(ResolvedDdnsEntry {
                resolved: DdnsEntry::HTTP(DdnsEntryHttp {
                    url: "http://otherHost?ip=203.0.113.25".to_string(),
                    method: HttpMethod::GET,
                    body: None,
                    headers: BTreeMap::new(),
                    username: None,
                    password: None,
                    ignore_error: false,
                    server_cert_validation: ServerCertValidation::MOZILLA,
                }),
                original: other_host_entry(),
            }),
        ];

        let actual = resolve(
            &some_entries(),
            &address_defs,
            &address_values,
            &HashMap::new(),
        );

        assert_eq!(actual, expected);
    }

    #[test]
    fn resolve_handles_parametrized_addresses() {
        let mut address_defs = HashMap::new();
        address_defs.insert(
            "ip1".to_string(),
            IpAddress::FromParameter(IpAddressFromParameter::new_no_parameter_name()),
        );
        address_defs.insert(
            "other_ip".to_string(),
            IpAddress::FromParameter(IpAddressFromParameter::new(
                "different_parameter".to_string(),
            )),
        );
        let mut address_values = HashMap::new();
        address_values.insert("ip1".to_string(), "203.0.113.39".parse().unwrap());
        address_values.insert(
            "different_parameter".to_string(),
            "2001:DB8:a2f3::29".parse().unwrap(),
        );

        let expected = vec![
            Ok(ResolvedDdnsEntry {
                resolved: DdnsEntry::HTTP(DdnsEntryHttp {
                    url: "http://someHost/path/203.0.113.39?update=2001:db8:a2f3::29".to_string(),
                    method: HttpMethod::POST,
                    body: None,
                    headers: BTreeMap::new(),
                    username: Some("user".to_string()),
                    password: Some("pass".to_string()),
                    ignore_error: true,
                    server_cert_validation: ServerCertValidation::MOZILLA,
                }),
                original: some_host_entry(),
            }),
            Ok(ResolvedDdnsEntry {
                resolved: DdnsEntry::HTTP(DdnsEntryHttp {
                    url: "http://otherHost?ip=2001:db8:a2f3::29".to_string(),
                    method: HttpMethod::GET,
                    body: None,
                    headers: BTreeMap::new(),
                    username: None,
                    password: None,
                    ignore_error: false,
                    server_cert_validation: ServerCertValidation::MOZILLA,
                }),
                original: other_host_entry(),
            }),
        ];

        let actual = resolve(
            &some_entries(),
            &address_defs,
            &address_values,
            &HashMap::new(),
        );

        assert_eq!(actual, expected)
    }

    #[test]
    fn resolve_handles_derived_addresses_that_reference_other_derived_addresses() {
        let mut address_defs = HashMap::new();
        address_defs.insert(
            "net_ip1".to_string(),
            IpAddress::Static(IpAddressStatic {
                address: "203.0.113.25".parse().unwrap(),
            }),
        );
        address_defs.insert(
            "host_ip1".to_string(),
            IpAddress::Static(IpAddressStatic {
                address: "0.0.0.42".parse().unwrap(),
            }),
        );
        address_defs.insert(
            "net_ip2".to_string(),
            IpAddress::Static(IpAddressStatic {
                address: "2001:DB8:a2f3:aaaa::29".parse().unwrap(),
            }),
        );
        address_defs.insert(
            "host_ip2".to_string(),
            IpAddress::Static(IpAddressStatic {
                address: "::4bcf:78ff:feac:8bd9".parse().unwrap(),
            }),
        );
        address_defs.insert(
            "subnet_ip".to_string(),
            IpAddress::Static(IpAddressStatic {
                address: "6666:7777:8888:9999::".parse().unwrap(),
            }),
        );
        address_defs.insert(
            "ip1".to_string(),
            IpAddress::Derived(IpAddressDerived {
                subnet_bits: 24,
                subnet_entry: "net_ip1".to_string(),
                host_entry: "host_ip1".to_string(),
            }),
        );
        address_defs.insert(
            "other_ip".to_string(),
            IpAddress::Derived(IpAddressDerived {
                subnet_bits: 48,
                subnet_entry: "net_ip2".to_string(),
                host_entry: "zderived1".to_string(),
            }),
        );
        address_defs.insert(
            "zderived1".to_string(),
            IpAddress::Derived(IpAddressDerived {
                subnet_bits: 64,
                subnet_entry: "subnet_ip".to_string(),
                host_entry: "host_ip2".to_string(),
            }),
        );

        let expected = vec![
            Ok(ResolvedDdnsEntry{
                resolved: DdnsEntry::HTTP(DdnsEntryHttp {
                    url: "http://someHost/path/203.0.113.42?update=2001:db8:a2f3:9999:4bcf:78ff:feac:8bd9"
                        .to_string(),
                    method: HttpMethod::POST,
                    body: None,
                    headers: BTreeMap::new(),
                    username: Some("user".to_string()),
                    password: Some("pass".to_string()),
                    ignore_error: true,
                    server_cert_validation: ServerCertValidation::MOZILLA,
                }),
                original: some_host_entry(),
            }),
            Ok(ResolvedDdnsEntry{
                resolved: DdnsEntry::HTTP(DdnsEntryHttp {
                    url: "http://otherHost?ip=2001:db8:a2f3:9999:4bcf:78ff:feac:8bd9".to_string(),
                    method: HttpMethod::GET,
                    body: None,
                    headers: BTreeMap::new(),
                    username: None,
                    password: None,
                    ignore_error: false,
                    server_cert_validation: ServerCertValidation::MOZILLA,
                }),
                original: other_host_entry(),
            }),
        ];

        let actual = resolve(
            &some_entries(),
            &address_defs,
            &HashMap::new(),
            &HashMap::new(),
        );

        assert_eq!(actual, expected);
    }

    #[test]
    fn resolve_produces_failed_entry_when_no_address_def_for_placeholder_is_available() {
        let mut address_defs = HashMap::new();
        address_defs.insert(
            "other_ip".to_string(),
            IpAddress::FromParameter(IpAddressFromParameter::new(
                "different_parameter".to_string(),
            )),
        );
        let mut address_values = HashMap::new();
        address_values.insert(
            "different_parameter".to_string(),
            "2001:DB8:a2f3::29".parse().unwrap(),
        );

        let actual = resolve(
            &some_entries(),
            &address_defs,
            &address_values,
            &HashMap::new(),
        );

        assert_eq!(actual.len(), 2);
        assert!(actual[0].is_err());
        let template = &actual[0].as_ref().unwrap_err().template;
        assert_eq!(template, "http://someHost/path/{ip1}?update={other_ip}");
        assert!(actual[1].is_ok());
    }

    #[test]
    fn resolve_produces_failed_entry_when_no_address_for_address_def_is_available() {
        let mut address_defs = HashMap::new();
        address_defs.insert(
            "ip1".to_string(),
            IpAddress::FromParameter(IpAddressFromParameter::new_no_parameter_name()),
        );
        address_defs.insert(
            "other_ip".to_string(),
            IpAddress::FromParameter(IpAddressFromParameter::new(
                "different_parameter".to_string(),
            )),
        );
        let mut address_values = HashMap::new();
        address_values.insert("ip1".to_string(), "203.0.113.39".parse().unwrap());

        let actual = resolve(
            &some_entries(),
            &address_defs,
            &address_values,
            &HashMap::new(),
        );

        assert_eq!(actual.len(), 2);
        assert!(actual[0].is_err());
        let template0 = &actual[0].as_ref().unwrap_err().template;
        assert_eq!(template0, "http://someHost/path/{ip1}?update={other_ip}");
        assert!(actual[1].is_err());
        let template1 = &actual[1].as_ref().unwrap_err().template;
        assert_eq!(template1, "http://otherHost?ip={other_ip}");
    }

    #[test]
    fn resolve_produces_no_error_when_fill_from_cache_is_possible() {
        let mut address_defs = HashMap::new();
        address_defs.insert(
            "ip1".to_string(),
            IpAddress::FromParameter(IpAddressFromParameter::new_no_parameter_name()),
        );
        address_defs.insert(
            "other_ip".to_string(),
            IpAddress::FromParameter(IpAddressFromParameter::new(
                "different_parameter".to_string(),
            )),
        );
        let mut address_values = HashMap::new();
        address_values.insert(
            "different_parameter".to_string(),
            "2001:db8:a2f3::29".parse().unwrap(),
        );

        let mut cache = HashMap::new();
        cache.insert("ip1".to_string(), "203.0.59.15".parse().unwrap());
        cache.insert(
            "different_parameter".to_string(),
            "2001:DB8:eeee::15".parse().unwrap(),
        ); // there is a new actual value which should take precedence over cached values.

        let expected = vec![
            Ok(ResolvedDdnsEntry {
                resolved: DdnsEntry::HTTP(DdnsEntryHttp {
                    url: "http://someHost/path/203.0.59.15?update=2001:db8:a2f3::29".to_string(),
                    method: HttpMethod::POST,
                    body: None,
                    headers: BTreeMap::new(),
                    username: Some("user".to_string()),
                    password: Some("pass".to_string()),
                    ignore_error: true,
                    server_cert_validation: ServerCertValidation::MOZILLA,
                }),
                original: some_host_entry(),
            }),
            Ok(ResolvedDdnsEntry {
                resolved: DdnsEntry::HTTP(DdnsEntryHttp {
                    url: "http://otherHost?ip=2001:db8:a2f3::29".to_string(),
                    method: HttpMethod::GET,
                    body: None,
                    headers: BTreeMap::new(),
                    username: None,
                    password: None,
                    ignore_error: false,
                    server_cert_validation: ServerCertValidation::MOZILLA,
                }),
                original: other_host_entry(),
            }),
        ];

        let actual = resolve(&some_entries(), &address_defs, &address_values, &cache);

        assert_eq!(actual, expected);
    }

    #[test]
    fn resolve_resolves_all_resolvable_ddns_entry_http_fields() {
        let mut address_defs = HashMap::new();
        address_defs.insert(
            "ip1".to_string(),
            IpAddress::Static(IpAddressStatic {
                address: "2001:DB8:123:beef::42".parse().unwrap(),
            }),
        );
        address_defs.insert(
            "other_ip".to_string(),
            IpAddress::Static(IpAddressStatic {
                address: "203.0.113.25".parse().unwrap(),
            }),
        );
        let address_values = HashMap::new();

        let input1 = DdnsEntry::HTTP(DdnsEntryHttp {
            url: "http://example.com/{ip1}".to_string(),
            username: Some("someUser".to_string()),
            password: Some("somePassword".to_string()),
            ignore_error: true,
            server_cert_validation: ServerCertValidation::MOZILLA,
            method: HttpMethod::POST,
            headers: BTreeMap::from([
                ("Content-Typ".to_string(), "text/plain".to_string()),
                ("X-My-Header".to_string(), "ip={other_ip}".to_string()),
            ]),
            body: Some("\nline1\nsomeIp={ip1}\n".to_string()),
        });
        let input2 = DdnsEntry::HTTP(DdnsEntryHttp {
            url: "https://other.org/x?y={other_ip}".to_string(),
            username: None,
            password: None,
            ignore_error: false,
            server_cert_validation: ServerCertValidation::MOZILLA,
            method: HttpMethod::GET,
            headers: BTreeMap::new(),
            body: None,
        });
        let input3 = DdnsEntry::FILE(DdnsEntryFile {
            file: "/etc/somewhere.conf".to_string(),
            replace: "myAddr={other_ip}".to_string(),
        });
        let entries = vec![input1.clone(), input2.clone(), input3.clone()];

        let actual = resolve(&entries, &address_defs, &address_values, &HashMap::new());

        assert_eq!(
            actual,
            vec![
                Ok(ResolvedDdnsEntry {
                    resolved: DdnsEntry::HTTP(DdnsEntryHttp {
                        url: "http://example.com/2001:db8:123:beef::42".to_string(),
                        username: Some("someUser".to_string()),
                        password: Some("somePassword".to_string()),
                        ignore_error: true,
                        server_cert_validation: ServerCertValidation::MOZILLA,
                        method: HttpMethod::POST,
                        headers: BTreeMap::from([
                            ("Content-Typ".to_string(), "text/plain".to_string()),
                            ("X-My-Header".to_string(), "ip=203.0.113.25".to_string(),),
                        ]),
                        body: Some("\nline1\nsomeIp=2001:db8:123:beef::42\n".to_string()),
                    }),
                    original: input1,
                }),
                Ok(ResolvedDdnsEntry {
                    resolved: DdnsEntry::HTTP(DdnsEntryHttp {
                        url: "https://other.org/x?y=203.0.113.25".to_string(),
                        username: None,
                        password: None,
                        ignore_error: false,
                        server_cert_validation: ServerCertValidation::MOZILLA,
                        method: HttpMethod::GET,
                        headers: BTreeMap::new(),
                        body: None,
                    }),
                    original: input2,
                }),
                Ok(ResolvedDdnsEntry {
                    resolved: DdnsEntry::FILE(DdnsEntryFile {
                        file: "/etc/somewhere.conf".to_string(),
                        replace: "myAddr=203.0.113.25".to_string(),
                    }),
                    original: input3,
                }),
            ]
        );
    }
}
