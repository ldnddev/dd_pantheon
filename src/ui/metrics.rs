use crate::models::{CacheLevel, MetricsPeriod, cache_ratio_level};
use crate::state::{AppState, AuthState, PeriodHit};
use crate::theme::Theme;
use crate::workflows::metrics::metrics_inflight;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Sparkline};

pub fn draw_dashboard(f: &mut Frame, state: &mut AppState, area: Rect) {
    state.period_hits.clear();
    if area.width == 0 || area.height == 0 {
        return;
    }
    let period_h = 1;
    let card_h = if area.height >= 6 { 3 } else { 0 };
    let mut constraints = vec![Constraint::Length(period_h)];
    if card_h > 0 {
        constraints.push(Constraint::Length(card_h));
    }
    constraints.push(Constraint::Min(1));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);
    draw_period_bar(f, state, chunks[0]);
    let mut idx = 1;
    if card_h > 0 {
        draw_metric_cards(f, state, chunks[idx]);
        idx += 1;
    }
    draw_graph(f, state, chunks[idx]);
}

fn draw_metric_cards(f: &mut Frame, state: &AppState, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Length(1),
            Constraint::Ratio(1, 3),
            Constraint::Length(1),
            Constraint::Ratio(1, 3),
        ])
        .split(area);
    let (visits, views, cache) = last_metric_values(state);
    draw_card(f, state, cols[0], "visitors", visits.as_str(), None);
    draw_card(f, state, cols[2], "views", views.as_str(), None);
    let cache_style = state.selected_metrics().and_then(|s| {
        s.points
            .last()
            .map(|p| ratio_style(&state.theme, p.cache_hit_ratio))
    });
    draw_card(f, state, cols[4], "cache", cache.as_str(), cache_style);
}

fn draw_card(
    f: &mut Frame,
    state: &AppState,
    area: Rect,
    title: &str,
    value: &str,
    value_style: Option<Style>,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(state.theme.border)
        .title(Span::styled(title, state.theme.secondary))
        .style(state.theme.body);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }
    let style = value_style.unwrap_or(state.theme.body.add_modifier(Modifier::BOLD));
    f.render_widget(
        Paragraph::new(value)
            .alignment(Alignment::Center)
            .style(style),
        inner,
    );
}

fn draw_graph(f: &mut Frame, state: &AppState, area: Rect) {
    let theme = &state.theme;
    if let Some(msg) = empty_message(state) {
        f.render_widget(
            Paragraph::new(msg)
                .alignment(Alignment::Center)
                .style(theme.secondary),
            area,
        );
        return;
    }
    if let Some(err) = error_message(state) {
        f.render_widget(Paragraph::new(err).style(theme.error), area);
        return;
    }
    let Some(series) = state.selected_metrics() else {
        let msg = if metrics_inflight(state) {
            "loading metrics…"
        } else {
            "no metrics yet"
        };
        f.render_widget(
            Paragraph::new(msg)
                .alignment(Alignment::Center)
                .style(theme.secondary),
            area,
        );
        return;
    };
    if series.points.is_empty() || area.height == 0 {
        f.render_widget(
            Paragraph::new("no metrics yet")
                .alignment(Alignment::Center)
                .style(theme.secondary),
            area,
        );
        return;
    }
    let visits: Vec<u64> = series.points.iter().map(|p| p.visits).collect();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border)
        .style(theme.body);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    f.render_widget(
        Sparkline::default()
            .data(&visits)
            .style(theme.info)
            .max(visits.iter().copied().max().unwrap_or(1)),
        inner,
    );
}

fn last_metric_values(state: &AppState) -> (String, String, String) {
    match state.selected_metrics().and_then(|s| s.points.last()) {
        Some(p) => (
            compact_count(p.visits),
            compact_count(p.pages_served),
            format!("{:.0}%", p.cache_hit_ratio * 100.0),
        ),
        None => ("—".into(), "—".into(), "—".into()),
    }
}

pub fn compact_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

pub fn draw(f: &mut Frame, state: &mut AppState, area: Rect, bordered: bool) {
    state.period_hits.clear();
    let theme = &state.theme;
    let inner = if bordered {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border)
            .style(theme.body);
        let inner = block.inner(area);
        f.render_widget(block, area);
        inner
    } else {
        area
    };
    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);
    draw_period_bar(f, state, chunks[0]);
    draw_body(f, state, chunks[1]);
}

