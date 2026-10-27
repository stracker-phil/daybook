//! The day's message in the channel: the focus list is edited into it, and the end-of-day list replies to
//! it, also sent to the channel.

use chrono::Local;
use tauri::{AppHandle, Manager};

use super::api::SlackError;
use super::day_message::{DayMessage, FocusTarget, message_gone, paragraphs};
use super::{in_context, logged_result, slack, token};
use crate::action_log::{self, ActionState, ActionTarget};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

fn day_message(app: &AppHandle) -> DayMessage {
    app.state::<AppState>()
        .slack
        .day_message
        .read(|day| day.of(&today()))
}

fn save(app: &AppHandle, day: DayMessage) -> AppResult<()> {
    app.state::<AppState>()
        .slack
        .day_message
        .update(|stored| *stored = day)
}

/// The channel the focus list goes to; empty when it isn't shared in Slack here.
pub fn focus_channel(app: &AppHandle) -> String {
    let settings = app.state::<AppState>().settings.get();
    if settings.focus.enabled && in_context(app) {
        settings.slack.channel.trim().to_string()
    } else {
        String::new()
    }
}

pub fn focus_target(app: &AppHandle) -> FocusTarget {
    day_message(app).target(!focus_channel(app).is_empty())
}

/// Going Off asks for the end-of-day list first, once a day.
pub fn asks_for_end_of_day_list(app: &AppHandle) -> bool {
    matches!(focus_target(app), FocusTarget::Post | FocusTarget::Edit)
}

fn failed(e: SlackError) -> AppError {
    AppError::Network(format!("Slack: {e}"))
}

fn not_connected() -> AppError {
    AppError::NotFound("Slack isn't connected (Settings → Slack).".into())
}

/// Puts `list` below the day's message. Without a message from today the list is posted and becomes
/// the day's message; a remembered one that can't be edited any more fails with a hint.
pub async fn update_day_message(app: &AppHandle, list: &str) -> AppResult<()> {
    let channel = focus_channel(app);
    if channel.is_empty() {
        return Err(AppError::InvalidInput(
            "The focus list isn't shared in Slack in this context.".into(),
        ));
    }
    let day = day_message(app);
    let text = paragraphs(&[&day.greeting, list]);
    if text.is_empty() {
        return Ok(());
    }
    let described = match day.anchor() {
        Some(_) => format!("{channel}: focus list in the day’s message"),
        None => format!("{channel}: focus list"),
    };
    let logged = action_log::sending(ActionTarget::SlackMessage, &described);
    let Some(secret) = token(app).await else {
        logged.update(ActionState::Skipped, Some(super::NOT_CONNECTED.into()));
        return Err(not_connected());
    };
    let api = &slack(app).api;
    if let Some((id, ts)) = day.anchor() {
        let updated = api
            .update_message(&secret, id, ts, &text)
            .await
            .map_err(|e| match e.code() {
                Some(code) if message_gone(code) => AppError::Network(format!(
                    "The day's message in Slack can't be edited ({code})."
                )),
                _ => failed(e),
            });
        return logged_result(app, &logged, updated);
    }
    let posted = api
        .post_message(&secret, &channel, &text)
        .await
        .map_err(failed);
    let posted = logged_result(app, &logged, posted)?;
    save(
        app,
        DayMessage {
            channel: Some(posted.channel),
            ts: Some(posted.ts).filter(|ts| !ts.is_empty()),
            greeting: String::new(),
            ..day
        },
    )
}

/// Replies to the day's message, also sent to the channel; without one, or when it is gone, the text
/// goes out as a plain channel message, so going Off is never held up. `false`: not shared or not
/// connected, nothing sent.
pub async fn post_day_reply(app: &AppHandle, text: &str) -> AppResult<bool> {
    let channel = focus_channel(app);
    if channel.is_empty() || text.trim().is_empty() {
        return Ok(false);
    }
    let logged = action_log::sending(
        ActionTarget::SlackMessage,
        &format!("{channel}: end-of-day list"),
    );
    let Some(secret) = token(app).await else {
        log::info!("End-of-day list not sent: Slack isn't connected");
        logged.update(ActionState::Skipped, Some(super::NOT_CONNECTED.into()));
        return Ok(false);
    };
    let api = &slack(app).api;
    let day = day_message(app);
    let replied = match day.anchor() {
        Some((id, ts)) => match api.post_reply(&secret, id, ts, text).await {
            Ok(()) => true,
            Err(e) if e.code().is_some_and(message_gone) => {
                log::info!("The day's message is gone ({e}); posting the list on its own");
                false
            }
            Err(e) => return logged_result(app, &logged, Err(failed(e))),
        },
        None => false,
    };
    if !replied {
        let posted = api
            .post_message(&secret, &channel, text)
            .await
            .map_err(failed);
        logged_result(app, &logged, posted)?;
    } else {
        logged.update(ActionState::Done, None);
    }
    save(
        app,
        DayMessage {
            eod_sent: true,
            ..day
        },
    )?;
    Ok(true)
}
