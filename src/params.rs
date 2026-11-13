//! Parsing of request parameters shared by the CLI and the HTTP server.
//!
//! Both read the same names (`theme`, `days`, `line`, ...) so a URL query and
//! the `graph_options` action input use one syntax.

use serde::de::DeserializeOwned;
use thiserror::Error;

/// Longest value echoed back in an error message.
const ECHO_LIMIT: usize = 40;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ParamError {
    #[error("missing `{0}`")]
    Missing(&'static str),
    #[error("{0}")]
    Conflict(&'static str),
    #[error("invalid `{name}` value `{value}`: {expected}")]
    Invalid {
        name: &'static str,
        value: String,
        expected: &'static str,
    },
}

/// An `Invalid` error whose echoed value is shortened, so a hostile query
/// cannot make an error message arbitrarily long.
pub fn invalid(name: &'static str, value: &str, expected: &'static str) -> ParamError {
    let value = if value.chars().count() > ECHO_LIMIT {
        format!("{}…", value.chars().take(ECHO_LIMIT).collect::<String>())
    } else {
        value.to_owned()
    };
    ParamError::Invalid {
        name,
        value,
        expected,
    }
}

/// `true`, `on`, `yes`, `1` or `false`, `off`, `no`, `0` (any case).
pub fn parse_bool(name: &'static str, value: &str) -> Result<bool, ParamError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "on" | "yes" | "1" => Ok(true),
        "false" | "off" | "no" | "0" => Ok(false),
        _ => Err(invalid(name, value, "use true or false")),
    }
}

/// Reads a kebab-case enum such as a theme or card name.
pub fn parse_enum<T: DeserializeOwned>(
    name: &'static str,
    value: &str,
    expected: &'static str,
) -> Result<T, ParamError> {
    serde_json::from_value(serde_json::Value::String(value.trim().to_owned()))
        .map_err(|_| invalid(name, value, expected))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bool_forms() {
        assert_eq!(parse_bool("area", "TRUE"), Ok(true));
        assert_eq!(parse_bool("area", " off "), Ok(false));
        assert!(parse_bool("area", "maybe").is_err());
    }

    #[test]
    fn long_values_are_shortened() {
        let ParamError::Invalid { value, .. } = invalid("x", &"a".repeat(500), "y") else {
            unreachable!()
        };
        assert_eq!(value.chars().count(), ECHO_LIMIT + 1);
    }
}
