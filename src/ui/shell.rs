use crate::state::{AppState, FocusPane, TreeSel};
use crate::ui::loader;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn draw_header(f: &mut Frame, state: &AppState, area: Rect) {
    let mut title = ">_ dd_pantheon".to_string();
    if state.demo && area.width >= 40 {
        title.push_str("  DEMO");
    }
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(state.theme.active_border)
        .style(state.theme.app_shell);
    let inner = block.inner(area);
    f.render_widget(Paragraph::new("").block(block), area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let status = header_status(state);
    let status_w = status.width() as u16;
    let quote_budget = inner.width.saturating_sub(if status_w == 0 {
        0
    } else {
        status_w.saturating_add(1)
    }) as usize;
    let quote = truncate_from_right(&state.header_copy, quote_budget);
    f.render_widget(Paragraph::new(quote).style(state.theme.app_shell), inner);
    if status_w > 0 && inner.width >= status_w {
        let rect = Rect::new(
            inner.x + inner.width.saturating_sub(status_w),
            inner.y + inner.height.saturating_sub(1) / 2,
            status_w,
            1,
        );
        let style = if state.job_running {
            state.theme.warning_style
        } else {
            state.theme.secondary
        };
        f.render_widget(
            Paragraph::new(Span::styled(status, style)).style(state.theme.app_shell),
            rect,
        );
    }
}

fn header_status(state: &AppState) -> String {
    let mut parts: Vec<String> = Vec::new();
    match &state.selected {
        TreeSel::Env { site, env } => parts.push(format!("{site}.{env}")),
        TreeSel::Site(site) => parts.push(site.clone()),
        TreeSel::None => {}
    }
    if let Some(frame) = loader::current(state) {
        parts.push(frame.to_string());
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" {} ", parts.join("  "))
    }
}

pub fn draw_footer(f: &mut Frame, state: &AppState, area: Rect) {
    let keys = footer_line(area.width, state);
    let bar = Paragraph::new(Line::from(keys)).style(state.theme.app_shell);
    f.render_widget(bar, area);
}

pub fn footer_keys(width: u16) -> String {
    footer_line_raw(width, FocusPane::Tree, false, false)
}

fn footer_line(width: u16, state: &AppState) -> String {
    footer_line_raw(width, state.focus, state.modal.is_some(), state.job_running)
}

fn footer_line_raw(width: u16, focus: FocusPane, modal: bool, job_running: bool) -> String {
    let raw = if width < 80 {
        "F1:Help  F2:Theme  C-q:Quit  /:Filter".to_string()
    } else if width < 120 {
        format!(
            "F1: Help   F2: Theme   Ctrl+Q: Quit   {}",
            pane_hint_medium(focus, modal, job_running)
        )
    } else {
        format!(
            "F1: Help   F2: Theme   Ctrl+Q: Quit   F3: Doctor   {}",
            pane_hint_wide(focus, modal, job_running)
        )
    };
    truncate_from_right(&raw, width as usize)
}

fn pane_hint_medium(focus: FocusPane, modal: bool, job_running: bool) -> String {
    let body = if modal {
        "Esc: Close   Enter: Confirm"
    } else {
        match focus {
            FocusPane::Tree => "j/k: Nav   Enter: Run   /: Filter   :: Pal",
            FocusPane::Inspector => "j/k: Scroll   Enter: Run   m: CMS",
            FocusPane::Preview => "Enter: Process   y: Copy   Esc: Unstage",
            FocusPane::Log => "j/k: Scroll   Enter: Expand   G: Follow",
        }
    };
    if job_running {
        format!("{body}   Ctrl+C: Cancel")
    } else {
        body.to_string()
    }
}

fn pane_hint_wide(focus: FocusPane, modal: bool, job_running: bool) -> String {
    let body = if modal {
        "Esc: Close   Enter: Confirm   click outside to dismiss"
    } else {
        match focus {
            FocusPane::Tree => {
                "j/k: Nav   Tab: Pane   Enter: Run   /: Filter   :: Palette   r: Refresh   (mouse: click/scroll)"
            }
            FocusPane::Inspector => {
                "j/k: Scroll   Enter: Run   m: CMS   [ ]: Tags   d/w/M: Period   (mouse: click/scroll)"
            }
            FocusPane::Preview => {
                "Enter: Process   y: Copy   Esc: Unstage   click Process/Copy/Cancel"
            }
            FocusPane::Log => {
                "j/k: Scroll   Enter: Expand   g/G: Top/Follow   PgUp/PgDn   (mouse: click/scroll)"
            }
        }
    };
    if job_running {
        format!("{body}   Ctrl+C: Cancel")
    } else {
        body.to_string()
    }
}

fn truncate_from_right(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if s.width() <= max {
        return s.to_string();
    }
    let mut out = String::new();
    for ch in s.chars() {
        let next = out.width() + ch.width().unwrap_or(0);
        if next > max {
            break;
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footer_truncates_from_the_right() {
        let s = truncate_from_right("F1:Help  F2:Theme  C-q:Quit  /:Filter", 10);
        assert_eq!(s, "F1:Help  F");
        assert!(s.starts_with("F1:Help"));
        assert_eq!(s.width(), 10);
    }
}
