//! Secrets in the login Keychain: generic passwords under the app's identifier, so the debug build keeps
//! its own. Never in files, logs or the page.

use std::fmt;

use zeroize::Zeroizing;

/// A secret value; `Debug` never shows it and the memory is wiped when dropped.
#[derive(Clone)]
pub struct Secret(Zeroizing<String>);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(Zeroizing::new(value.into()))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(…)")
    }
}

pub trait SecretStore: Send + Sync {
    fn get(&self, account: &str) -> Result<Option<Secret>, String>;
    fn set(&self, account: &str, secret: &Secret) -> Result<(), String>;
    /// Removing what isn't there succeeds.
    fn remove(&self, account: &str) -> Result<(), String>;
}

const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

pub struct Keychain {
    service: &'static str,
}

impl Keychain {
    pub fn new(service: &'static str) -> Self {
        Self { service }
    }
}

impl SecretStore for Keychain {
    fn get(&self, account: &str) -> Result<Option<Secret>, String> {
        match security_framework::passwords::get_generic_password(self.service, account) {
            Ok(bytes) => String::from_utf8(bytes)
                .map(|text| Some(Secret::new(text)))
                .map_err(|_| "The stored secret is not text".into()),
            Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn set(&self, account: &str, secret: &Secret) -> Result<(), String> {
        security_framework::passwords::set_generic_password(
            self.service,
            account,
            secret.expose().as_bytes(),
        )
        .map_err(|e| e.to_string())
    }

    fn remove(&self, account: &str) -> Result<(), String> {
        match security_framework::passwords::delete_generic_password(self.service, account) {
            Err(e) if e.code() != ERR_SEC_ITEM_NOT_FOUND => Err(e.to_string()),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_hides_the_value() {
        let shown = format!("{:?}", Secret::new("xoxp-secret"));

        assert!(!shown.contains("xoxp-secret"), "{shown}");
    }

    #[test]
    fn expose_returns_the_value() {
        assert_eq!(Secret::new("xoxp-secret").expose(), "xoxp-secret");
    }
}
