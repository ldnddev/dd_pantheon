use crate::models::{InspectorTab, LayoutId};
use crate::state::{AppState, FocusPane};
use crate::ui::pane_block;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

pub fn draw(f: &mut Frame, state: &mut AppState, area: Rect) {
    if area.height == 0 {
        return;
    }
    match state.layout {
        LayoutId::TabbedInspector => draw_tabbed(f, state, area),
        LayoutId::ClassicStack | LayoutId::ThreeColumn => draw_stacked(f, state, area),
    }
}

fn inspector_title(state: &AppState) -> String {
    let mut title = match &state.selected {
        crate::state::TreeSel::Env { site, env } => format!("Inspector — {site}.{env}"),
        crate::state::TreeSel::Site(site) => format!("Inspector — {site}"),
        crate::state::TreeSel::None => "Inspector".to_string(),
    };
    if state.demo {
        title.push_str("  DEMO");
    }
    title
}

fn draw_stacked(f: &mut Frame, state: &mut AppState, area: Rect) {
    let focused = state.focus == FocusPane::Inspector;
    let block = pane_block(inspector_title(state), focused, &state.theme);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 3 {
        let summary = summary_line(state);
        f.render_widget(Paragraph::new(summary).style(state.theme.body), inner);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(4),
            Constraint::Length(inner.height.saturating_div(3).clamp(4, 8)),
            Constraint::Length(6.min(inner.height.saturating_sub(6)).max(3)),
        ])
        .split(inner);

    draw_info(f, state, chunks[0]);
    crate::ui::metrics::draw(f, state, chunks[1], false);
    crate::ui::actions::draw(f, state, chunks[2], "actions");
}

fn draw_tabbed(f: &mut Frame, state: &mut AppState, area: Rect) {
    let focused = state.focus == FocusPane::Inspector;
    let tabs = tab_line(state);
    let block = pane_block(tabs, focused, &state.theme).title(inspector_title(state));
    let inner = block.inner(area);
    f.render_widget(block, area);

    match state.inspector_tab {
        InspectorTab::Info => draw_info(f, state, inner),
        InspectorTab::Metrics => crate::ui::metrics::draw(f, state, inner, false),
        InspectorTab::Local => draw_local(f, state, inner),
        InspectorTab::Actions => crate::ui::actions::draw(f, state, inner, "actions"),
    }
}

fn tab_line(state: &AppState) -> Line<'static> {
    let tabs = [
        InspectorTab::Info,
        InspectorTab::Metrics,
        InspectorTab::Local,
        InspectorTab::Actions,
    ];
    let mut spans = Vec::new();
    for (i, tab) in tabs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        let selected = state.inspector_tab == *tab;
        let style = if selected {
            state.theme.active_label.add_modifier(Modifier::UNDERLINED)
        } else {
            state.theme.secondary
        };
        spans.push(Span::styled(format!("[{}]", tab.label()), style));
    }
    Line::from(spans)
}

