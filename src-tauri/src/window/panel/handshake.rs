//! Waits per session for the overlay page to answer, or gives up after a timeout.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::oneshot;

use crate::lock::MutexExt;

pub struct Handshake<T> {
    waiters: Mutex<HashMap<u32, oneshot::Sender<T>>>,
}

impl<T> Default for Handshake<T> {
    fn default() -> Self {
        Self {
            waiters: Mutex::default(),
        }
    }
}

impl<T> Handshake<T> {
    /// Registers `session` before the page is asked, so an early answer isn't lost. Waiters of
    /// earlier sessions are dropped: sessions only count up.
    pub fn expect(&self, session: u32) -> oneshot::Receiver<T> {
        let (tx, rx) = oneshot::channel();
        let mut waiters = self.waiters.lock_unpoisoned();
        waiters.retain(|known, _| *known > session);
        waiters.insert(session, tx);
        rx
    }

    /// An answer nobody waits for (an old session) is dropped.
    pub fn answer(&self, session: u32, value: T) {
        if let Some(tx) = self.waiters.lock_unpoisoned().remove(&session) {
            let _ = tx.send(value);
        }
    }

    /// The page's answer, `None` when it didn't come within `timeout`.
    pub async fn wait(answer: oneshot::Receiver<T>, timeout: Duration) -> Option<T> {
        tokio::time::timeout(timeout, answer).await.ok()?.ok()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    const SHORT: Duration = Duration::from_millis(20);
    const LONG: Duration = Duration::from_secs(5);

    #[tokio::test]
    async fn an_answer_given_before_waiting_is_received() {
        let handshake = Handshake::default();
        let rx = handshake.expect(1);
        handshake.answer(1, 132.0);
        assert_eq!(Handshake::wait(rx, LONG).await, Some(132.0));
    }

    #[tokio::test]
    async fn no_answer_ends_the_wait_with_none_after_the_timeout() {
        let handshake = Handshake::<f64>::default();
        let rx = handshake.expect(1);
        assert_eq!(Handshake::wait(rx, SHORT).await, None);
    }

    #[tokio::test]
    async fn an_answer_for_another_session_does_not_satisfy_the_wait() {
        let handshake = Handshake::default();
        let rx = handshake.expect(2);
        handshake.answer(1, 99.0);
        assert_eq!(Handshake::wait(rx, SHORT).await, None);
    }

    #[tokio::test]
    async fn expecting_a_later_session_drops_the_waiters_of_earlier_ones() {
        let handshake = Handshake::default();
        let old = handshake.expect(1);
        let current = handshake.expect(2);
        let started = Instant::now();
        assert_eq!(Handshake::wait(old, LONG).await, None);
        assert!(started.elapsed() < Duration::from_secs(1));
        handshake.answer(2, 80.0);
        assert_eq!(Handshake::wait(current, LONG).await, Some(80.0));
    }
}