fn draw_period_bar(f: &mut Frame, state: &mut AppState, area: Rect) {
    let prefix = "metrics (";
    let mut spans = vec![Span::styled(prefix, state.theme.secondary)];
    let mut x = area.x + prefix.chars().count() as u16;
    for (i, period) in [
        MetricsPeriod::Day,
        MetricsPeriod::Week,
        MetricsPeriod::Month,
    ]
    .into_iter()
    .enumerate()
    {
        if i > 0 {
            spans.push(Span::styled(" / ", state.theme.secondary));
            x = x.saturating_add(3);
        }
        let active = state.metrics_period == period;
        let key = period.key_label();
        let label = if active {
            format!("[{key}]")
        } else {
            key.to_string()
        };
        let w = label.chars().count() as u16;
        let style = if active {
            state.theme.active_label.add_modifier(Modifier::BOLD)
        } else {
            state.theme.secondary
        };
        spans.push(Span::styled(label, style));
        state.period_hits.push(PeriodHit {
            period,
            area: Rect::new(x, area.y, w, 1),
        });
        x = x.saturating_add(w);
    }
    spans.push(Span::styled(")", state.theme.secondary));
    if area.width > 28 {
        spans.push(Span::styled("  lagged", state.theme.secondary));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_body(f: &mut Frame, state: &AppState, area: Rect) {
    let theme = &state.theme;
    if let Some(msg) = empty_message(state) {
        f.render_widget(Paragraph::new(msg).style(theme.secondary), area);
        return;
    }

    if let Some(err) = error_message(state) {
        f.render_widget(Paragraph::new(err).style(theme.error), area);
        return;
    }

    let Some(series) = state.selected_metrics() else {
        let msg = if metrics_inflight(state) {
            "loading metrics…"
        } else {
            "No metrics yet for this env"
        };
        f.render_widget(Paragraph::new(msg).style(theme.secondary), area);
        return;
    };

    if series.points.is_empty() {
        f.render_widget(
            Paragraph::new("No metrics yet for this env").style(theme.secondary),
            area,
        );
        return;
    }

    if area.height < 3 {
        let last = series
            .points
            .last()
            .map(|p| p.cache_hit_ratio)
            .unwrap_or(0.0);
        f.render_widget(
            Paragraph::new(format!("cache {:.0}%", last * 100.0)).style(ratio_style(theme, last)),
            area,
        );
        return;
    }

    let visits: Vec<u64> = series.points.iter().map(|p| p.visits).collect();
    let pages: Vec<u64> = series.points.iter().map(|p| p.pages_served).collect();
    let last = series.points.last();
    let ratio = last.map(|p| p.cache_hit_ratio).unwrap_or(0.0);

    let table_h = area.height.saturating_sub(4).min(8);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(table_h.max(1)),
        ])
        .split(area);

    draw_spark_row(f, chunks[0], "visits", &visits, theme.info, theme);
    draw_spark_row(f, chunks[1], "pages", &pages, theme.active_label, theme);

    let gauge = Gauge::default()
        .ratio(ratio.clamp(0.0, 1.0))
        .label(format!("cache {:.0}%", ratio * 100.0))
        .gauge_style(ratio_style(theme, ratio));
    f.render_widget(gauge, chunks[2]);

    let n = series.points.len().min(14);
    let start = series.points.len().saturating_sub(n);
    let mut lines = Vec::new();
    for p in series.points[start..].iter().rev() {
        lines.push(Line::from(Span::styled(
            format!(
                "{}  {:>5}v  {:>5}p  {:>3.0}%",
                display_datetime(&p.datetime),
                p.visits,
                p.pages_served,
                p.cache_hit_ratio * 100.0
            ),
            theme.secondary,
        )));
    }
    f.render_widget(Paragraph::new(lines), chunks[3]);
}

fn draw_spark_row(
    f: &mut Frame,
    area: Rect,
    label: &str,
    data: &[u64],
    style: Style,
    theme: &Theme,
) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(7), Constraint::Min(1)])
        .split(area);
    f.render_widget(Paragraph::new(label).style(theme.secondary), chunks[0]);
    if chunks[1].width == 0 {
        return;
    }
    f.render_widget(
        Sparkline::default()
            .data(data)
            .style(style)
            .max(data.iter().copied().max().unwrap_or(1)),
        chunks[1],
    );
}

pub fn display_datetime(raw: &str) -> String {
    let raw = raw.trim();
    if let Ok(d) = chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        return d.format("%Y-%m-%d").to_string();
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
        return dt.format("%Y-%m-%d").to_string();
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S") {
        return dt.format("%Y-%m-%d").to_string();
    }
    raw.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datetime_display_strips_time() {
        assert_eq!(display_datetime("2026-08-17"), "2026-08-17");
        assert_eq!(display_datetime("2026-08-17T12:01:03Z"), "2026-08-17");
        assert_eq!(display_datetime("2026-08-17 12:01:03"), "2026-08-17");
    }

    #[test]
    fn compact_count_formats_thousands() {
        assert_eq!(compact_count(940), "940");
        assert_eq!(compact_count(1_200), "1.2k");
        assert_eq!(compact_count(4_800), "4.8k");
        assert_eq!(compact_count(1_500_000), "1.5M");
    }
}

fn empty_message(state: &AppState) -> Option<&'static str> {
    if !state.demo {
        if !state.tools_enabled {
            return Some("terminus not on PATH");
        }
        match state.auth {
            AuthState::LoggedOut | AuthState::Unknown => {
                return Some("Not logged in — open login from F3");
            }
            AuthState::LoggedIn { .. } => {}
        }
    }
    if state.selected_site().is_some_and(|s| s.frozen) {
        return Some("Site frozen — metrics unavailable");
    }
    if state.selected_env().is_none() {
        return Some("select an environment");
    }
    None
}

fn ratio_style(theme: &Theme, ratio: f64) -> Style {
    match cache_ratio_level(ratio) {
        CacheLevel::Ok => theme.success,
        CacheLevel::Warn => theme.warning_style,
        CacheLevel::Bad => theme.error,
    }
}

fn error_message(state: &AppState) -> Option<String> {
    if state.selected_metrics().is_some() {
        return None;
    }
    if let Some(err) = &state.metrics_error {
        return Some(format!("{err}  · r to retry"));
    }
    None
}
