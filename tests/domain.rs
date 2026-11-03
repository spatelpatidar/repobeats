use std::str::FromStr;

use repobeats::domain::{RepositoryName, RepositoryNameError};

#[test]
fn validates_repository_names() {
    let repository = RepositoryName::from_str("aprilnea/repobeats").unwrap();
    assert_eq!(repository.owner(), "aprilnea");
    assert_eq!(repository.name(), "repobeats");
    assert_eq!(repository.to_string(), "aprilnea/repobeats");

    assert_eq!(
        RepositoryName::from_str("missing-owner").unwrap_err(),
        RepositoryNameError::Format
    );
    assert!(RepositoryName::from_str("owner/repo/extra").is_err());
    assert!(RepositoryName::from_str("-owner/repo").is_err());
}

#[test]
fn repository_names_keep_the_snapshot_json_format() {
    let repository = RepositoryName::from_str("owner/repo").unwrap();
    assert_eq!(
        serde_json::to_string(&repository).unwrap(),
        r#""owner/repo""#
    );
    assert_eq!(
        serde_json::from_str::<RepositoryName>(r#""owner/repo""#).unwrap(),
        repository
    );
}
