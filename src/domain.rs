use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use thiserror::Error;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RepositoryName {
    owner: String,
    name: String,
}

impl RepositoryName {
    pub fn new(
        owner: impl Into<String>,
        name: impl Into<String>,
    ) -> Result<Self, RepositoryNameError> {
        let owner = owner.into();
        let name = name.into();
        validate_owner(&owner)?;
        validate_name(&name)?;
        Ok(Self { owner, name })
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for RepositoryName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.owner, self.name)
    }
}

impl FromStr for RepositoryName {
    type Err = RepositoryNameError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (owner, name) = value
            .trim()
            .split_once('/')
            .ok_or(RepositoryNameError::Format)?;
        Self::new(owner, name)
    }
}

impl Serialize for RepositoryName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for RepositoryName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum RepositoryNameError {
    #[error("repository must use the owner/name format")]
    Format,
    #[error("invalid GitHub repository owner")]
    Owner,
    #[error("invalid GitHub repository name")]
    Name,
}

fn validate_owner(owner: &str) -> Result<(), RepositoryNameError> {
    if owner.is_empty()
        || owner.len() > 39
        || !owner
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        || owner.starts_with('-')
        || owner.ends_with('-')
    {
        return Err(RepositoryNameError::Owner);
    }
    Ok(())
}

fn validate_name(name: &str) -> Result<(), RepositoryNameError> {
    if name.is_empty()
        || name.len() > 100
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(RepositoryNameError::Name);
    }
    Ok(())
}
