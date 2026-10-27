//! The API token in the Keychain, read once and then kept in memory; made when there is none.
//! Blocking.

use std::sync::Mutex;

use crate::lock::MutexExt;
use crate::secrets::{Keychain, Secret, SecretStore};

const ACCOUNT: &str = "api-token";
const TOKEN_BYTES: usize = 32;

pub struct ApiToken {
    store: Box<dyn SecretStore>,
    cached: Mutex<Option<Secret>>,
}

impl Default for ApiToken {
    fn default() -> Self {
        Self::new(Box::new(Keychain::new(crate::IDENTIFIER)))
    }
}

/// 32 random bytes as 64 hex digits.
pub fn new_token() -> Result<Secret, String> {
    let mut bytes = [0u8; TOKEN_BYTES];
    security_framework::random::SecRandom::default()
        .copy_bytes(&mut bytes)
        .map_err(|e| e.to_string())?;
    Ok(Secret::new(
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>(),
    ))
}

impl ApiToken {
    pub fn new(store: Box<dyn SecretStore>) -> Self {
        Self {
            store,
            cached: Mutex::default(),
        }
    }

    /// The stored token, or a new one stored now.
    pub fn get(&self) -> Result<Secret, String> {
        let mut cached = self.cached.lock_unpoisoned();
        if let Some(token) = cached.as_ref() {
            return Ok(token.clone());
        }
        let token = match self.store.get(ACCOUNT)? {
            Some(token) if !token.expose().is_empty() => token,
            _ => {
                let token = new_token()?;
                self.store.set(ACCOUNT, &token)?;
                token
            }
        };
        *cached = Some(token.clone());
        Ok(token)
    }

    /// Replaces the token: scripts with the old one stop working.
    pub fn renew(&self) -> Result<Secret, String> {
        self.adopt(new_token()?)
    }

    /// Stores `token`, e.g. one that was in `settings.json`.
    pub fn adopt(&self, token: Secret) -> Result<Secret, String> {
        self.store.set(ACCOUNT, &token)?;
        *self.cached.lock_unpoisoned() = Some(token.clone());
        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use super::*;

    #[derive(Clone, Default)]
    struct MemoryStore {
        items: Arc<Mutex<HashMap<String, String>>>,
        failing: bool,
    }

    impl MemoryStore {
        fn with(account: &str, value: &str) -> Self {
            let store = Self::default();
            store
                .items
                .lock()
                .unwrap()
                .insert(account.into(), value.into());
            store
        }

        fn stored(&self) -> Option<String> {
            self.items.lock().unwrap().get(ACCOUNT).cloned()
        }
    }

    impl SecretStore for MemoryStore {
        fn get(&self, account: &str) -> Result<Option<Secret>, String> {
            Ok(self
                .items
                .lock()
                .unwrap()
                .get(account)
                .map(|v| Secret::new(v.as_str())))
        }

        fn set(&self, account: &str, secret: &Secret) -> Result<(), String> {
            if self.failing {
                return Err("Keychain locked".into());
            }
            self.items
                .lock()
                .unwrap()
                .insert(account.into(), secret.expose().into());
            Ok(())
        }

        fn remove(&self, account: &str) -> Result<(), String> {
            self.items.lock().unwrap().remove(account);
            Ok(())
        }
    }

    fn api_token(store: &MemoryStore) -> ApiToken {
        ApiToken::new(Box::new(store.clone()))
    }

    fn is_hex_token(token: &str) -> bool {
        token.len() == 64
            && token
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    }

    #[test]
    fn new_tokens_are_64_lowercase_hex_digits_and_differ() {
        let first = new_token().unwrap();
        let second = new_token().unwrap();

        assert!(is_hex_token(first.expose()), "{}", first.expose());
        assert!(is_hex_token(second.expose()), "{}", second.expose());
        assert_ne!(first.expose(), second.expose());
    }

    #[test]
    fn get_makes_and_stores_a_token_when_there_is_none() {
        let store = MemoryStore::default();

        let token = api_token(&store).get().unwrap();

        assert!(is_hex_token(token.expose()));
        assert_eq!(store.stored().as_deref(), Some(token.expose()));
    }

    #[test]
    fn get_returns_the_stored_token() {
        let store = MemoryStore::with(ACCOUNT, "kept-token");

        assert_eq!(api_token(&store).get().unwrap().expose(), "kept-token");
    }

    #[test]
    fn an_empty_stored_token_is_replaced() {
        let store = MemoryStore::with(ACCOUNT, "");

        let token = api_token(&store).get().unwrap();

        assert!(is_hex_token(token.expose()));
        assert_eq!(store.stored().as_deref(), Some(token.expose()));
    }

    #[test]
    fn get_keeps_returning_the_same_token() {
        let api = api_token(&MemoryStore::default());

        assert_eq!(api.get().unwrap().expose(), api.get().unwrap().expose());
    }

    #[test]
    fn adopt_stores_the_given_token_and_get_returns_it() {
        let store = MemoryStore::with(ACCOUNT, "old");
        let api = api_token(&store);

        let adopted = api.adopt(Secret::new("imported-token")).unwrap();

        assert_eq!(adopted.expose(), "imported-token");
        assert_eq!(store.stored().as_deref(), Some("imported-token"));
        assert_eq!(api.get().unwrap().expose(), "imported-token");
    }

    #[test]
    fn renew_replaces_the_token_in_the_store_and_in_memory() {
        let store = MemoryStore::with(ACCOUNT, "old");
        let api = api_token(&store);
        assert_eq!(api.get().unwrap().expose(), "old");

        let renewed = api.renew().unwrap();

        assert!(is_hex_token(renewed.expose()));
        assert_eq!(store.stored().as_deref(), Some(renewed.expose()));
        assert_eq!(api.get().unwrap().expose(), renewed.expose());
    }

    #[test]
    fn a_store_that_refuses_the_token_reports_its_error() {
        let store = MemoryStore {
            failing: true,
            ..MemoryStore::default()
        };
        let api = api_token(&store);

        assert_eq!(api.get().unwrap_err(), "Keychain locked");
        assert_eq!(api.adopt(Secret::new("x")).unwrap_err(), "Keychain locked");
        assert_eq!(api.renew().unwrap_err(), "Keychain locked");
    }
}
