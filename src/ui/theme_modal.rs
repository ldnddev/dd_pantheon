use crate::state::{AppState, Modal};
use crate::theme::color_from_rgb;
use ldnddev_theme::{ThemeEditorRow, theme_editor_rows};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn draw(f: &mut Frame, state: &AppState, area: Rect) {
    let Some(Modal::ThemeEditor(editor)) = &state.modal else {
        return;
    };
    let rows = theme_editor_rows(&editor.fields);
    let channel = ["R", "G", "B"][editor.channel.min(2)];
    let target = editor.save_target.label().to_uppercase();
    let mut lines: Vec<Line<'_>> = vec![
        Line::from(format!(
            "Save: {target} (Tab)   Channel: {channel} ([/])   Y save   R reset   Esc revert"
        )),
        Line::from(if editor.editing_hex {
            format!("Hex: {}█   Enter apply   Esc cancel edit", editor.hex_draft)
        } else {
            format!(
                "Hex: {}   Enter to type   +/- or h/l nudge",
                editor.hex_draft
            )
        }),
        Line::from(""),
    ];
    let view_h = area.height.saturating_sub(6) as usize;
    let start = rows
        .iter()
        .position(|row| match row {
            ThemeEditorRow::Color(idx) => *idx >= editor.scroll,
            ThemeEditorRow::Header(_) => false,
        })
        .unwrap_or(0);
    let start = if start > 0 && matches!(rows[start - 1], ThemeEditorRow::Header(_)) {
        start - 1
    } else {
        start
    };
    for row in rows.iter().skip(start).take(view_h.max(1)) {
        match row {
            ThemeEditorRow::Header(name) => {
                lines.push(Line::from(Span::styled(
                    format!(" {name}"),
                    state.theme.modal_header,
                )));
            }
            ThemeEditorRow::Color(idx) => {
                let field = editor.fields[*idx];
                let color = editor
                    .palette
                    .get(field.key)
                    .map(color_from_rgb)
                    .unwrap_or(ratatui::style::Color::Black);
                let hex = editor
                    .palette
                    .get(field.key)
                    .map(|c| c.to_hex())
                    .unwrap_or_else(|| "#000000".into());
                let rgb = editor.palette.get(field.key);
                let (r, g, b) = rgb.map(|c| (c.r, c.g, c.b)).unwrap_or((0, 0, 0));
                let cursor = if *idx == editor.selected { ">" } else { " " };
                let style = if *idx == editor.selected {
                    state.theme.selected
                } else {
                    state.theme.modal_text
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("{cursor} "), style),
                    Span::styled("██ ", ratatui::style::Style::default().fg(color)),
                    Span::styled(format!("{:<22} {hex}  {r:3},{g:3},{b:3}", field.key), style),
                ]));
            }
        }
    }
    let text = Paragraph::new(lines).block(
        Block::default()
            .title("F2 Theme editor")
            .borders(Borders::ALL)
            .border_style(state.theme.active_border)
            .style(state.theme.modal),
    );
    f.render_widget(text, area);
}
