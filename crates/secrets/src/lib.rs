//! Thin wrapper around the OS credential store (macOS Keychain, Windows Credential
//! Manager, Linux Secret Service via the `keyring` crate). Connection passwords and
//! private key passphrases live here, never in the app's own SQLite metadata store.
use thiserror::Error;

const SERVICE: &str = "com.structly.app";

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("keychain error: {0}")]
    Backend(#[from] keyring::Error),
}

pub type SecretResult<T> = Result<T, SecretError>;

/// `account` should be the connection's id, so each saved connection gets its own entry.
pub fn set_secret(account: &str, secret: &str) -> SecretResult<()> {
    let entry = keyring::Entry::new(SERVICE, account)?;
    entry.set_password(secret)?;
    Ok(())
}

pub fn get_secret(account: &str) -> SecretResult<Option<String>> {
    let entry = keyring::Entry::new(SERVICE, account)?;
    match entry.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn delete_secret(account: &str) -> SecretResult<()> {
    let entry = keyring::Entry::new(SERVICE, account)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}
