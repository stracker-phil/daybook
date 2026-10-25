//! The Slack token in the Keychain, read once and then kept in memory. Blocking.

use std::sync::Mutex;

use crate::lock::MutexExt;
use crate::secrets::{Keychain, Secret, SecretStore};

const ACCOUNT: &str = "slack-token";

pub struct Tokens {
    store: Box<dyn SecretStore>,
    /// `None` until read.
    cached: Mutex<Option<Option<Secret>>>,
}

impl Default for Tokens {
    fn default() -> Self {
        Self::new(Box::new(Keychain::new(crate::IDENTIFIER)))
    }
}

impl Tokens {
    pub fn new(store: Box<dyn SecretStore>) -> Self {
        Self {
            store,
            cached: Mutex::default(),
        }
    }

    /// An unreadable Keychain entry counts as none.
    pub fn get(&self) -> Option<Secret> {
        let mut cached = self.cached.lock_unpoisoned();
        cached
            .get_or_insert_with(|| {
                self.store.get(ACCOUNT).unwrap_or_else(|e| {
                    log::warn!("Couldn't read the Slack token from the Keychain: {e}");
                    None
                })
            })
            .clone()
    }

    pub fn save(&self, token: Secret) -> Result<(), String> {
        self.store.set(ACCOUNT, &token)?;
        *self.cached.lock_unpoisoned() = Some(Some(token));
        Ok(())
    }

    pub fn remove(&self) -> Result<(), String> {
        self.store.remove(ACCOUNT)?;
        *self.cached.lock_unpoisoned() = Some(None);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use super::*;

    #[derive(Default)]
    struct MemoryStore {
        items: Mutex<HashMap<String, String>>,
        unreadable: bool,
    }

    impl SecretStore for Arc<MemoryStore> {
        fn get(&self, account: &str) -> Result<Option<Secret>, String> {
            if self.unreadable {
                return Err("locked".into());
            }
            Ok(self
                .items
                .lock()
                .unwrap()
                .get(account)
                .map(|v| Secret::new(v.as_str())))
        }

        fn set(&self, account: &str, secret: &Secret) -> Result<(), String> {
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

    fn tokens() -> Tokens {
        Tokens::new(Box::new(Arc::new(MemoryStore::default())))
    }

    #[test]
    fn there_is_no_token_until_one_is_saved() {
        assert!(tokens().get().is_none());
    }

    #[test]
    fn a_saved_token_is_returned() {
        let tokens = tokens();

        tokens.save(Secret::new("xoxp-test-token")).unwrap();

        assert_eq!(tokens.get().unwrap().expose(), "xoxp-test-token");
    }

    #[test]
    fn a_removed_token_is_gone() {
        let tokens = tokens();
        tokens.save(Secret::new("xoxp-test-token")).unwrap();

        tokens.remove().unwrap();

        assert!(tokens.get().is_none());
    }

    #[test]
    fn a_token_already_in_the_store_is_read() {
        let store = Arc::new(MemoryStore::default());
        store.set(ACCOUNT, &Secret::new("xoxp-stored")).unwrap();

        let tokens = Tokens::new(Box::new(store));

        assert_eq!(tokens.get().unwrap().expose(), "xoxp-stored");
    }

    #[test]
    fn an_unreadable_store_reads_as_no_token() {
        let store = Arc::new(MemoryStore {
            unreadable: true,
            ..Default::default()
        });

        assert!(Tokens::new(Box::new(store)).get().is_none());
    }
}
