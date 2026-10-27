//! `daybook://<action>?<params>` links from other apps run the automation actions (docs/automation.md).
//! macOS hands a link to the running app, or starts it first; links wait until the page is ready. A
//! failure shows as a notification, never in a window.

use std::sync::Mutex;

use tauri::{AppHandle, Manager};
use tauri_plugin_deep_link::DeepLinkExt;
use url::Url;

use crate::automation::{self, Params};
use crate::lock::MutexExt;
use crate::notify;

const FAILED: &str = "Daybook couldn’t do that";

/// Links that came before the page was ready; `None` once it is.
pub struct PendingLinks(Mutex<Option<Vec<String>>>);

impl Default for PendingLinks {
    fn default() -> Self {
        Self(Mutex::new(Some(Vec::new())))
    }
}

pub fn listen(handle: &AppHandle) {
    let app = handle.clone();
    handle.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            log::info!("Received a deep link");
            let waiting = match app.state::<PendingLinks>().0.lock_unpoisoned().as_mut() {
                Some(pending) => {
                    pending.push(url.to_string());
                    true
                }
                None => false,
            };
            if !waiting {
                spawn(&app, url.to_string());
            }
        }
    });
}

/// The page listens: links that waited run now, later ones at once.
pub fn page_ready(app: &AppHandle) {
    let pending = app.state::<PendingLinks>().0.lock_unpoisoned().take();
    for url in pending.unwrap_or_default() {
        spawn(app, url);
    }
}

fn spawn(app: &AppHandle, url: String) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let (action, outcome) = match parse_link(&url) {
            Ok((action, params)) => {
                let outcome = automation::run(&app, &action, &params).await;
                (action, outcome.map_err(|e| e.message))
            }
            Err(message) => ("link".to_string(), Err(message)),
        };
        match outcome {
            Ok(_) => log::info!("Ran \"{action}\" from a link"),
            Err(message) => {
                log::warn!("A link's \"{action}\" failed: {message}");
                notify::show(&app, FAILED, &format!("{action}: {message}"));
            }
        }
    });
}

/// The action (the link's host; none: `show`) and the query's parameters, the last of a name winning.
pub fn parse_link(url: &str) -> Result<(String, Params), String> {
    let url = Url::parse(url).map_err(|e| e.to_string())?;
    if !url.path().trim_end_matches('/').is_empty() {
        return Err("Daybook has no such link".into());
    }
    let action = match url.host_str().unwrap_or_default() {
        "" => "show".to_string(),
        host => host.to_lowercase(),
    };
    Ok((action, url.query_pairs().into_owned().collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(items: &[(&str, &str)]) -> Params {
        items
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn the_host_is_the_action_and_the_query_its_parameters() {
        assert_eq!(
            parse_link("daybook://command?id=journal:open"),
            Ok(("command".to_string(), params(&[("id", "journal:open")])))
        );
        assert_eq!(
            parse_link("daybook://status?status=break&note=Lunch"),
            Ok((
                "status".to_string(),
                params(&[("status", "break"), ("note", "Lunch")])
            ))
        );
    }

    #[test]
    fn a_link_without_a_host_shows_daybook() {
        for url in ["daybook://", "daybook:///", "daybook://?x=1"] {
            let (action, _) = parse_link(url).unwrap_or_else(|e| panic!("{url}: {e}"));

            assert_eq!(action, "show", "{url}");
        }
    }

    #[test]
    fn the_action_is_lowercased() {
        assert_eq!(
            parse_link("daybook://STATE"),
            Ok(("state".to_string(), Params::new()))
        );
    }

    #[test]
    fn a_link_without_a_query_has_no_parameters() {
        assert_eq!(
            parse_link("daybook://state"),
            Ok(("state".to_string(), Params::new()))
        );
    }

    #[test]
    fn a_path_is_an_error() {
        for url in [
            "daybook://show/day",
            "daybook://command/x?id=a",
            "daybook:///show",
        ] {
            assert_eq!(
                parse_link(url),
                Err("Daybook has no such link".to_string()),
                "{url}"
            );
        }
    }

    #[test]
    fn a_trailing_slash_is_not_a_path() {
        assert_eq!(
            parse_link("daybook://show/"),
            Ok(("show".to_string(), Params::new()))
        );
    }

    #[test]
    fn the_last_of_a_repeated_parameter_wins() {
        assert_eq!(
            parse_link("daybook://status?status=break&status=off"),
            Ok(("status".to_string(), params(&[("status", "off")])))
        );
    }

    #[test]
    fn parameters_are_percent_decoded() {
        assert_eq!(
            parse_link("daybook://slack?emoji=%3Atomato%3A&text=Deep%20work+now%20%26%20then"),
            Ok((
                "slack".to_string(),
                params(&[("emoji", ":tomato:"), ("text", "Deep work now & then")])
            ))
        );
    }

    #[test]
    fn text_that_is_not_a_url_is_an_error() {
        assert!(parse_link("not a url").is_err());
        assert!(parse_link("").is_err());
    }
}
