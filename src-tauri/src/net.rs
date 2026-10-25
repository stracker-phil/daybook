//! The only way out to the network (ADR 0020): form posts to a fixed list of hosts, over HTTPS, with a
//! timeout. No telemetry, no update check; the page never talks to the network.

use std::time::Duration;

use url::Url;

/// Every host Daybook talks to.
pub const HOSTS: &[&str] = &["slack.com"];
const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum NetError {
    #[error("{0} is not a host Daybook talks to")]
    NotAllowed(String),
    #[error("{0}")]
    Request(String),
    #[error("HTTP {0}")]
    Status(u16),
}

#[derive(Clone)]
pub struct Http {
    client: reqwest::Client,
    hosts: &'static [&'static str],
}

impl Http {
    pub fn new(hosts: &'static [&'static str]) -> Self {
        let client = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .user_agent(concat!("Daybook/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_else(|e| {
                log::warn!("The HTTP client fell back to its defaults: {e}");
                reqwest::Client::new()
            });
        Self { client, hosts }
    }

    /// An `https` URL on a listed host; tests may also use plain HTTP to a local server.
    pub fn allows(&self, url: &Url) -> bool {
        let Some(host) = url.host_str() else {
            return false;
        };
        let local_test_server = cfg!(test) && host == "127.0.0.1";
        self.hosts.contains(&host) && (url.scheme() == "https" || local_test_server)
    }

    /// The body of a 2xx answer.
    pub async fn post_form(
        &self,
        url: &Url,
        bearer: &str,
        form: &[(&str, &str)],
    ) -> Result<Vec<u8>, NetError> {
        if !self.allows(url) {
            return Err(NetError::NotAllowed(url.host_str().unwrap_or("").into()));
        }
        let response = self
            .client
            .post(url.clone())
            .bearer_auth(bearer)
            .form(form)
            .send()
            .await
            .map_err(transport_error)?;
        let status = response.status();
        if !status.is_success() {
            return Err(NetError::Status(status.as_u16()));
        }
        let body = response.bytes().await.map_err(transport_error)?;
        Ok(body.to_vec())
    }
}

/// Without the URL, which may carry a token in a query.
fn transport_error(e: reqwest::Error) -> NetError {
    NetError::Request(e.without_url().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed(http: &Http, url: &str) -> bool {
        http.allows(&Url::parse(url).unwrap())
    }

    #[test]
    fn only_https_on_listed_hosts_and_loopback_http_are_allowed() {
        let http = Http::new(&["slack.com", "127.0.0.1"]);
        for (url, expected) in [
            ("https://slack.com/api/auth.test", true),
            ("http://slack.com/api/auth.test", false),
            ("https://evil.com/api", false),
            ("https://evil.slack.com.example/api", false),
            ("https://slack.com.evil.com/api", false),
            ("https://api.slack.com/api", false),
            ("http://127.0.0.1:8080/api", true),
            ("http://localhost:8080/api", false),
            ("ftp://slack.com/file", false),
        ] {
            assert_eq!(allowed(&http, url), expected, "{url}");
        }
    }

    #[test]
    fn loopback_over_http_needs_to_be_listed() {
        let http = Http::new(&["slack.com"]);

        assert!(!allowed(&http, "http://127.0.0.1:8080/api"));
        assert!(allowed(&http, "https://slack.com/api"));
    }

    #[tokio::test]
    async fn posting_to_a_disallowed_url_fails_without_connecting() {
        let http = Http::new(&["slack.com"]);
        let url = Url::parse("https://evil.example/api").unwrap();

        let err = http
            .post_form(&url, "token", &[("a", "b")])
            .await
            .unwrap_err();

        assert_eq!(err, NetError::NotAllowed("evil.example".into()));
    }
}
