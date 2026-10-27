//! Sends a status in the background: a failure retries once after a pause, and a newer status makes an
//! older one and its retry obsolete.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use super::api::SlackError;

const RETRY_AFTER: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq)]
pub enum SendError {
    NotConnected,
    Slack(SlackError),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome<T> {
    Sent(T),
    Replaced,
    NotConnected,
    /// Also on the retry.
    Failed(SlackError),
}

pub struct Sender {
    /// Every push gets the next number; only the newest one may send.
    generation: AtomicU64,
    retry_after: Duration,
}

impl Default for Sender {
    fn default() -> Self {
        Self::new(RETRY_AFTER)
    }
}

impl Sender {
    pub fn new(retry_after: Duration) -> Self {
        Self {
            generation: AtomicU64::new(0),
            retry_after,
        }
    }

    /// Makes a pending send and its retry obsolete, for a status set elsewhere.
    pub fn supersede(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Runs `attempt`, and once more after `retry_after` when Slack fails. `on_failure` hears of each
    /// failure, with whether a retry follows.
    pub async fn push<T, F, Fut, OnFailure>(&self, attempt: F, on_failure: OnFailure) -> Outcome<T>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, SendError>>,
        OnFailure: Fn(&SlackError, bool),
    {
        let run = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let is_current = || self.generation.load(Ordering::SeqCst) == run;
        let mut retried = false;
        loop {
            if !is_current() {
                return Outcome::Replaced;
            }
            let error = match attempt().await {
                Ok(sent) => return Outcome::Sent(sent),
                Err(SendError::NotConnected) => return Outcome::NotConnected,
                Err(SendError::Slack(error)) => error,
            };
            if !is_current() {
                return Outcome::Replaced;
            }
            let retry_follows = !retried;
            on_failure(&error, retry_follows);
            if retried {
                return Outcome::Failed(error);
            }
            retried = true;
            tokio::time::sleep(self.retry_after).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use super::*;

    type Script = Mutex<VecDeque<Result<u32, SendError>>>;

    fn script(results: Vec<Result<u32, SendError>>) -> Script {
        Mutex::new(results.into())
    }

    fn next(script: &Script) -> Result<u32, SendError> {
        script
            .lock()
            .unwrap()
            .pop_front()
            .expect("a scripted result")
    }

    fn api_error(code: &str) -> SlackError {
        SlackError::Api {
            method: "users.profile.set".into(),
            code: code.into(),
        }
    }

    fn failure(code: &str) -> SendError {
        SendError::Slack(api_error(code))
    }

    fn sender() -> Sender {
        Sender::new(Duration::from_millis(10))
    }

    #[tokio::test]
    async fn a_successful_attempt_is_sent() {
        let results = script(vec![Ok(7)]);

        let outcome = sender().push(|| async { next(&results) }, |_, _| {}).await;

        assert_eq!(outcome, Outcome::Sent(7));
    }

    #[tokio::test]
    async fn a_failure_is_retried_once_and_the_retry_can_succeed() {
        let results = script(vec![Err(failure("ratelimited")), Ok(3)]);
        let flags = Mutex::new(Vec::new());

        let outcome = sender()
            .push(
                || async { next(&results) },
                |_, retrying| flags.lock().unwrap().push(retrying),
            )
            .await;

        assert_eq!(outcome, Outcome::Sent(3));
        assert_eq!(*flags.lock().unwrap(), vec![true]);
    }

    #[tokio::test]
    async fn a_failure_on_the_retry_ends_in_an_error() {
        let results = script(vec![
            Err(failure("ratelimited")),
            Err(failure("fatal_error")),
        ]);
        let heard = Mutex::new(Vec::new());

        let outcome = sender()
            .push(
                || async { next(&results) },
                |error, retrying| heard.lock().unwrap().push((error.clone(), retrying)),
            )
            .await;

        assert_eq!(outcome, Outcome::Failed(api_error("fatal_error")));
        assert_eq!(
            *heard.lock().unwrap(),
            vec![
                (api_error("ratelimited"), true),
                (api_error("fatal_error"), false)
            ]
        );
    }

    #[tokio::test]
    async fn not_connected_ends_without_a_retry_or_a_failure_report() {
        let results = script(vec![Err(SendError::NotConnected)]);
        let flags = Mutex::new(Vec::new());

        let outcome = sender()
            .push(
                || async { next(&results) },
                |_, retrying| flags.lock().unwrap().push(retrying),
            )
            .await;

        assert_eq!(outcome, Outcome::NotConnected);
        assert!(flags.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_newer_push_replaces_an_older_one_waiting_for_its_retry() {
        let sender = Sender::new(Duration::from_millis(200));
        let first_results = script(vec![Err(failure("ratelimited")), Ok(1)]);
        let second_results = script(vec![Ok(2)]);

        let first = sender.push(|| async { next(&first_results) }, |_, _| {});
        let second = async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            sender
                .push(|| async { next(&second_results) }, |_, _| {})
                .await
        };
        let (first, second) = tokio::join!(first, second);

        assert_eq!(first, Outcome::Replaced);
        assert_eq!(second, Outcome::Sent(2));
    }
}
