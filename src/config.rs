use std::path::Path;
use std::fs::File;
use std::io::{Error, ErrorKind, Read};
use std::fmt::{Display, Formatter};
use std::collections::HashMap;
use std::net::IpAddr;

#[derive(Clone, PartialEq, Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: Server,
    #[serde(default)]
    #[serde(rename = "ddns_entry")]
    pub ddns_entries: Vec<DdnsEntry>,
    #[serde(default)]
    #[serde(rename = "ip_address")]
    pub ip_addresses: HashMap<String, IpAddress>,
}

#[derive(Clone, PartialEq, Debug, Deserialize)]
pub struct Server {
    pub username: Option<String>,
    pub password: Option<String>,
    pub port: Option<u16>
}

impl Default for Server {
    fn default() -> Self {
        Server {
            username: None,
            password: None,
            port: None
        }
    }
}

#[derive(Clone, PartialEq, Debug, Deserialize)]
pub struct DdnsEntry {
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl Display for DdnsEntry {
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        write!(f, "{}", self.url)
    }
}

#[derive(Clone, PartialEq, Debug, Deserialize)]
#[serde(tag = "type")]
pub enum IpAddress {
    #[serde(rename = "parameter")]
    FromParameter {
        parameter: String,
    },
    #[serde(rename = "static")]
    Static {
        address: IpAddr,
    },
    #[serde(rename = "derived")]
    Derived {
        subnet_bits: u8,
        host_address: IpAddr,
        subnet_entry: String,
    }
}

pub fn read_config(config_file: &Path) -> Result<Config, Error> {
    let mut file = File::open(config_file)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    ::toml::from_str(&contents).map_err(|e| Error::new(ErrorKind::InvalidData, format!("{}", e)))
}

#[cfg(test)]
mod tests {
    extern crate tempdir;

    use std::fs::File;
    use std::path::PathBuf;
    use std::io::Write;
    use self::tempdir::TempDir;
    use super::*;

    #[test]
    fn can_read_maximal_config_file() {
        let config_file_content = br#"
        [server]
        username = "a_user"
        password = "a_password"
        port = 3001

        [ip_address.addr1]
        type = "parameter"
        parameter = "addr1"

        [ip_address.some_static_addr]
        type = "static"
        address = "2001:DB8:123:abcd::1"

        [ip_address.calculated_address]
        type = "derived"
        subnet_bits = 64
        subnet_entry = "addr1"
        host_address = "0.0.0.42"

        [[ddns_entry]]
        url = "http://example.com/{addr1}"
        username = "someUser"
        password = "somePassword"

        [[ddns_entry]]
        url = "https://other.org/x?y={some_static_addr}"
        "#;

        let (_temp_dir, config_file_path) = create_temp_file(config_file_content);

        let mut ip_addresses = HashMap::new();
        ip_addresses.insert("addr1".to_string(), IpAddress::FromParameter {
            parameter: "addr1".to_string()
        });
        ip_addresses.insert("some_static_addr".to_string(), IpAddress::Static {
            address: "2001:DB8:123:abcd::1".parse().unwrap(),
        });
        ip_addresses.insert("calculated_address".to_string(), IpAddress::Derived {
            subnet_bits: 64,
            subnet_entry: "addr1".to_string(),
            host_address: "0.0.0.42".parse().unwrap()
        });
        let expected = Config {
            server: Server {
                username: Some("a_user".to_string()),
                password: Some("a_password".to_string()),
                port: Some(3001)
            },
            ip_addresses,
            ddns_entries: vec![
                DdnsEntry {
                    url: "http://example.com/{addr1}".to_string(),
                    username: Some("someUser".to_string()),
                    password: Some("somePassword".to_string()),
                },
                DdnsEntry {
                    url: "https://other.org/x?y={some_static_addr}".to_string(),
                    username: None,
                    password: None,
                }
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
            server: Server {
                username: None,
                password: None,
                port: None
            },
            ip_addresses: HashMap::new(),
            ddns_entries: vec![],
        };

        let actual = read_config(&config_file_path)
            .expect("It should be possible to read the test config file.");

        assert_eq!(expected, actual);
    }

    fn create_temp_file(content: &[u8]) -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new("rddns_config_test").unwrap();
        let temp_file_path = temp_dir.path().join("maximal_config_file");
        let mut config_file = File::create(&temp_file_path).unwrap();
        config_file.write_all(content).unwrap();
        (temp_dir, temp_file_path)
    }
}