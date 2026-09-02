use crate::state::AppState;
use crate::theme::color_hex;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub fn draw(f: &mut Frame, state: &AppState, area: Rect, scroll: u16) {
    let t = &state.theme;
    let c = t.colors;
    let mut lines = vec![
        header(state, "Status"),
        Line::from(format!("source          {}", t.source.label())),
        Line::from(format!("schema version  {}", t.version)),
        Line::from(format!("load            {}", state.theme_status.message)),
        Line::from(""),
        header(state, "Tokens"),
        token("base_background", c.base_background),
        token("body_background", c.body_background),
        token("modal_background", c.modal_background),
        token("text_primary", c.text_primary),
        token("text_secondary", c.text_secondary),
        token("text_labels", c.text_labels),
        token("text_active_focus", c.text_active_focus),
        token("modal_labels", c.modal_labels),
        token("modal_text", c.modal_text),
        token("modal_header", c.modal_header),
        token("selected_background", c.selected_background),
        token("border_default", c.border_default),
        token("border_active", c.border_active),
        token("scrollbar", c.scrollbar),
        token("success", c.success),
        token("warning", c.warning),
        token("error", c.error),
        token("info", c.info),
        token("folders", c.folders),
        token("files", c.files),
        token("links", c.links),
    ];
    if let Some(warn) = &t.warning {
        lines.push(Line::from(""));
        lines.push(header(state, "Warning"));
        lines.push(Line::from(Span::styled(
            warn.clone(),
            state.theme.warning_style,
        )));
    }

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
        .block(
            Block::default()
                .title("F2 Theme")
                .borders(Borders::ALL)
                .border_style(state.theme.active_border)
                .style(state.theme.modal),
        )
        .style(state.theme.modal_text);
    f.render_widget(p, area);
}

fn header<'a>(state: &'a AppState, title: &'a str) -> Line<'a> {
    Line::from(Span::styled(title.to_string(), state.theme.modal_header))
}

fn token(name: &str, color: ratatui::style::Color) -> Line<'static> {
    Line::from(format!("  {name:<22} {}", color_hex(color)))
}
