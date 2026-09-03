use crate::state::{AppState, FocusPane};
use crate::ui::pane_block;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap};

pub fn draw(f: &mut Frame, state: &AppState, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let focused = state.focus == FocusPane::Log;
    if state.log_collapsed() {
        let p = Paragraph::new(Line::from(Span::styled(
            "Job log — idle",
            state.theme.secondary,
        )))
        .style(state.theme.app_shell);
        f.render_widget(p, area);
        return;
    }

    let title = if state.job_running {
        "Job log  running  Ctrl+C cancel"
    } else if state.demo {
        "Job log  demo"
    } else {
        "Job log"
    };
    let text = if state.log_lines.is_empty() {
        "no output".to_string()
    } else {
        state.log_lines.join("\n")
    };
    let p = Paragraph::new(text)
        .style(state.theme.body)
        .wrap(Wrap { trim: false })
        .scroll((state.log_scroll, 0))
        .block(pane_block(title, focused, &state.theme));
    f.render_widget(p, area);
    if state.log_lines.len() as u16 + 2 > area.height {
        let mut sb =
            ScrollbarState::new(state.log_lines.len().max(1)).position(state.log_scroll as usize);
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight).style(state.theme.scrollbar),
            area,
            &mut sb,
        );
    }
}
