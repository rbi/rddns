use base64::{encode_config_buf, decode};
use regex::Regex;
use std::str::from_utf8;
use std::convert::TryFrom;

#[derive(Clone, PartialEq, Debug)]
pub struct BasicAuth {
    pub username: String,
    pub password: Option<String>,
}

impl BasicAuth {
    fn decoded_to_basic_auth(decoded: &str) -> BasicAuth {
        match decoded.find(":") {
            Some(seperator) => BasicAuth {
                username: decoded[..seperator].to_owned(),
                password: Some(decoded[seperator + 1..].to_owned()),
            },
            None => {
                BasicAuth {
                    username: decoded.to_owned(),
                    password: None
                }
            }
        }
    }
}

impl TryFrom<&str> for BasicAuth {

    type Error = String;

    fn try_from(value: &str) -> Result<Self, String> {
        lazy_static! {
            static ref BASIC_HEADER: Regex = Regex::new(r"^Basic\s+([A-Za-z0-9+/=]+)$").unwrap();
        }

        match BASIC_HEADER.captures(value) {
            Some(caps) => match decode(&caps[1]) {
                Ok(decoded) => match from_utf8(&decoded) {
                    Ok(decoded) => Ok(BasicAuth::decoded_to_basic_auth(decoded)),
                    Err(err) => Err(format!("Basic auth header decoding failed: {}", err.to_string()))
                }
                Err(err) => Err(format!("Failed to base64 decode basic auth header: {}",  err.to_string()))
            },
            None => {
                Err("The value passed did not match the expected Basic auth header format.".to_owned())
            }
        }
    }
}

pub fn to_auth_header_value(username: &str, password: &str) -> String {
    let mut buffer = String::with_capacity(username.len() + password.len() + 1);
    buffer += username;
    buffer += ":";
    buffer += password;
    to_auth_header_value_no_password(&buffer)
}

pub fn to_auth_header_value_no_password(username: &str) -> String {
    let mut buffer = String::from("Basic ");
    encode_config_buf(username, base64::STANDARD, &mut buffer);
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_auth_header_value_works() {
        assert_eq!(to_auth_header_value("a_user", "the_password"), "Basic YV91c2VyOnRoZV9wYXNzd29yZA==");
        assert_eq!(to_auth_header_value("", ""), "Basic Og==");
    }

    #[test]
    fn to_auth_header_value_no_password_works() {
        assert_eq!(to_auth_header_value_no_password("my_user"), "Basic bXlfdXNlcg==");
    }

    #[test]
    fn basic_auth_from_user_name_password_string_works() {
        assert_eq!(Ok(BasicAuth {
            username: "user 1".to_string(),
            password: Some("super secret".to_string()),
        }), BasicAuth::try_from("Basic dXNlciAxOnN1cGVyIHNlY3JldA=="));
        assert_eq!(Ok(BasicAuth {
            username: "".to_string(),
            password: Some("".to_string()),
        }), BasicAuth::try_from("Basic Og=="));
    }

    #[test]
    fn basic_auth_from_user_name_only_works() {
        assert_eq!(Ok(BasicAuth {
            username: "the user".to_string(),
            password: None,
        }), BasicAuth::try_from("Basic dGhlIHVzZXI="));
    }

    #[test]
    fn basic_auth_with_wrong_input_is_error() {
        assert!(BasicAuth::try_from("").is_err());
        assert!(BasicAuth::try_from("Basic not_base_64").is_err());
        assert!(BasicAuth::try_from("abitrary string").is_err());
    }
}