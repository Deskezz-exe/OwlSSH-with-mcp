//! Server passwords live only in the OS keychain (Windows Credential Manager,
//! macOS Keychain, Secret Service on Linux). They are never written to disk by
//! the app and never sent to the frontend.

use crate::error::{AppError, AppResult};

const SERVICE: &str = "dev.deskezz.owlssh";

fn entry(host_id: &str) -> AppResult<keyring::Entry> {
    keyring::Entry::new(SERVICE, host_id).map_err(|e| AppError::Keychain(e.to_string()))
}

pub fn set_password(host_id: &str, password: &str) -> AppResult<()> {
    entry(host_id)?
        .set_password(password)
        .map_err(|e| AppError::Keychain(e.to_string()))
}

pub fn delete_password(host_id: &str) -> AppResult<()> {
    match entry(host_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::Keychain(e.to_string())),
    }
}
