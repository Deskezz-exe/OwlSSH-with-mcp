use serde::{Serialize, Serializer};

/// Every error a command can return. Serialized as `{ kind, message, details }`
/// so the frontend can pick a translated text by `kind`, show the message, and
/// act on `details` (for example the fingerprint of an unknown host key).
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Keychain(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Ssh(String),
    #[error("{0}")]
    Auth(String),
    #[error("password required for {username}@{hostname}")]
    NeedPassword {
        host_id: String,
        hostname: String,
        username: String,
    },
    #[error("passphrase required for {key_path}")]
    NeedPassphrase { host_id: String, key_path: String },
    #[error("unknown host key for {hostname}:{port}")]
    HostKeyUnknown {
        host_id: String,
        hostname: String,
        port: u16,
        fingerprint: String,
        algorithm: String,
    },
    #[error("host key for {hostname}:{port} has changed")]
    HostKeyChanged {
        host_id: String,
        hostname: String,
        port: u16,
        fingerprint: String,
        algorithm: String,
    },
    #[error("{0}")]
    Timeout(String),
}

impl AppError {
    pub fn kind(&self) -> &'static str {
        match self {
            AppError::NotFound(_) => "notFound",
            AppError::Invalid(_) => "invalid",
            AppError::Keychain(_) => "keychain",
            AppError::Io(_) | AppError::Json(_) => "io",
            AppError::Ssh(_) => "ssh",
            AppError::Auth(_) => "auth",
            AppError::NeedPassword { .. } => "needPassword",
            AppError::NeedPassphrase { .. } => "needPassphrase",
            AppError::HostKeyUnknown { .. } => "hostKeyUnknown",
            AppError::HostKeyChanged { .. } => "hostKeyChanged",
            AppError::Timeout(_) => "timeout",
        }
    }

    fn details(&self) -> serde_json::Value {
        use serde_json::json;
        match self {
            AppError::NeedPassword {
                host_id,
                hostname,
                username,
            } => json!({ "hostId": host_id, "hostname": hostname, "username": username }),
            AppError::NeedPassphrase { host_id, key_path } => {
                json!({ "hostId": host_id, "keyPath": key_path })
            }
            AppError::HostKeyUnknown {
                host_id,
                hostname,
                port,
                fingerprint,
                algorithm,
            }
            | AppError::HostKeyChanged {
                host_id,
                hostname,
                port,
                fingerprint,
                algorithm,
            } => json!({
                "hostId": host_id,
                "hostname": hostname,
                "port": port,
                "fingerprint": fingerprint,
                "algorithm": algorithm,
            }),
            _ => serde_json::Value::Null,
        }
    }
}

impl From<russh::Error> for AppError {
    fn from(e: russh::Error) -> Self {
        AppError::Ssh(e.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("AppError", 3)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("message", &self.to_string())?;
        s.serialize_field("details", &self.details())?;
        s.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;
