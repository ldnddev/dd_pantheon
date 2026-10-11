use crate::models::ConnectionMode;
use crate::state::{AppState, FocusPane, TreeRow};
use crate::theme::Theme;
use crate::ui::pane_block;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Scrollbar, ScrollbarOrientation, ScrollbarState};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

struct RowView {
    site_name: String,
    env: Option<String>,
    expanded: bool,
    frozen: bool,
    mode: Option<ConnectionMode>,
    locked: bool,
}

pub fn draw(f: &mut Frame, state: &mut AppState, area: Rect) {
    let focused = state.focus == FocusPane::Tree;
    let count = state.sites.len();
    let filter = if state.filter.is_empty() {
        String::new()
    } else {
        format!("  /{}", state.filter)
    };
    let tag = state
        .tag_filter
        .as_ref()
        .map(|t| format!("  tag:{t}"))
        .unwrap_or_default();
    let title = format!("Available sites ({count}){filter}{tag}");

    let views: Vec<RowView> = state
        .tree_rows
        .iter()
        .map(|row| row_view(state, row))
        .collect();
    let len = views.len();
    let selected = state.tree_state.selected().unwrap_or(0);

    // Borders take one column on each side. The scrollbar is drawn on the border.
    let content_w = area.width.saturating_sub(2) as usize;
    let items: Vec<ListItem> = if views.is_empty() {
        let msg = empty_tree_message(state);
        vec![ListItem::new(Line::from(Span::styled(
            msg,
            state.theme.secondary,
        )))]
    } else {
        views
            .iter()
            .map(|row| tree_item(row, &state.theme, content_w))
            .collect()
    };

    let list = List::new(items)
        .block(pane_block(title, focused, &state.theme))
        .highlight_style(state.theme.selected);
    f.render_stateful_widget(list, area, &mut state.tree_state);

    if len as u16 + 2 > area.height {
        let mut sb = ScrollbarState::new(len).position(selected);
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight).style(state.theme.scrollbar),
            area,
            &mut sb,
        );
    }
}

fn empty_tree_message(state: &AppState) -> &'static str {
    use crate::state::AuthState;
    if state.demo {
        return "no sites";
    }
    if !state.tools_enabled {
        return "terminus not on PATH";
    }
    match state.auth {
        AuthState::LoggedOut | AuthState::Unknown => "not logged in — Ctrl+L",
        AuthState::LoggedIn { .. } => {
            if state.inflight_readonly.contains_key("site:list") {
                "loading sites…"
            } else {
                "no sites — r to refresh, n to create"
            }
        }
    }
}

fn row_view(state: &AppState, row: &TreeRow) -> RowView {
    let site = state.site(&row.site);
    let env = row.env.as_ref().and_then(|id| {
        state
            .envs
            .get(&row.site)
            .and_then(|envs| envs.iter().find(|e| e.id == *id))
    });
    RowView {
        site_name: row.site.clone(),
        env: row.env.clone(),
        expanded: row.expanded,
        frozen: site.is_some_and(|s| s.frozen),
        mode: env.map(|e| e.connection_mode),
        locked: env.is_some_and(|e| e.locked),
    }
}

struct RowText {
    prefix: String,
    name: String,
    show_status: bool,
}

fn row_label(row: &RowView) -> &str {
    row.env.as_deref().unwrap_or(row.site_name.as_str())
}

fn row_prefix(row: &RowView) -> String {
    if row.env.is_none() {
        let glyph = if row.expanded { "▾" } else { "▸" };
        format!("{glyph} ")
    } else {
        "    ▪ ".to_string()
    }
}

/// Status after the name: frozen, or connection mode and lock. Tags and
/// framework stay in the inspector.
fn row_status(row: &RowView) -> String {
    if row.env.is_none() {
        if row.frozen {
            "  frozen".to_string()
        } else {
            String::new()
        }
    } else {
        let mut status = String::new();
        if let Some(mode) = row.mode {
            status.push_str("  ");
            status.push_str(mode.label());
        }
        if row.locked {
            status.push_str("  lock");
        }
        status
    }
}

fn row_text(row: &RowView, width: usize) -> RowText {
    if width == 0 {
        return RowText {
            prefix: String::new(),
            name: String::new(),
            show_status: false,
        };
    }
    let prefix = row_prefix(row);
    if prefix.width() >= width {
        return RowText {
            prefix: truncate_with_ellipsis(&prefix, width),
            name: String::new(),
            show_status: false,
        };
    }
    let status = row_status(row);
    // Keep the status by shrinking the name. Drop status only when one
    // character of the name plus … cannot sit beside it.
    let (name, show_status) = fit_name(row_label(row), status.width(), width - prefix.width());
    RowText {
        prefix,
        name,
        show_status,
    }
}

