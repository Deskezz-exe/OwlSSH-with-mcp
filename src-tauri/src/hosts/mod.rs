//! Server profiles: the model, the JSON-backed store and `~/.ssh/config` import.

pub mod ssh_config;
mod store;

pub use store::HostStore;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum AuthKind {
    #[default]
    Password,
    Key,
    Agent,
}

/// A saved server. The password is not part of it: it sits in the OS keychain
/// under the host id, `has_password` only says whether one was stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Host {
    pub id: String,
    pub name: String,
    pub hostname: String,
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub auth: AuthKind,
    #[serde(default)]
    pub key_path: Option<String>,
    #[serde(default)]
    pub has_password: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub jump_host_id: Option<String>,
    #[serde(default)]
    pub sort_order: i64,
}

/// What the host editor sends. `id: None` creates a host.
/// `password`: `None` keeps the stored one, `Some("")` removes it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostInput {
    pub id: Option<String>,
    pub name: String,
    pub hostname: String,
    pub port: u16,
    pub username: String,
    pub auth: AuthKind,
    pub key_path: Option<String>,
    pub password: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub favorite: bool,
    pub jump_host_id: Option<String>,
}

impl HostInput {
    pub fn validate(&self) -> Result<(), String> {
        if self.hostname.trim().is_empty() {
            return Err("hostname is required".into());
        }
        if self.hostname.trim().chars().any(char::is_whitespace) {
            return Err("hostname must not contain spaces".into());
        }
        if self.username.trim().is_empty() {
            return Err("username is required".into());
        }
        if self.port == 0 {
            return Err("port must be between 1 and 65535".into());
        }
        if self.auth == AuthKind::Key
            && self.key_path.as_deref().is_none_or(|p| p.trim().is_empty())
        {
            return Err("key file is required for key authentication".into());
        }
        if self.jump_host_id.is_some() && self.jump_host_id == self.id {
            return Err("a host cannot be its own jump host".into());
        }
        Ok(())
    }
}

/// Trims the tags, drops empty ones and case-insensitive duplicates, keeps the order.
pub fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for tag in tags {
        let t = tag.trim();
        if !t.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(t)) {
            out.push(t.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> HostInput {
        HostInput {
            id: None,
            name: "web".into(),
            hostname: "10.0.0.1".into(),
            port: 22,
            username: "root".into(),
            auth: AuthKind::Password,
            key_path: None,
            password: None,
            tags: vec![],
            favorite: false,
            jump_host_id: None,
        }
    }

    #[test]
    fn validates_required_fields() {
        assert!(input().validate().is_ok());
        assert!(HostInput {
            hostname: " ".into(),
            ..input()
        }
        .validate()
        .is_err());
        assert!(HostInput {
            hostname: "a b".into(),
            ..input()
        }
        .validate()
        .is_err());
        assert!(HostInput {
            username: "".into(),
            ..input()
        }
        .validate()
        .is_err());
        assert!(HostInput { port: 0, ..input() }.validate().is_err());
        assert!(HostInput {
            auth: AuthKind::Key,
            ..input()
        }
        .validate()
        .is_err());
    }

    #[test]
    fn rejects_self_jump() {
        let i = HostInput {
            id: Some("a".into()),
            jump_host_id: Some("a".into()),
            ..input()
        };
        assert!(i.validate().is_err());
    }

    #[test]
    fn normalizes_tags() {
        let tags = vec![" prod ".into(), "".into(), "Prod".into(), "db".into()];
        assert_eq!(normalize_tags(&tags), vec!["prod", "db"]);
    }
}
