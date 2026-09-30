use crate::state::{AppState, FocusPane};
use crate::ui::loader;
use crate::ui::pane_block;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn draw(f: &mut Frame, state: &mut AppState, area: Rect) {
    draw_inner(f, state, area, false);
}

pub fn draw_expanded(f: &mut Frame, state: &mut AppState, area: Rect) {
    draw_inner(f, state, area, true);
}

fn draw_inner(f: &mut Frame, state: &mut AppState, area: Rect, expanded: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    if state.log_follow {
        state.snap_log_follow();
    }
    let focused = expanded || state.focus == FocusPane::Log;

    let mut title_spans = vec![Span::raw("Job log activity")];
    if expanded {
        title_spans.push(Span::styled("  Esc close", state.theme.secondary));
    } else if focused {
        title_spans.push(Span::styled("  Enter expand", state.theme.info));
    }
    if !state.log_follow && !state.log_lines.is_empty() {
        title_spans.push(Span::styled("  G follow", state.theme.secondary));
    }
    let loader_frame = loader::current(state);
    if let Some(frame) = loader_frame {
        title_spans.push(Span::styled(
            format!("  {frame}"),
            state.theme.warning_style,
        ));
        title_spans.push(Span::styled("  Ctrl+C cancel", state.theme.warning_style));
    }
    let title = Line::from(title_spans);
    let status = loader_frame.unwrap_or_else(|| state.connection_status());
    let meta = Line::from(format!(
        "STATUS: {status}   SESSION: {}",
        state.session_started
    ))
    .right_aligned()
    .style(if state.job_running {
        state.theme.warning_style
    } else {
        state.theme.secondary
    });
    let inner_w = area.width.saturating_sub(2) as usize;
    let vis = area.height.saturating_sub(2).max(1) as usize;
    let start = (state.log_scroll as usize).min(state.log_lines.len());
    let end = (start + vis).min(state.log_lines.len());
    let text = if state.log_lines.is_empty() {
        if state.job_running {
            "running… output appears here".to_string()
        } else {
            "jobs print here · Enter expands · G follows tail".to_string()
        }
    } else {
        state.log_lines[start..end]
            .iter()
            .map(|line| truncate_line(line, inner_w))
            .collect::<Vec<_>>()
            .join("\n")
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

fn truncate_line(line: &str, width: usize) -> &str {
    if width == 0 || line.width() <= width {
        return line;
    }
    let mut cols = 0;
    for (i, ch) in line.char_indices() {
        let w = ch.width().unwrap_or(0);
        if cols + w > width {
            return &line[..i];
        }
        cols += w;
    }
    line
}