fn draw_info(f: &mut Frame, state: &mut AppState, area: Rect) {
    state.tag_chips.clear();
    let mut lines = Vec::new();
    if !state.demo {
        match state.auth {
            crate::state::AuthState::LoggedOut => {
                lines.push(Line::from(Span::styled(
                    "not logged in — Ctrl+L to login",
                    state.theme.secondary,
                )));
            }
            crate::state::AuthState::Unknown if !state.tools_enabled => {
                lines.push(Line::from(Span::styled(
                    "terminus not on PATH",
                    state.theme.secondary,
                )));
            }
            _ => {}
        }
    }
    if let Some(site) = state.selected_site().cloned() {
        lines.push(kv(state, "framework", site.framework.label()));
        if site.frozen {
            lines.push(kv(state, "status", "frozen"));
        }
        if !site.orgs.is_empty() {
            let orgs = site
                .orgs
                .iter()
                .map(|o| o.org_name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(kv(state, "org", &orgs));
        } else {
            lines.push(Line::from(Span::styled(
                "tags require an organization",
                state.theme.secondary,
            )));
        }
        let tags_line_idx = lines.len();
        let tag_names: Vec<String> = site.tags.iter().map(|t| t.name.clone()).collect();
        if tag_names.is_empty() {
            lines.push(kv(state, "tags", "—"));
        } else {
            lines.push(tag_chips_line(state, &tag_names, area, tags_line_idx));
        }
        if let Some(plan) = &site.plan_name {
            lines.push(kv(state, "plan", plan));
        }
        if let Some(up) = site.upstream_label.as_ref().or(site.upstream.as_ref()) {
            lines.push(kv(state, "upstream", up));
        }
        if let Some(region) = &site.region {
            lines.push(kv(state, "region", region));
        }
        lines.push(kv(state, "id", &site.id));
    }
    if let Some(env) = state.selected_env() {
        lines.push(kv(state, "env", &env.id));
        lines.push(kv(state, "mode", env.connection_mode.label()));
        if let Some(d) = &env.domain {
            lines.push(kv(state, "domain", d));
        }
        if env.locked {
            lines.push(kv(state, "lock", "locked"));
        }
        if let Some(php) = &env.php_version {
            lines.push(kv(state, "php", php));
        }
        if let Some(runtime) = &env.php_runtime_generation {
            lines.push(kv(state, "runtime", runtime));
        }
        if let Some(drush) = &env.drush_version {
            lines.push(kv(state, "drush", drush));
        }
        if let Some(created) = &env.created {
            lines.push(kv(state, "created", created));
        }
    } else if matches!(state.selected, crate::state::TreeSel::Site(_)) {
        lines.push(Line::from(Span::styled(
            "select an environment for metrics",
            state.theme.secondary,
        )));
    }
    if let Some(local) = state.selected_site().and_then(|s| s.local.as_ref()) {
        let running = match local.running {
            Some(true) => "running",
            Some(false) => "stopped",
            None => "?",
        };
        lines.push(kv(
            state,
            "local",
            &format!("{}  {running}", local.path.display()),
        ));
    }

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: true })
        .scroll((state.inspector_scroll, 0));
    f.render_widget(p, area);
}

fn draw_local(f: &mut Frame, state: &AppState, area: Rect) {
    let mut lines = Vec::new();
    if let Some(local) = state.selected_site().and_then(|s| s.local.as_ref()) {
        lines.push(kv(state, "path", &local.path.display().to_string()));
        if let Some(name) = &local.lando_name {
            lines.push(kv(state, "app", name));
        }
        if let Some(recipe) = &local.recipe {
            lines.push(kv(state, "recipe", recipe));
        }
        let running = match local.running {
            Some(true) => "running",
            Some(false) => "stopped",
            None => "unknown",
        };
        lines.push(kv(state, "status", running));
        if let Some(url) = &local.url {
            lines.push(kv(state, "url", url));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "no local path bound",
            state.theme.secondary,
        )));
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn tag_chips_line(
    state: &mut AppState,
    tag_names: &[String],
    area: Rect,
    line_idx: usize,
) -> Line<'static> {
    let y = area.y.saturating_add(
        line_idx
            .saturating_sub(state.inspector_scroll as usize)
            .min(u16::MAX as usize) as u16,
    );
    let mut x = area.x.saturating_add(11);
    let mut spans = vec![Span::styled(format!("{:11}", "tags"), state.theme.label)];
    let focused = state.focus == FocusPane::Inspector;
    for name in tag_names {
        if x >= area.x.saturating_add(area.width.saturating_sub(4)) {
            spans.push(Span::styled(" …", state.theme.secondary));
            break;
        }
        let selected = state.selected_chip.as_deref() == Some(name.as_str());
        let show_close = focused && selected;
        let label = if show_close {
            format!("[{name} ×]")
        } else {
            format!("[{name}]")
        };
        let w = label.chars().count() as u16;
        let style = if selected {
            state.theme.active_label
        } else {
            state.theme.label
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
        let body = Rect::new(x, y, w, 1);
        let close = if show_close {
            Rect::new(x.saturating_add(w.saturating_sub(2)), y, 1, 1)
        } else {
            Rect::new(x, y, 0, 0)
        };
        state.tag_chips.push(crate::state::TagChipHit {
            name: name.clone(),
            body,
            close,
        });
        x = x.saturating_add(w.saturating_add(1));
    }
    Line::from(spans)
}

fn kv(state: &AppState, key: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{key:<11}"), state.theme.label),
        Span::raw(value.to_string()),
    ])
}

fn summary_line(state: &AppState) -> String {
    match &state.selected {
        crate::state::TreeSel::Env { site, env } => format!("{site}.{env}"),
        crate::state::TreeSel::Site(site) => site.clone(),
        crate::state::TreeSel::None => "no selection".into(),
    }
}
