use std::net::IpAddr;

use std::option::Option;

use base64::{engine::general_purpose, Engine as _};

use cidr_utils::cidr::IpCidr;

use crate::config::{FromParameterFormat, IpAddressFromParameter};

pub fn resolve_parameter(config: &IpAddressFromParameter, value: &str) -> Option<IpAddr> {
    let mut value = value;
    let decoded_value;
    if config.base64_encoded {
        if let Some(decoded) = base64_decode(value) {
            decoded_value = decoded;
            value = &decoded_value;
        } else {
            return None;
        }
    }

    match config.format {
        FromParameterFormat::IpAddress => match value.parse() {
            Ok(ip) => Some(ip),
            Err(_) => {
                warn!("Value passed for IP address parameter \"{}\" is not a valid IP address. Ignoring it.", config.parameter.clone().unwrap_or("?".to_string()));
                None
            }
        },
        FromParameterFormat::IpNetwork => match value.parse::<IpCidr>() {
            Ok(cidr) => Some(cidr.first_as_ip_addr()),
            Err(_) => {
                warn!("Value passed for IP address parameter \"{}\" is not a valid CIDR IP network. Ignoring it.", config.parameter.clone().unwrap_or("?".to_string()));
                None
            }
        },
    }
}

fn base64_decode(encoded: &str) -> Option<String> {
    match general_purpose::STANDARD.decode(encoded) {
        Ok(decoded) => match String::from_utf8(decoded) {
            Ok(decoded_string) => Some(decoded_string),
            Err(_) => {
                warn!("Could not base64 decode an IP address parameter.");
                None
            }
        },
        Err(_) => {
            warn!("Could not base64 decode an IP address parameter.");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use crate::{
        config::{FromParameterFormat, IpAddressFromParameter},
        resolver::resolver_parameter::resolve_parameter,
    };

    #[test]
    fn ip_v4_address_resolved() {
        let actual = resolve_parameter(
            &IpAddressFromParameter {
                parameter: None,
                base64_encoded: false,
                format: FromParameterFormat::IpAddress,
            },
            "11.22.33.44",
        );

        assert_eq!(actual, Some(IpAddr::V4(Ipv4Addr::new(11, 22, 33, 44))));
    }

    #[test]
    fn ip_v6_address_resolved() {
        let actual = resolve_parameter(
            &IpAddressFromParameter {
                parameter: None,
                base64_encoded: false,
                format: FromParameterFormat::IpAddress,
            },
            "2001:db8:123:abcd::1",
        );

        assert_eq!(
            actual,
            Some(IpAddr::V6(Ipv6Addr::new(
                0x2001, 0x0db8, 0x0123, 0xabcd, 0x0000, 0x0000, 0x0000, 0x0001
            )))
        );
    }

    #[test]
    fn ip_addr_rubbish_input_is_handled_gracefully() {
        let actual = resolve_parameter(
            &IpAddressFromParameter {
                parameter: None,
                base64_encoded: false,
                format: FromParameterFormat::IpAddress,
            },
            "not an ip address",
        );

        assert_eq!(actual, None);
    }

    #[test]
    fn ip_v4_networks_resolved() {
        let actual = resolve_parameter(
            &IpAddressFromParameter {
                parameter: None,
                base64_encoded: false,
                format: FromParameterFormat::IpNetwork,
            },
            "123.234.0.0/24",
        );
        assert_eq!(actual, Some(IpAddr::V4(Ipv4Addr::new(123, 234, 0, 0))));
    }

    #[test]
    fn ip_v6_networks_resolved() {
        let actual = resolve_parameter(
            &IpAddressFromParameter {
                parameter: None,
                base64_encoded: false,
                format: FromParameterFormat::IpNetwork,
            },
            "2001:db8:123:abcd::/56",
        );

        assert_eq!(
            actual,
            Some(IpAddr::V6(Ipv6Addr::new(
                0x2001, 0x0db8, 0x0123, 0xab00, 0x0000, 0x0000, 0x0000, 0x0000
            )))
        );
    }

    #[test]
    fn base64_decoded() {
        let actual = resolve_parameter(
            &IpAddressFromParameter {
                parameter: None,
                base64_encoded: true,
                format: FromParameterFormat::IpAddress,
            },
            "MjIuMzMuNDQuNTU=",
        );

        assert_eq!(actual, Some(IpAddr::V4(Ipv4Addr::new(22, 33, 44, 55))));
    }

    #[test]
    fn base64_rubbish_input_is_handled_gracefully() {
        let actual = resolve_parameter(
            &IpAddressFromParameter {
                parameter: None,
                base64_encoded: true,
                format: FromParameterFormat::IpAddress,
            },
            "rubbi~$--sh",
        );

        assert_eq!(actual, None);
    }
}
