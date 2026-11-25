use crate::state::AppState;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use std::time::{Duration, Instant};

/// Bounce frames while a job is running.
pub const FRAMES: [&str; 4] = ["▃ d_d ▃", "▅ d_d ▅", "█ d_d █", "▅ d_d ▅"];
const BARS: [char; 4] = ['▃', '▅', '█', '▅'];

pub const FRAME_MS: u128 = 150;

pub fn frame_at(elapsed: Duration) -> &'static str {
    FRAMES[frame_index_at(elapsed)]
}

pub fn spans(state: &AppState) -> Vec<Span<'static>> {
    let Some(started) = live_started_at(state) else {
        return Vec::new();
    };
    spans_at(
        started.elapsed(),
        state.theme.colors.info,
        state.theme.colors.text_active_focus,
    )
}

pub fn spans_at(elapsed: Duration, info: Color, focus: Color) -> Vec<Span<'static>> {
    let bar = BARS[frame_index_at(elapsed)].to_string();
    vec![
        Span::styled(bar.clone(), Style::default().fg(info)),
        Span::styled(
            " d_d ",
            Style::default().fg(focus).add_modifier(Modifier::BOLD),
        ),
        Span::styled(bar, Style::default().fg(info)),
    ]
}

pub fn frame_index_at(elapsed: Duration) -> usize {
    (elapsed.as_millis() / FRAME_MS) as usize % FRAMES.len()
}

fn live_started_at(state: &AppState) -> Option<Instant> {
    if !state.job_running {
        return None;
    }
    state
        .jobs
        .values()
        .filter(|j| j.status.is_live())
        .map(|j| j.started_at)
        .min()
}

pub fn current(state: &AppState) -> Option<&'static str> {
    live_started_at(state).map(|started| frame_at(started.elapsed()))
}

pub fn frame_index(state: &AppState) -> Option<usize> {
    live_started_at(state).map(|started| frame_index_at(started.elapsed()))
}

/// Time until the bounce glyph should advance.
pub fn next_frame_in(state: &AppState) -> Option<Duration> {
    let started = live_started_at(state)?;
    let into = started.elapsed().as_millis() % FRAME_MS;
    Some(Duration::from_millis((FRAME_MS - into) as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_bounce_low_mid_high_mid() {
        assert_eq!(frame_at(Duration::from_millis(0)), "▃ d_d ▃");
        assert_eq!(frame_at(Duration::from_millis(150)), "▅ d_d ▅");
        assert_eq!(frame_at(Duration::from_millis(300)), "█ d_d █");
        assert_eq!(frame_at(Duration::from_millis(450)), "▅ d_d ▅");
        assert_eq!(frame_at(Duration::from_millis(600)), "▃ d_d ▃");
    }

    fn span_label(elapsed: Duration) -> String {
        spans_at(elapsed, Color::White, Color::White)
            .into_iter()
            .map(|span| span.content.into_owned())
            .collect()
    }

    #[test]
    fn span_frames_match_the_string_loader() {
        assert_eq!(span_label(Duration::from_millis(0)), "▃ d_d ▃");
        assert_eq!(span_label(Duration::from_millis(149)), "▃ d_d ▃");
        assert_eq!(span_label(Duration::from_millis(150)), "▅ d_d ▅");
        assert_eq!(span_label(Duration::from_millis(300)), "█ d_d █");
        assert_eq!(span_label(Duration::from_millis(450)), "▅ d_d ▅");
        assert_eq!(span_label(Duration::from_millis(600)), "▃ d_d ▃");
    }
}
