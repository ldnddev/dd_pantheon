use crate::state::AppState;
use std::time::{Duration, Instant};

/// Bounce frames while a job is running.
pub const FRAMES: [&str; 4] = ["▃ d_d ▃", "▅ d_d ▅", "█ d_d █", "▅ d_d ▅"];

const FRAME_MS: u128 = 150;

pub fn frame_at(elapsed: Duration) -> &'static str {
    let i = (elapsed.as_millis() / FRAME_MS) as usize % FRAMES.len();
    FRAMES[i]
}

pub fn current(state: &AppState) -> Option<&'static str> {
    if !state.job_running {
        return None;
    }
    let started = state
        .jobs
        .values()
        .filter(|j| j.status.is_live())
        .map(|j| j.started_at)
        .min()
        .unwrap_or_else(Instant::now);
    Some(frame_at(started.elapsed()))
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
}
