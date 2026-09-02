use crate::models::{ConnectionMode, Framework};
use crate::state::{AppState, FocusPane, TreeRow};
use crate::theme::Theme;
use crate::ui::pane_block;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Scrollbar, ScrollbarOrientation, ScrollbarState};

struct RowView {
    site_name: String,
    env: Option<String>,
    expanded: bool,
    frozen: bool,
    tags: Vec<String>,
    framework: Option<Framework>,
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
    let title = format!("Sites ({count}){filter}{tag}");

    let views: Vec<RowView> = state
        .tree_rows
        .iter()
        .map(|row| row_view(state, row))
        .collect();
    let len = views.len();
    let selected = state.tree_state.selected().unwrap_or(0);

    let items: Vec<ListItem> = if views.is_empty() {
        let msg = empty_tree_message(state);
        vec![ListItem::new(Line::from(Span::styled(
            msg,
            state.theme.secondary,
        )))]
    } else {
        views
            .iter()
            .map(|row| tree_item(row, &state.theme))
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
                "no sites"
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
        tags: site
            .map(|s| s.tags.iter().take(3).map(|t| t.name.clone()).collect())
            .unwrap_or_default(),
        framework: site.map(|s| s.framework),
        mode: env.map(|e| e.connection_mode),
        locked: env.is_some_and(|e| e.locked),
    }
}

fn tree_item<'a>(row: &'a RowView, theme: &Theme) -> ListItem<'a> {
    if row.env.is_none() {
        let glyph = if row.expanded { "▾" } else { "▸" };
        let mut spans = vec![
            Span::raw(format!("{glyph} ")),
            Span::styled(
                row.site_name.as_str(),
                if row.frozen {
                    Style::default().fg(theme
                        .colors
                        .text_disabled
                        .unwrap_or(theme.colors.text_secondary))
                } else {
                    Style::default().fg(theme.colors.folders)
                },
            ),
        ];
        if row.frozen {
            spans.push(Span::styled("  frozen", theme.warning_style));
        }
        for tag in &row.tags {
            spans.push(Span::styled(format!(" [{tag}]"), theme.label));
        }
        if let Some(fw) = row.framework {
            spans.push(Span::styled(format!(" [{}]", fw.short()), theme.secondary));
        }
        ListItem::new(Line::from(spans))
    } else {
        let env_id = row.env.as_deref().unwrap_or("");
        let mut spans = vec![Span::styled(
            format!("    {env_id}"),
            Style::default().fg(theme.colors.files),
        )];
        if let Some(mode) = row.mode {
            spans.push(Span::styled(format!("  {}", mode.label()), theme.secondary));
        }
        if row.locked {
            spans.push(Span::styled("  lock", theme.warning_style));
        }
        ListItem::new(Line::from(spans))
    }
}
