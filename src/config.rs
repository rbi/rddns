use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{Error, ErrorKind, Read};
use std::net::IpAddr;
use std::path::Path;

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
}

impl DdnsEntry {
    pub fn template(&self) -> &String {
        match self {
            DdnsEntry::HTTP(http) => &http.url,
            DdnsEntry::FILE(file) => &file.replace,
        }
    }
}

impl Display for DdnsEntry {
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        write!(f, "{}", self.template())
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Deserialize)]
pub struct DdnsEntryHttp {
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default = "get_false")]
    pub ignore_error: bool,
}

impl Display for DdnsEntryHttp {
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        write!(f, "{}", self.url)
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
                }),
                DdnsEntry::HTTP(DdnsEntryHttp {
                    url: "https://other.org/x?y={some_static_addr}".to_string(),
                    username: None,
                    password: None,
                    ignore_error: false,
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
