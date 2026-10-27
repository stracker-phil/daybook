//! The local HTTP API (docs/automation.md, its ADR): `GET|POST http://127.0.0.1:<port>/v1/<action>`
//! with the token, running the automation actions. One request per connection; only local tools may
//! use it. Off unless Settings → Automation turns it on; restarts when its settings change.

pub mod request;
pub mod token;

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use ts_rs::TS;

use crate::automation::{self, Action, AutomationError, Outcome};
use crate::error::{AppError, AppResult};
use crate::lock::MutexExt;
use crate::secrets::Secret;
use crate::stage;
use crate::state::AppState;
use request::{Head, MAX_HEAD_BYTES, check, head_end, params, parse_head, response, stage_verb};
use token::ApiToken;

const READ_TIMEOUT: Duration = Duration::from_secs(10);
const ACCEPT_BACKOFF: Duration = Duration::from_millis(100);
const BIND_RETRIES: u32 = 20;
const BIND_RETRY_PAUSE: Duration = Duration::from_millis(25);

#[derive(Default)]
struct Server {
    task: Option<tauri::async_runtime::JoinHandle<()>>,
    /// Why it doesn't listen although it is on.
    error: Option<String>,
}

#[derive(Default)]
pub struct HttpApi {
    token: ApiToken,
    server: Mutex<Server>,
}

fn http_api(app: &AppHandle) -> &HttpApi {
    app.state::<HttpApi>().inner()
}

async fn blocking<T: Send + 'static>(
    app: &AppHandle,
    work: impl FnOnce(&HttpApi) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || work(http_api(&app)))
        .await
        .map_err(|e| e.to_string())?
}

/// Moves a token from `settings.json` to the Keychain, then starts the server when it is on.
pub fn install(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        move_token_to_keychain(&app).await;
        restart(&app);
    });
}

/// A token imported from the prototype is stored in the Keychain and then cleared from the file;
/// when the Keychain refuses, it stays in the file for the next start.
async fn move_token_to_keychain(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut settings = state.settings.get();
    if settings.api.token.is_empty() {
        return;
    }
    let token = Secret::new(std::mem::take(&mut settings.api.token));
    if let Err(e) = blocking(app, move |api| api.token.adopt(token)).await {
        log::warn!("Couldn't move the API token to the Keychain: {e}");
        return;
    }
    match state.settings.update(settings).await {
        Ok(_) => log::info!("Moved the API token from the settings to the Keychain"),
        Err(e) => log::warn!("Couldn't clear the API token from the settings: {e}"),
    }
}

/// An aborted server releases its port a moment later, so a restart retries briefly.
fn bind(port: u16, restarting: bool) -> std::io::Result<std::net::TcpListener> {
    let mut attempt = 0;
    loop {
        let bound = std::net::TcpListener::bind(("127.0.0.1", port))
            .and_then(|l| l.set_nonblocking(true).map(|()| l));
        match bound {
            Err(e)
                if restarting
                    && e.kind() == std::io::ErrorKind::AddrInUse
                    && attempt < BIND_RETRIES =>
            {
                attempt += 1;
                std::thread::sleep(BIND_RETRY_PAUSE);
            }
            other => return other,
        }
    }
}

fn bind_error(port: u16, error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::AddrInUse => format!("Port {port} is already used by another app."),
        _ => error.to_string(),
    }
}

/// Stops the server and starts it again with the saved settings, unless it is off.
pub fn restart(app: &AppHandle) {
    let settings = app.state::<AppState>().settings.get().api;
    let mut server = http_api(app).server.lock_unpoisoned();
    stage::unpublish(app);
    let restarting = server.task.take().inspect(|task| task.abort()).is_some();
    server.error = None;
    if !settings.enabled {
        if restarting {
            log::info!("HTTP API stopped");
        }
        return;
    }
    match bind(settings.port, restarting) {
        Ok(listener) => {
            log::info!("HTTP API listening on 127.0.0.1:{}", settings.port);
            server.task = Some(tauri::async_runtime::spawn(serve(
                app.clone(),
                listener,
                settings.port,
            )));
            if settings.presentations {
                stage::publish(app, settings.port);
            }
        }
        Err(e) => {
            let error = bind_error(settings.port, &e);
            log::warn!("HTTP API not started: {error}");
            server.error = Some(error);
        }
    }
}

async fn serve(app: AppHandle, listener: std::net::TcpListener, port: u16) {
    let listener = match TcpListener::from_std(listener) {
        Ok(listener) => listener,
        Err(e) => {
            log::error!("Cannot serve the HTTP API: {e}");
            return;
        }
    };
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(handle(app.clone(), stream, port));
            }
            Err(e) => {
                log::warn!("HTTP API accept failed: {e}");
                // Errors like EMFILE persist for a moment; retrying at once would spin.
                tokio::time::sleep(ACCEPT_BACKOFF).await;
            }
        }
    }
}

