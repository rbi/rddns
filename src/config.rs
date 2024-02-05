use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::collections::{BTreeMap, HashMap};
use std::fmt::{self, Display, Formatter};
use std::fs::File;
use std::io::{Error, ErrorKind, Read};
use std::marker::PhantomData;
use std::net::IpAddr;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use serde_json::json;
use stunclient::just_give_me_the_udp_socket_and_its_external_address;

#[derive(Clone, PartialEq, Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    #[serde(rename = "trigger")]
    pub triggers: Vec<Trigger>,
    #[serde(default)]
    #[serde(rename = "ddns_entry")]
    pub ddns_entries: Vec<DdnsEntry>,
    #[serde(default)]
    #[serde(rename = "ip")]
    pub ip_addresses: HashMap<String, IpAddress>,
}

#[derive(Clone, PartialEq, Debug, Deserialize)]
#[serde(tag = "type")]
pub enum Trigger {
    #[serde(rename = "http")]
    HTTP(TriggerHttp),
    #[serde(rename = "timed")]
    TIMED(TriggerTimed),
}

#[derive(Clone, PartialEq, Debug, Deserialize)]
pub struct TriggerHttp {
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default = "default_server_port")]
    pub port: u16,
}

impl Default for TriggerHttp {
    fn default() -> Self {
        TriggerHttp {
            username: None,
            password: None,
            port: default_server_port(),
        }
    }
}

#[derive(Clone, PartialEq, Debug, Deserialize)]
pub struct TriggerTimed {
    #[serde(default = "default_interval")]
    pub interval: u32,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Deserialize)]
#[serde(tag = "type")]
pub enum DdnsEntry {
    #[serde(rename = "http")]
    HTTP(DdnsEntryHttp),
    #[serde(rename = "file")]
    FILE(DdnsEntryFile),
    #[serde(rename = "cloudflare")]
    CLOUDFLARE(DdnsEntryCloudflare)
}

impl DdnsEntry {
    pub fn resolvables(&self) -> Vec<String> {
        match self {
            DdnsEntry::HTTP(http) => http.resolvables(),
            DdnsEntry::FILE(file) => file.resolvables(),
            _ => panic!("Invalid DNS Entry!"),
        }
    }

    pub fn resolve(&self, resolved: Vec<String>) -> DdnsEntry {
        match self {
            DdnsEntry::HTTP(http) => DdnsEntry::HTTP(http.resolve(resolved)),
            DdnsEntry::FILE(file) => DdnsEntry::FILE(file.resolve(resolved)),
            _ => panic!("Invalid DNS Entry!"),
        }
    }
}

impl Display for DdnsEntry {
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        match self {
            DdnsEntry::HTTP(http) => http.fmt(f),
            DdnsEntry::FILE(file) => file.fmt(f),
            DdnsEntry::CLOUDFLARE(cf) => cf.fmt(f),
        }
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Deserialize)]
pub struct DdnsEntryHttp {
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default = "get_false")]
    pub ignore_error: bool,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_server_cert_validation")]
    pub server_cert_validation: ServerCertValidation,
    #[serde(default)]
    pub method: HttpMethod,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    pub body: Option<String>,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Deserialize)]
pub struct DdnsEntryCloudflare {
    pub zone_id: String,
    pub record_id: String,
    pub record_name: String,
    pub record_type: String,
    pub record_proxied: bool,
    pub record_content: String,
    pub record_comment: String,
    #[serde(default = "default_ttl")]
    pub record_ttl: u16,
    pub api_token: String,
    #[serde(default = "get_false")]
    pub ignore_error: bool,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_server_cert_validation")]
    pub server_cert_validation: ServerCertValidation
}

impl DdnsEntryCloudflare {

    pub fn to_http(&self) -> DdnsEntryHttp {
        DdnsEntryHttp {
            url: format!("https://api.cloudflare.com/client/v4/zones/{}/dns_records/{}", self.zone_id, self.record_id),
            headers: BTreeMap::from(
                [
                    (String::from("Content-Type"), String::from("application/json")),
                    (String::from("Authorization"), format!("Bearer {}", self.api_token))
                ]
            ),
            body: Some(serde_json::to_string_pretty(&json!({ // Pretty print is required, otherwise the placeholder detection is triggered!
                "content": self.record_content.clone(),
                "name": self.record_name.clone(),
                "proxied": self.record_proxied.clone(),
                "type": self.record_type.clone(),
                "comment": self.record_comment.clone(),
                "tags": [],
                "ttl": self.record_ttl.clone()
            })).unwrap()),
            method: HttpMethod::PUT,
            ignore_error: self.ignore_error,
            server_cert_validation: self.server_cert_validation.clone(),
            password: None,
            username: None
        }
    }

}

