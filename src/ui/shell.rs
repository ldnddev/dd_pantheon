use crate::state::AppState;
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
    let mut block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(state.theme.active_border)
        .style(state.theme.app_shell);
    if let Some(frame) = loader::current(state) {
        block = block.title(
            Line::from(Span::styled(
                format!(" {frame} "),
                state.theme.warning_style,
            ))
            .right_aligned(),
        );
    }
    let header = Paragraph::new(state.header_copy.as_str()).block(block);
    f.render_widget(header, area);
}

pub fn draw_footer(f: &mut Frame, state: &AppState, area: Rect) {
    let keys = footer_keys(area.width);
    let bar = Paragraph::new(Line::from(keys)).style(state.theme.app_shell);
    f.render_widget(bar, area);
    let _ = state;
}

pub fn footer_keys(width: u16) -> String {
    let raw = if width < 80 {
        "F1:Help  F2:Theme  C-q:Quit  /:Filter"
    } else if width < 120 {
        "F1: Help   F2: Theme   Ctrl+Q: Quit   j/k: Nav   Enter: Run   /: Filter   :: Pal"
    } else {
        "F1: Help   F2: Theme   Ctrl+Q: Quit   F3: Doctor   j/k: Nav   Tab: Pane   Enter: Run   /: Filter   :: Palette   r: Refresh   (mouse: click/scroll)"
    };
    truncate_from_right(raw, width as usize)
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