fn fit_name(name: &str, status_w: usize, budget: usize) -> (String, bool) {
    let name_w = name.width();
    if name_w + status_w <= budget {
        return (name.to_string(), status_w > 0);
    }
    let ell_w = "…".width();
    if status_w > 0 && status_w < budget {
        let name_budget = budget - status_w;
        if name_budget > ell_w {
            return (truncate_with_ellipsis(name, name_budget), true);
        }
    }
    (truncate_with_ellipsis(name, budget), false)
}

fn truncate_with_ellipsis(text: &str, budget: usize) -> String {
    if text.width() <= budget {
        return text.to_string();
    }
    let ell = "…";
    let ell_w = ell.width();
    if budget < ell_w {
        return String::new();
    }
    let keep = budget - ell_w;
    let mut out = String::new();
    for ch in text.chars() {
        let next = out.width() + ch.width().unwrap_or(0);
        if next > keep {
            break;
        }
        out.push(ch);
    }
    out.push_str(ell);
    out
}

fn tree_item<'a>(row: &'a RowView, theme: &Theme, width: usize) -> ListItem<'a> {
    let text = row_text(row, width);
    let name_style = if row.env.is_none() {
        if row.frozen {
            Style::default().fg(theme
                .colors
                .text_disabled
                .unwrap_or(theme.colors.text_secondary))
        } else {
            Style::default().fg(theme.colors.folders)
        }
    } else {
        Style::default().fg(theme.colors.files)
    };
    let mut spans = Vec::new();
    if !text.prefix.is_empty() {
        spans.push(Span::raw(text.prefix));
    }
    if !text.name.is_empty() {
        spans.push(Span::styled(text.name, name_style));
    }
    if text.show_status {
        if row.env.is_none() {
            spans.push(Span::styled("  frozen", theme.warning_style));
        } else {
            if let Some(mode) = row.mode {
                spans.push(Span::styled(format!("  {}", mode.label()), theme.secondary));
            }
            if row.locked {
                spans.push(Span::styled("  lock", theme.warning_style));
            }
        }
    }
    ListItem::new(Line::from(spans))
}

#[cfg(test)]
fn visible_line(row: &RowView, width: usize) -> String {
    let text = row_text(row, width);
    let mut line = text.prefix;
    line.push_str(&text.name);
    if text.show_status {
        line.push_str(&row_status(row));
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    fn site(name: &str, frozen: bool) -> RowView {
        RowView {
            site_name: name.into(),
            env: None,
            expanded: false,
            frozen,
            mode: None,
            locked: false,
        }
    }

    fn env_row(id: &str, mode: ConnectionMode, locked: bool) -> RowView {
        RowView {
            site_name: "acme-wp".into(),
            env: Some(id.into()),
            expanded: true,
            frozen: false,
            mode: Some(mode),
            locked,
        }
    }

    #[test]
    fn short_site_row_is_name_only() {
        let line = visible_line(&site("acme-wp", false), 32);
        assert_eq!(line, "▸ acme-wp");
    }

    #[test]
    fn frozen_status_stays_when_the_name_shrinks() {
        let line = visible_line(&site("super-long-client-site-name", true), 20);
        assert!(line.ends_with("  frozen"), "{line}");
        assert!(line.contains('…'), "{line}");
        assert!(!line.contains('['), "{line}");
        assert!(line.width() <= 20, "{line}");
    }

    #[test]
    fn status_drops_only_when_the_name_cannot_keep_a_character() {
        let line = visible_line(&site("super-long-client-site-name", true), 10);
        assert!(!line.contains("frozen"), "{line}");
        assert!(line.contains('…'), "{line}");
        assert!(line.width() <= 10, "{line}");
    }

    #[test]
    fn env_row_keeps_mode_and_lock() {
        let line = visible_line(&env_row("live", ConnectionMode::Git, true), 32);
        assert_eq!(line, "    ▪ live  git  lock");
        let sftp = visible_line(&env_row("feat-x", ConnectionMode::Sftp, false), 32);
        assert_eq!(sftp, "    ▪ feat-x  sftp");
    }
}
