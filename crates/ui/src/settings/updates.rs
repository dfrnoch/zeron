//! Settings → General → Updates: the running version, what the release
//! checker last found, a manual "Check for updates", and — when a newer
//! release exists — the same next step the sidebar update strip offers.
//!
//! The shell owns the update lifecycle (the engine's `UpdateStatus` stream,
//! the manual check, the desktop download/restart flow) and pushes an
//! [`UpdatesView`] snapshot into the General page each frame; clicks go back
//! out as [`ShortcutsEvent`]s.

use gpui::{AnyElement, Context, SharedString, div, prelude::*, px};

use crate::settings::shortcuts::{ShortcutsEvent, ShortcutsPage};
use crate::settings::widgets;
use crate::theme::Theme;

/// The next step toward a newer release, per install kind (see
/// `Shell::update_strip_label` for the sidebar's wording of the same states).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateStep {
    /// Desktop install, nothing staged yet.
    Download,
    Downloading,
    /// Staged — one click swaps it in and relaunches.
    Restart,
    /// The download or apply failed; clicking tries again.
    Retry(SharedString),
    /// Symlink-managed install: updated from the CLI, no button.
    RunCli,
    /// Source build or hand-copied binary: the GitHub releases page.
    OpenReleases,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UpdatesView {
    pub current_version: SharedString,
    pub latest_version: Option<SharedString>,
    pub update_available: bool,
    /// Epoch ms of the last successful check.
    pub checked_at: Option<i64>,
    /// A manual check is in flight.
    pub checking: bool,
    /// Why the last manual check failed.
    pub error: Option<SharedString>,
    /// Present only while `update_available`.
    pub step: Option<UpdateStep>,
}

/// The row's status fragment. Pure.
pub fn status_line(view: &UpdatesView, now_ms: i64) -> String {
    if view.checking {
        return "Checking for updates…".into();
    }
    if view.update_available
        && let Some(latest) = &view.latest_version
    {
        return match view.step {
            Some(UpdateStep::Downloading) => format!("Downloading v{latest}…"),
            Some(UpdateStep::Restart) => format!("v{latest} is ready — restart to apply"),
            Some(UpdateStep::RunCli) => format!("v{latest} is available · run `zeron update`"),
            _ => format!("v{latest} is available"),
        };
    }
    match view.checked_at {
        Some(at) => format!("Up to date · checked {}", checked_ago(now_ms - at)),
        None => "Not checked yet".into(),
    }
}

fn checked_ago(elapsed_ms: i64) -> String {
    let secs = elapsed_ms.max(0) / 1000;
    if secs < 60 {
        "just now".into()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86_400)
    }
}

/// The primary button's label; `None` renders no button. Pure.
pub fn step_label(step: &UpdateStep) -> Option<&'static str> {
    match step {
        UpdateStep::Download => Some("Download update"),
        UpdateStep::Downloading => Some("Downloading…"),
        UpdateStep::Restart => Some("Restart to update"),
        UpdateStep::Retry(_) => Some("Try again"),
        UpdateStep::OpenReleases => Some("Open releases"),
        UpdateStep::RunCli => None,
    }
}

pub fn render(view: &UpdatesView, theme: &Theme, cx: &mut Context<ShortcutsPage>) -> AnyElement {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let checking = view.checking;
    let step = view.step.as_ref().filter(|_| view.update_available);

    let check_button = widgets::text_action(
        theme,
        widgets::ActionTone::Outlined,
        if checking {
            "Checking…"
        } else {
            "Check for updates"
        },
    )
    .id("check-for-updates")
    .debug_selector(|| "check-for-updates".into())
    .flex_none()
    .tab_index(0)
    .role(gpui::Role::Button)
    .aria_label("Check for updates")
    .focus_visible(|s| s.border_2().border_color(theme.accent))
    .when(checking, |el| el.opacity(0.35).cursor_default())
    .when(!checking, |el| {
        el.on_click(cx.listener(|_, _, _, cx| cx.emit(ShortcutsEvent::CheckForUpdates)))
    });

    let step_button = step.and_then(|step| {
        let label = step_label(step)?;
        let busy = matches!(step, UpdateStep::Downloading);
        Some(
            widgets::text_action(theme, widgets::ActionTone::Solid, label)
                .id("apply-update")
                .debug_selector(|| "apply-update".into())
                .flex_none()
                .tab_index(0)
                .role(gpui::Role::Button)
                .aria_label(label)
                .focus_visible(|s| s.border_2().border_color(theme.accent))
                .when(busy, |el| el.opacity(0.35).cursor_default())
                .when(!busy, |el| {
                    el.on_click(cx.listener(|_, _, _, cx| cx.emit(ShortcutsEvent::InstallUpdate)))
                }),
        )
    });

    let error = match step {
        Some(UpdateStep::Retry(message)) => Some(format!("Update failed: {message}").into()),
        _ => view.error.clone().map(|message| -> SharedString {
            format!("Couldn't check for updates: {message}").into()
        }),
    };

    widgets::section_card(theme)
        .child(
            widgets::card_row(theme, true)
                .child(
                    div()
                        .flex_1()
                        .min_w(px(160.0))
                        .child(widgets::row_title(theme, "Updates"))
                        .child(widgets::meta_line(
                            theme,
                            vec![
                                div()
                                    .child(format!("Zeron v{}", view.current_version))
                                    .into_any_element(),
                                div().child(status_line(view, now_ms)).into_any_element(),
                            ],
                        )),
                )
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .children(step_button)
                        .child(check_button),
                ),
        )
        .when_some(error, |card, error| {
            card.child(widgets::card_row(theme, false).child(widgets::error_strip(theme, error)))
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> UpdatesView {
        UpdatesView {
            current_version: "0.2.96".into(),
            ..UpdatesView::default()
        }
    }

    #[test]
    fn status_line_covers_each_state() {
        let now = 10_000_000;
        assert_eq!(status_line(&view(), now), "Not checked yet");
        assert_eq!(
            status_line(
                &UpdatesView {
                    checked_at: Some(now - 5 * 60_000),
                    ..view()
                },
                now
            ),
            "Up to date · checked 5m ago"
        );
        assert_eq!(
            status_line(
                &UpdatesView {
                    checking: true,
                    update_available: true,
                    latest_version: Some("0.2.97".into()),
                    ..view()
                },
                now
            ),
            "Checking for updates…"
        );
        let available = UpdatesView {
            update_available: true,
            latest_version: Some("0.2.97".into()),
            step: Some(UpdateStep::Download),
            ..view()
        };
        assert_eq!(status_line(&available, now), "v0.2.97 is available");
        assert_eq!(
            status_line(
                &UpdatesView {
                    step: Some(UpdateStep::Restart),
                    ..available.clone()
                },
                now
            ),
            "v0.2.97 is ready — restart to apply"
        );
        assert_eq!(
            status_line(
                &UpdatesView {
                    step: Some(UpdateStep::RunCli),
                    ..available
                },
                now
            ),
            "v0.2.97 is available · run `zeron update`"
        );
    }

    #[test]
    fn cli_managed_installs_get_no_button() {
        assert_eq!(step_label(&UpdateStep::RunCli), None);
        assert_eq!(step_label(&UpdateStep::Restart), Some("Restart to update"));
    }
}
