//! The tooltip's window work: the card for a hovered button, its choices, and picks from it.

use std::time::Duration;

use tauri::{AppHandle, Manager};

use super::geometry::{self, bar_inner_edge};
use super::model::{RailButton, RailTool};
use super::tip::{Placement, RailTip, TipAction};
use super::{emit, rail, window};
use crate::bus;
use crate::events::{
    RAIL_TIP_CONTENT_EVENT, RAIL_TIP_HIDDEN_EVENT, RAIL_TIP_PLACED_EVENT, RAIL_TIP_POINTER_EVENT,
};
use crate::lock::MutexExt;
use crate::store::settings::ToolId;
use crate::tools;

/// The page fades the card out first.
const FADE: Duration = Duration::from_millis(160);

pub(super) fn apply_tip(app: &AppHandle, actions: Vec<TipAction>) {
    for action in actions {
        match action {
            TipAction::Column(frame) => window::set_frame(app, window::TIP, frame),
            TipAction::Show(frame) => {
                window::show(app, window::TIP);
                window::set_frame(app, window::TIP, frame);
            }
            TipAction::HideLater => {
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(FADE).await;
                    if !rail(&handle).tip.lock_unpoisoned().visible() {
                        window::order_out(&handle, window::TIP);
                    }
                });
            }
            TipAction::Clickable(clickable) => window::set_clickable(app, clickable),
            TipAction::Content(content) => {
                emit(app, window::TIP, RAIL_TIP_CONTENT_EVENT, content);
            }
            TipAction::Placed(sequence) => emit(app, window::TIP, RAIL_TIP_PLACED_EVENT, sequence),
            TipAction::Hidden(sequence) => emit(app, window::TIP, RAIL_TIP_HIDDEN_EVENT, sequence),
            TipAction::Pointer(point) => emit(app, window::TIP, RAIL_TIP_POINTER_EVENT, point),
        }
    }
}

pub(super) fn hide_tip(app: &AppHandle) {
    let actions = rail(app).tip.lock_unpoisoned().hide();
    apply_tip(app, actions);
}

fn find_tool(app: &AppHandle, id: ToolId) -> Option<RailTool> {
    let model = rail(app).model.lock_unpoisoned();
    model
        .as_ref()?
        .buttons
        .iter()
        .find_map(|button| match button {
            RailButton::Tool(tool) if tool.id == id => Some(tool.clone()),
            _ => None,
        })
}

/// Where the card for `tool` goes and which choices it lists; `None` while the rail is hidden.
fn tip_placement(
    app: &AppHandle,
    tool: &RailTool,
    center: f64,
) -> Option<(Placement, Option<ToolId>)> {
    let controller = rail(app).controller.lock_unpoisoned();
    let (frame, screen) = (controller.frame()?, controller.screen()?);
    let settings = controller.settings();
    let placement = Placement {
        x: bar_inner_edge(frame, controller.look(), controller.side()),
        y: frame.y + center,
        side: controller.side(),
        work_area: screen.visible,
        theme: settings.theme,
        highlight: settings.highlight,
    };
    Some((placement, tool.menu.filter(|_| settings.hover_menus)))
}

/// The page's button `id` is hovered, its centre `center` points below the rail window's top.
pub fn show_tip(app: &AppHandle, id: ToolId, center: f64) {
    if !center.is_finite() {
        return;
    }
    let Some(tool) = find_tool(app, id) else {
        return;
    };
    let Some((placement, menu)) = tip_placement(app, &tool, center) else {
        return;
    };
    let tip = RailTip {
        label: tool.label,
        detail: tool.detail,
        shortcut: tool.shortcut,
    };
    let actions = rail(app).tip.lock_unpoisoned().show(tip, menu, placement);
    apply_tip(app, actions);
    send_content(app);
}

/// Sends the current card with its tool's choices.
pub(super) fn send_content(app: &AppHandle) {
    let rail = rail(app);
    let menu = rail.tip.lock_unpoisoned().menu();
    let choices = menu.map_or_else(Vec::new, |menu| {
        let cached = rail.tools.lock_unpoisoned().clone();
        let state = cached.unwrap_or_else(|| tools::snapshot(app));
        tools::tool_choices(menu, &state)
    });
    let content = rail.tip.lock_unpoisoned().content(choices);
    apply_tip(app, content.into_iter().collect());
}

pub fn leave_tip(app: &AppHandle) {
    let actions = rail(app).tip.lock_unpoisoned().leave();
    apply_tip(app, actions);
}

pub fn tip_rendered(app: &AppHandle, sequence: u32, card: geometry::Frame) {
    let actions = rail(app).tip.lock_unpoisoned().rendered(sequence, card);
    apply_tip(app, actions);
}

/// Runs a choice the tooltip offers; anything else is ignored.
pub async fn pick(app: &AppHandle, command: &str) {
    let Some(actions) = rail(app).tip.lock_unpoisoned().pick(command) else {
        log::warn!("The tooltip doesn't offer {command}");
        return;
    };
    apply_tip(app, actions);
    app.state::<bus::Commands>().run(app.clone(), command).await;
}
