//! A loopback stand-in for Slack's Web API, for tests.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::api::SlackApi;
use crate::net::Http;

/// What the fake server was asked.
#[derive(Debug, Clone)]
pub struct Recorded {
    pub path: String,
    pub authorization: Option<String>,
    pub form: Vec<(String, String)>,
}

impl Recorded {
    pub fn field(&self, name: &str) -> Option<&str> {
        self.form
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Answers with the given `(status, body)` pairs in order and records every request.
pub struct FakeSlack {
    pub api: SlackApi,
    requests: Arc<Mutex<Vec<Recorded>>>,
}

impl FakeSlack {
    pub async fn start(answers: Vec<(u16, &str)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = requests.clone();
        let mut answers: VecDeque<(u16, String)> = answers
            .into_iter()
            .map(|(status, body)| (status, body.to_string()))
            .collect();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let Some(request) = read_request(&mut socket).await else {
                    continue;
                };
                log.lock().unwrap().push(request);
                let (status, body) = answers
                    .pop_front()
                    .unwrap_or((500, "no answer left".into()));
                let response = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            }
        });
        let api = SlackApi::with_base(
            Http::new(&["127.0.0.1"]),
            &format!("http://127.0.0.1:{port}/api/"),
        );
        Self { api, requests }
    }

    pub fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }

    /// The paths asked, in order (`/api/users.setPresence`).
    pub fn paths(&self) -> Vec<String> {
        self.requests().into_iter().map(|r| r.path).collect()
    }
}

async fn read_request(socket: &mut TcpStream) -> Option<Recorded> {
    let mut data = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        if let Some(at) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break at;
        }
        let read = socket.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        data.extend_from_slice(&chunk[..read]);
    };
    let head = String::from_utf8_lossy(&data[..header_end]).to_string();
    let header = |name: &str| {
        head.lines().skip(1).find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name)
                .then(|| value.trim().to_string())
        })
    };
    let length: usize = header("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    while data.len() < header_end + 4 + length {
        let read = socket.read(&mut chunk).await.ok()?;
        if read == 0 {
            break;
        }
        data.extend_from_slice(&chunk[..read]);
    }
    let body = &data[header_end + 4..];
    Some(Recorded {
        path: head.lines().next()?.split_whitespace().nth(1)?.to_string(),
        authorization: header("authorization"),
        form: url::form_urlencoded::parse(body)
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect(),
    })
}
