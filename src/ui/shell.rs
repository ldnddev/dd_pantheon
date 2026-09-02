use crate::state::AppState;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn draw_header(f: &mut Frame, state: &AppState, area: Rect) {
    let mut title = "dd_pantheon".to_string();
    if state.demo && area.width >= 40 {
        title.push_str("  DEMO");
    }
    let header = Paragraph::new(state.header_copy.as_str()).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(state.theme.active_border)
            .style(state.theme.app_shell),
    );
    f.render_widget(header, area);
}

pub fn draw_footer(f: &mut Frame, state: &AppState, area: Rect) {
    let keys = footer_keys(area.width);
    let bar = Paragraph::new(Line::from(keys)).style(state.theme.app_shell);
    f.render_widget(bar, area);
    let _ = state;
}

pub fn footer_keys(width: u16) -> &'static str {
    if width < 80 {
        "F1:Help  F2:Theme  C-q:Quit  /:Filter"
    } else if width < 120 {
        "F1: Help   F2: Theme   Ctrl+Q: Quit   F4: Layout   j/k: Nav   Enter: Run   /: Filter   :: Pal"
    } else {
        "F1: Help   F2: Theme   Ctrl+Q: Quit   F3: Doctor   F4: Layout   j/k: Nav   Tab: Pane   Enter: Run   /: Filter   :: Palette   r: Refresh   (mouse: click/scroll)"
    }
}