impl Display for DdnsEntryCloudflare {
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        write!(f, "{} {}", self.zone_id, self.record_id)
    }
}

impl Display for DdnsEntryHttp {
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        write!(f, "{} {}", self.method, self.url)
    }
}

impl DdnsEntryHttp {
    fn resolvables(&self) -> Vec<String> {
        let mut size = 1; // url
        if self.body.is_some() {
            size += 1;
        }
        size += self.headers.len();

        let mut result = Vec::with_capacity(size);
        result.push(self.url.clone());
        if let Some(body) = &self.body {
            result.push(body.clone());
        }

        for (_key, value) in &self.headers {
            result.push(value.clone());
        }

        result
    }

    fn resolve(&self, resolved: Vec<String>) -> DdnsEntryHttp {
        let mut resolved = resolved.as_slice();

        let url = if let Some((first, rest)) = resolved.split_first() {
            resolved = rest;
            first.clone()
        } else {
            self.url.clone()
        };

        let body = if let Some(orig_body) = &self.body {
            if let Some((first, rest)) = resolved.split_first() {
                resolved = rest;
                Some(first.clone())
            } else {
                Some(orig_body.clone())
            }
        } else {
            None
        };

        let mut headers = BTreeMap::new();

        for (header_name, header_value) in &self.headers {
            let new_value = if let Some((first, rest)) = resolved.split_first() {
                resolved = rest;
                first.clone()
            } else {
                header_value.clone()
            };
            headers.insert(header_name.clone(), new_value);
        }

        DdnsEntryHttp {
            url: url,
            username: self.username.clone(),
            password: self.password.clone(),
            ignore_error: self.ignore_error,
            server_cert_validation: self.server_cert_validation.clone(),
            method: self.method.clone(),
            headers: headers,
            body: body,
        }
    }
}

#[derive(Clone, Default, Eq, PartialEq, Hash, Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ServerCertValidation {
    #[serde(rename = "mozilla")]
    #[default]
    MOZILLA,
    #[serde(rename = "system")]
    SYSTEM,
    #[serde(rename = "custom")]
    CUSTOM(ServerCertValidationCustom),
    #[serde(rename = "disabled")]
    DISABLED,
}

impl FromStr for ServerCertValidation {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mozilla" => Ok(ServerCertValidation::MOZILLA),
            "system" => Ok(ServerCertValidation::SYSTEM),
            "disabled" => Ok(ServerCertValidation::DISABLED),
            _ => Err(format!(
                "Cannot deserialize \"{}\" as server_cert_validation option.",
                s
            )),
        }
    }
}

// based on https://github.com/serde-rs/serde-rs.github.io/blob/7ea65bc1a4b2d8c6ed6bc7350e60d4c0f2cff454/_src/string-or-struct.md
fn deserialize_server_cert_validation<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Deserialize<'de> + FromStr<Err = String>,
    D: Deserializer<'de>,
{
    struct StringOrStruct<T>(PhantomData<fn() -> T>);

    impl<'de, T> Visitor<'de> for StringOrStruct<T>
    where
        T: Deserialize<'de> + FromStr<Err = String>,
    {
        type Value = T;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string or map")
        }

        fn visit_str<E>(self, value: &str) -> Result<T, E>
        where
            E: de::Error,
        {
            Ok(FromStr::from_str(value).unwrap())
        }

        fn visit_map<M>(self, map: M) -> Result<T, M::Error>
        where
            M: MapAccess<'de>,
        {
            Deserialize::deserialize(de::value::MapAccessDeserializer::new(map))
        }
    }

    deserializer.deserialize_any(StringOrStruct(PhantomData))
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Deserialize)]
pub struct ServerCertValidationCustom {
    pub ca: PathBuf,
}

#[derive(Clone, Default, Eq, PartialEq, Hash, Debug, Deserialize)]
pub enum HttpMethod {
    // All Methods defined in RFC 7231 plus PATCH (RFC 5789)
    #[default]
    GET,
    HEAD,
    POST,
    PUT,
    DELETE,
    CONNECT,
    OPTIONS,
    TRACE,
    PATCH
}

impl Display for HttpMethod {
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Deserialize)]
pub struct DdnsEntryFile {
    pub file: String,
    pub replace: String,
}

impl Display for DdnsEntryFile {
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        write!(f, "file: {}, replace: {} ", self.file, self.replace)
    }
}

