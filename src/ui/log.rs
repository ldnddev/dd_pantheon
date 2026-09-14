use crate::state::{AppState, FocusPane};
use crate::ui::pane_block;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};

pub fn draw(f: &mut Frame, state: &AppState, area: Rect) {
    draw_inner(f, state, area, false);
}

pub fn draw_expanded(f: &mut Frame, state: &AppState, area: Rect) {
    draw_inner(f, state, area, true);
}

fn draw_inner(f: &mut Frame, state: &AppState, area: Rect, expanded: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let focused = expanded || state.focus == FocusPane::Log;
    if !expanded && state.log_collapsed() {
        let p = Paragraph::new(Line::from(Span::styled(
            "Job log — idle",
            state.theme.secondary,
        )))
        .style(state.theme.app_shell);
        f.render_widget(p, area);
        return;
    }

    let status = if state.job_running {
        "RUNNING"
    } else {
        state.connection_status()
    };
    let mut title_spans = vec![Span::raw("Job log activity")];
    if expanded {
        title_spans.push(Span::styled("  Esc close", state.theme.secondary));
    } else if focused {
        title_spans.push(Span::styled("  Enter expand", state.theme.info));
    }
    if state.job_running {
        title_spans.push(Span::styled("  Ctrl+C cancel", state.theme.warning_style));
    }
    let title = Line::from(title_spans);
    let meta = Line::from(format!(
        "STATUS: {status}   SESSION: {}",
        state.session_started
    ))
    .right_aligned()
    .style(state.theme.secondary);
    let text = if state.log_lines.is_empty() {
        "no output".to_string()
    } else {
        state.log_lines.join("\n")
    };
    let block = if expanded {
        Block::default()
            .title(title)
            .title(meta)
            .borders(Borders::ALL)
            .border_style(state.theme.active_border)
            .style(state.theme.modal)
    } else {
        pane_block(title, focused, &state.theme).title(meta)
    };
    let p = Paragraph::new(text)
        .style(if expanded {
            state.theme.modal_text
        } else {
            state.theme.body
        })
        .wrap(Wrap { trim: false })
        .scroll((state.log_scroll, 0))
        .block(block);
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
