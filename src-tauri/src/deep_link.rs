//! `daybook://` links from other apps. macOS hands a link to the running app, or starts it first.
//! Links are queued like a launch, so they wait until the page listens. A link that fails shows as an
//! error in the window.

use tauri::AppHandle;
use tauri_plugin_deep_link::DeepLinkExt;
use url::Url;

use crate::ipc::IpcMessage;
use crate::launch;

pub fn listen(handle: &AppHandle) {
    let app = handle.clone();
    handle.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            log::info!("Received a deep link");
            launch::queue(&app, link_message(url.as_str()));
        }
    });
}

/// The message a link stands for, or an `Error` naming what is wrong with it.
pub fn link_message(url: &str) -> IpcMessage {
    parse_link(url).unwrap_or_else(|message| IpcMessage::Error {
        message: format!("A daybook:// link failed: {message}"),
    })
}

fn parse_link(url: &str) -> Result<IpcMessage, String> {
    let url = Url::parse(url).map_err(|e| e.to_string())?;
    let path = url.path().trim_end_matches('/');
    match (url.host_str().unwrap_or_default(), path) {
        ("show", "") => Ok(IpcMessage::Show { focus: true }),
        _ => Err("Daybook has no such link".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failure(reason: &str) -> IpcMessage {
        IpcMessage::Error {
            message: format!("A daybook:// link failed: {reason}"),
        }
    }

    #[test]
    fn show_links_bring_the_window_to_the_front() {
        for url in ["daybook://show", "daybook://show/", "daybook://show?x=1"] {
            assert_eq!(link_message(url), IpcMessage::Show { focus: true }, "{url}");
        }
    }

    #[test]
    fn unknown_links_become_errors_naming_the_problem() {
        for url in ["daybook://nope", "daybook://show/day"] {
            assert_eq!(
                link_message(url),
                failure("Daybook has no such link"),
                "{url}"
            );
        }
    }

    #[test]
    fn text_that_is_not_a_url_becomes_an_error() {
        for url in ["", "not a url", "show"] {
            let IpcMessage::Error { message } = link_message(url) else {
                panic!("expected an error for `{url}`");
            };
            assert!(
                message.starts_with("A daybook:// link failed: "),
                "{url}: {message}"
            );
        }
    }
}