impl DdnsEntryFile {
    fn resolvables(&self) -> Vec<String> {
        vec![self.replace.clone()]
    }

    fn resolve(&self, resolved: Vec<String>) -> DdnsEntryFile {
        DdnsEntryFile {
            file: self.file.clone(),
            replace: if let Some(first) = resolved.first() {
                first.clone()
            } else {
                self.replace.clone()
            },
        }
    }
}

#[derive(Clone, PartialEq, Debug, Deserialize)]
#[serde(tag = "type")]
pub enum IpAddress {
    #[serde(rename = "parameter")]
    FromParameter(IpAddressFromParameter),
    #[serde(rename = "static")]
    Static(IpAddressStatic),
    #[serde(rename = "derived")]
    Derived(IpAddressDerived),
    #[serde(rename = "interface")]
    Interface(IpAddressInterface),
    #[serde(rename = "stun")]
    Stun(IpAddressStun)
}

#[derive(Clone, PartialEq, Debug, Deserialize)]
pub struct IpAddressFromParameter {
    pub parameter: Option<String>,
    #[serde(default = "get_false")]
    pub base64_encoded: bool,
    #[serde(default = "default_from_parameter_format")]
    pub format: FromParameterFormat,
}

#[cfg(test)]
impl IpAddressFromParameter {
    pub fn new(parameter: String) -> Self {
        IpAddressFromParameter {
            parameter: Some(parameter),
            base64_encoded: false,
            format: FromParameterFormat::IpAddress,
        }
    }
    pub fn new_no_parameter_name() -> Self {
        IpAddressFromParameter {
            parameter: None,
            base64_encoded: false,
            format: FromParameterFormat::IpAddress,
        }
    }
}

#[derive(Clone, PartialEq, Debug, Deserialize)]
pub enum FromParameterFormat {
    IpAddress,
    IpNetwork,
}

#[derive(Clone, PartialEq, Debug, Deserialize)]
pub struct IpAddressStatic {
    pub address: IpAddr,
}

#[derive(Clone, PartialEq, Debug, Deserialize)]
pub struct IpAddressStun {
    pub stun_server: String,
    pub ipv6: bool
}

#[derive(Clone, PartialEq, Debug, Deserialize)]
pub struct IpAddressDerived {
    pub subnet_bits: u8,
    pub host_entry: String,
    pub subnet_entry: String,
}

#[derive(Clone, PartialEq, Debug, Deserialize)]
pub struct IpAddressInterface {
    pub interface: String,
    pub network: String,
}

pub fn read_config(config_file: &Path) -> Result<Config, Error> {
    let mut file = File::open(config_file)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    ::toml::from_str(&contents).map_err(|e| Error::new(ErrorKind::InvalidData, format!("{}", e)))
}

fn get_false() -> bool {
    false
}

fn default_ttl() -> u16 {
    1
}

fn default_interval() -> u32 {
    300
}

fn default_server_port() -> u16 {
    3092
}

fn default_from_parameter_format() -> FromParameterFormat {
    FromParameterFormat::IpAddress
}

#[cfg(test)]
mod tests {
    extern crate tempdir;

    use self::tempdir::TempDir;
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;