async fn read(stream: &mut TcpStream, buffer: &mut Vec<u8>) -> Result<(), AutomationError> {
    let mut chunk = [0u8; 4096];
    let n = stream
        .read(&mut chunk)
        .await
        .map_err(|e| AutomationError::bad(e.to_string()))?;
    if n == 0 {
        return Err(AutomationError::bad("The request ended early."));
    }
    buffer.extend_from_slice(&chunk[..n]);
    Ok(())
}

/// The head and the bytes read after it.
async fn read_head(stream: &mut TcpStream) -> Result<(Head, Vec<u8>), AutomationError> {
    let mut buffer = Vec::with_capacity(1024);
    let end = loop {
        if let Some(end) = head_end(&buffer) {
            break end;
        }
        if buffer.len() > MAX_HEAD_BYTES {
            return Err(AutomationError::new(
                413,
                "The request headers are too large.",
            ));
        }
        read(stream, &mut buffer).await?;
    };
    Ok((parse_head(&buffer[..end])?, buffer.split_off(end)))
}

/// The body after `leftover`, as long as the head says; a client that waits is told to go on.
async fn read_body(
    stream: &mut TcpStream,
    head: &Head,
    mut body: Vec<u8>,
) -> Result<Vec<u8>, AutomationError> {
    let length = head.content_length()?;
    if body.len() < length
        && head
            .header("expect")
            .is_some_and(|v| v.eq_ignore_ascii_case("100-continue"))
    {
        let _ = stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").await;
    }
    while body.len() < length {
        read(stream, &mut body).await?;
    }
    body.truncate(length);
    Ok(body)
}

/// The token, the guards, Presentations for Stage, the body, then the action or Stage verb.
async fn answer(
    app: &AppHandle,
    stream: &mut TcpStream,
    port: u16,
) -> Result<Outcome, AutomationError> {
    let token = token(app)
        .await
        .map_err(|e| AutomationError::new(503, format!("The API token isn't available: {e}")))?;
    let read = async {
        let (head, leftover) = read_head(stream).await?;
        let route = check(&head, token.expose(), port)?;
        if stage_verb(&route).is_some() {
            stage::admit(app)?;
        }
        let body = read_body(stream, &head, leftover).await?;
        Ok::<_, AutomationError>((route, head, body))
    };
    let (route, head, body) = match tokio::time::timeout(READ_TIMEOUT, read).await {
        Ok(read) => read?,
        Err(_) => return Err(AutomationError::bad("The request took too long.")),
    };
    let params = params(&head, &body)?;
    match stage_verb(&route) {
        Some(verb) => stage::run(app, verb, &params).await,
        None => automation::run(app, &route, &params).await,
    }
}

async fn handle(app: AppHandle, mut stream: TcpStream, port: u16) {
    let result = answer(&app, &mut stream, port).await;
    if let Err(e) = &result
        && e.status >= 500
    {
        log::warn!("HTTP API request failed: {}", e.message);
    }
    let _ = stream.write_all(&response(result)).await;
    let _ = stream.shutdown().await;
}

/// The server as Settings → Automation shows it.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ApiStatus {
    pub running: bool,
    /// Why it doesn't listen although it is on.
    pub error: Option<String>,
    pub port: u16,
}

pub fn api_status(app: &AppHandle) -> ApiStatus {
    let server = http_api(app).server.lock_unpoisoned();
    ApiStatus {
        running: server.task.is_some(),
        error: server.error.clone(),
        port: app.state::<AppState>().settings.get().api.port,
    }
}

/// The discovery file carries the token, so a new one rewrites it.
fn refresh_discovery(app: &AppHandle) {
    let settings = app.state::<AppState>().settings.get().api;
    let running = http_api(app).server.lock_unpoisoned().task.is_some();
    if running && settings.presentations {
        stage::publish(app, settings.port);
    }
}

/// The token every request needs; made on first use.
pub async fn token(app: &AppHandle) -> Result<Secret, String> {
    blocking(app, |api| api.token.get()).await
}

/// Where the actions are: `<base>/<action>`.
pub fn base_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/v1")
}

/// Two requests to copy, with `<token>` standing for the token.
pub fn examples(port: u16) -> Vec<String> {
    let base = base_url(port);
    let auth = r#"curl -s -H "Authorization: Bearer <token>""#;
    vec![
        format!("{auth} {base}/{}", Action::State.name()),
        format!(
            "{auth} -d status=break -d note=Lunch {base}/{}",
            Action::Status.name()
        ),
    ]
}

/// A new token; the running server takes it at once.
#[tauri::command]
pub async fn automation_new_token(app: AppHandle) -> AppResult<String> {
    let token = blocking(&app, |api| api.token.renew())
        .await
        .map_err(AppError::Io)?;
    log::info!("Made a new API token");
    refresh_discovery(&app);
    Ok(token.expose().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_port_in_use_is_reported_as_used_by_another_app() {
        let held = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = held.local_addr().unwrap().port();

        let error = std::net::TcpListener::bind(("127.0.0.1", port)).unwrap_err();

        assert_eq!(
            bind_error(port, &error),
            format!("Port {port} is already used by another app.")
        );
    }

    #[test]
    fn other_bind_errors_show_the_systems_message() {
        let error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "no way");

        assert_eq!(bind_error(80, &error), "no way");
    }
}
