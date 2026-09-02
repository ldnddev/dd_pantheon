use crate::models::{CacheLevel, cache_ratio_level};
use crate::state::AppState;
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Sparkline};

pub fn draw(f: &mut Frame, state: &AppState, area: Rect, bordered: bool) {
    let theme = &state.theme;
    let inner = if bordered {
        let block = Block::default()
            .title("metrics  [d] w M")
            .borders(Borders::ALL)
            .border_style(theme.border)
            .style(theme.body);
        let inner = block.inner(area);
        f.render_widget(block, area);
        inner
    } else {
        area
    };

    let Some(series) = state.selected_metrics() else {
        let msg = if state.selected_env().is_none() {
            "select an environment"
        } else {
            "no metrics for this env"
        };
        f.render_widget(Paragraph::new(msg).style(theme.secondary), inner);
        return;
    };

    if inner.height < 3 {
        let last = series
            .points
            .last()
            .map(|p| p.cache_hit_ratio)
            .unwrap_or(0.0);
        f.render_widget(
            Paragraph::new(format!("cache {:.0}%", last * 100.0)).style(ratio_style(theme, last)),
            inner,
        );
        return;
    }

    let visits: Vec<u64> = series.points.iter().map(|p| p.visits).collect();
    let pages: Vec<u64> = series.points.iter().map(|p| p.pages_served).collect();
    let last = series.points.last();
    let ratio = last.map(|p| p.cache_hit_ratio).unwrap_or(0.0);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(inner);

    f.render_widget(
        Sparkline::default()
            .data(&visits)
            .style(theme.info)
            .max(visits.iter().copied().max().unwrap_or(1)),
        chunks[0],
    );
    f.render_widget(
        Sparkline::default()
            .data(&pages)
            .style(theme.label)
            .max(pages.iter().copied().max().unwrap_or(1)),
        chunks[1],
    );

    let gauge = Gauge::default()
        .ratio(ratio.clamp(0.0, 1.0))
        .label(format!("cache {:.0}%", ratio * 100.0))
        .gauge_style(ratio_style(theme, ratio));
    f.render_widget(gauge, chunks[2]);

    if let Some(p) = last {
        let line = Line::from(vec![Span::styled(
            format!("{}  {}v  {}p", p.datetime, p.visits, p.pages_served),
            theme.secondary,
        )]);
        f.render_widget(Paragraph::new(line), chunks[3]);
    }
}

fn ratio_style(theme: &Theme, ratio: f64) -> Style {
    match cache_ratio_level(ratio) {
        CacheLevel::Ok => theme.success,
        CacheLevel::Warn => theme.warning_style,
        CacheLevel::Bad => theme.error,
    }
}