    #[test]
    fn can_read_maximal_config_file() {
        let config_file_content = br#"
[[trigger]]
type = "http"
username = "a_user"
password = "a_password"
port = 3001

[[trigger]]
type = "timed"
interval = 5153

[ip.addr1]
type = "parameter"
parameter = "addr1"

[ip.parameter_max]
type = "parameter"
parameter = "p_max"
base64_encoded = true
format = "IpNetwork"

[ip.some_static_addr]
type = "static"
address = "2001:DB8:123:abcd::1"

[ip.interfaceAddress]
type = "interface"
interface = "eth0"
network = "::/0"

[ip.calculated_address]
type = "derived"
subnet_bits = 64
subnet_entry = "addr1"
host_entry = "some_static_addr"

[[ddns_entry]]
type = "http"
url = "http://example.com/{addr1}"
username = "someUser"
password = "somePassword"
ignore_error = true
server_cert_validation = { type = "custom", ca = "./some/path/myCa.pem" }
method = "POST"
headers = { Content-Typ = "text/plain", X-My-Header = "ip={some_static_addr}" }
body = """
    line1
    someIp={interfaceAddress}
"""

[[ddns_entry]]
type = "http"
url = "https://ur.l"
server_cert_validation = "system"

[[ddns_entry]]
type = "http"
url = "https://other.org/x?y={some_static_addr}"

[[ddns_entry]]
type = "file"
file = "/etc/somewhere.conf"
replace = "myAddr={some_static_addr}"
"#;

        let (_temp_dir, config_file_path) = create_temp_file(config_file_content);

        let mut ip_addresses = HashMap::new();
        ip_addresses.insert(
            "addr1".to_string(),
            IpAddress::FromParameter(IpAddressFromParameter {
                parameter: Some("addr1".to_string()),
                base64_encoded: false,
                format: FromParameterFormat::IpAddress,
            }),
        );
        ip_addresses.insert(
            "parameter_max".to_string(),
            IpAddress::FromParameter(IpAddressFromParameter {
                parameter: Some("p_max".to_string()),
                base64_encoded: true,
                format: FromParameterFormat::IpNetwork,
            }),
        );
        ip_addresses.insert(
            "some_static_addr".to_string(),
            IpAddress::Static(IpAddressStatic {
                address: "2001:DB8:123:abcd::1".parse().unwrap(),
            }),
        );
        ip_addresses.insert(
            "interfaceAddress".to_string(),
            IpAddress::Interface(IpAddressInterface {
                interface: "eth0".parse().unwrap(),
                network: "::/0".parse().unwrap(),
            }),
        );
        ip_addresses.insert(
            "calculated_address".to_string(),
            IpAddress::Derived(IpAddressDerived {
                subnet_bits: 64,
                subnet_entry: "addr1".to_string(),
                host_entry: "some_static_addr".to_string(),
            }),
        );
        let expected = Config {
            triggers: vec![
                Trigger::HTTP(TriggerHttp {
                    username: Some("a_user".to_string()),
                    password: Some("a_password".to_string()),
                    port: 3001,
                }),
                Trigger::TIMED(TriggerTimed { interval: 5153 }),
            ],
            ip_addresses,
            ddns_entries: vec![
                DdnsEntry::HTTP(DdnsEntryHttp {
                    url: "http://example.com/{addr1}".to_string(),
                    username: Some("someUser".to_string()),
                    password: Some("somePassword".to_string()),
                    ignore_error: true,
                    server_cert_validation: ServerCertValidation::CUSTOM(
                        ServerCertValidationCustom {
                            ca: PathBuf::from("./some/path/myCa.pem"),
                        },
                    ),
                    method: HttpMethod::POST,
                    headers: BTreeMap::from([
                        ("Content-Typ".to_string(), "text/plain".to_string()),
                        (
                            "X-My-Header".to_string(),
                            "ip={some_static_addr}".to_string(),
                        ),
                    ]),
                    body: Some("    line1\n    someIp={interfaceAddress}\n".to_string()),
                }),
                DdnsEntry::HTTP(DdnsEntryHttp {
                    url: "https://ur.l".to_string(),
                    username: None,
                    password: None,
                    ignore_error: false,
                    server_cert_validation: ServerCertValidation::SYSTEM,
                    method: HttpMethod::GET,
                    headers: BTreeMap::new(),
                    body: None,
                }),
                DdnsEntry::HTTP(DdnsEntryHttp {
                    url: "https://other.org/x?y={some_static_addr}".to_string(),
                    username: None,
                    password: None,
                    ignore_error: false,
                    server_cert_validation: ServerCertValidation::MOZILLA,
                    method: HttpMethod::GET,
                    headers: BTreeMap::new(),
                    body: None,
                }),
                DdnsEntry::FILE(DdnsEntryFile {
                    file: "/etc/somewhere.conf".to_string(),
                    replace: "myAddr={some_static_addr}".to_string(),
                }),
            ],
        };
        let actual = read_config(&config_file_path)
            .expect("It should be possible to read the test config file.");

        assert_eq!(expected, actual);
    }

    #[test]
    fn can_read_minimal_config_file() {
        let config_file_content = br#""#;

        let (_temp_dir, config_file_path) = create_temp_file(config_file_content);

        let expected = Config {
            triggers: vec![],
            ip_addresses: HashMap::new(),
            ddns_entries: vec![],
        };

        let actual = read_config(&config_file_path)
            .expect("It should be possible to read the test config file.");

        assert_eq!(expected, actual);
    }

    #[test]
    fn can_read_exemplary_config_file() {
        let config_file_path = Path::new("example_config.toml");

        read_config(&config_file_path).expect("The exemplary config file should be readable.");
    }

    fn create_temp_file(content: &[u8]) -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new("rddns_config_test").unwrap();
        let temp_file_path = temp_dir.path().join("maximal_config_file");
        let mut config_file = File::create(&temp_file_path).unwrap();
        config_file.write_all(content).unwrap();
        (temp_dir, temp_file_path)
    }
}
