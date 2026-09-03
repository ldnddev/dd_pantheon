use crate::models::{CacheLevel, MetricsPeriod, cache_ratio_level};
use crate::state::{AppState, AuthState, PeriodHit};
use crate::theme::Theme;
use crate::workflows::metrics::metrics_inflight;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Sparkline};

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
    let mut spans = vec![Span::styled("metrics  ", state.theme.secondary)];
    let mut x = area.x + 9;
    for (i, period) in [
        MetricsPeriod::Day,
        MetricsPeriod::Week,
        MetricsPeriod::Month,
    ]
    .into_iter()
    .enumerate()
    {
        if i > 0 {
            spans.push(Span::raw(" "));
            x = x.saturating_add(1);
        }
        let active = state.metrics_period == period;
        let label = if active {
            format!("[{}]", period.key_label())
        } else {
            format!(" {} ", period.key_label())
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
    spans.push(Span::styled("  lagged · not APM", state.theme.secondary));
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
