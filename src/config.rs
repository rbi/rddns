use std::path::Path;
use std::fs::File;
use std::io::{Error, ErrorKind, Read};
use std::fmt::{Display, Formatter};

#[derive(Clone, PartialEq, Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub ddns_entries: Vec<DdnsEntry>,
    #[serde(default)]
    pub ip_addresses: Vec<IpAddress>,
}

#[derive(Clone, PartialEq, Debug, Deserialize)]
pub struct DdnsEntry {
    pub url: String,
    pub username: String,
    pub password: String,
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
        parameter: String
    },
    #[serde(rename = "static")]
    Static {
        address: String
    },
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
        [[ip_addresses]]
        type = "parameter"
        parameter = "addr1"

        [[ip_addresses]]
        type = "static"
        address = "2001:DB8:123:abcd::1"

        [[ddns_entries]]
        url = "http://example.com"
        username = "someUser"
        password = "somePassword"

        [[ddns_entries]]
        url = "https://other.org/x?y=z"
        username = "other_user"
        password = "other_password"
        "#;

        let (_temp_dir, config_file_path) = create_temp_file(config_file_content);

        let expected = Config {
            ip_addresses: vec![
                IpAddress::FromParameter {
                    parameter: "addr1".to_string()
                },
                IpAddress::Static {
                    address: "2001:DB8:123:abcd::1".to_string()
                }
            ],

            ddns_entries: vec![
                DdnsEntry {
                    url: "http://example.com".to_string(),
                    username: "someUser".to_string(),
                    password: "somePassword".to_string(),
                },
                DdnsEntry {
                    url: "https://other.org/x?y=z".to_string(),
                    username: "other_user".to_string(),
                    password: "other_password".to_string(),
                }
            ]
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
            ip_addresses: vec![],
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