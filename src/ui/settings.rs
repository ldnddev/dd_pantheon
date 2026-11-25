use crate::state::{AppState, SETTINGS_HELP, SETTINGS_PAGES};
use crate::ui::help;
use ldnddev_theme::{PickerRow, ThemeEditorRow, theme_editor_rows};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

pub fn draw(f: &mut Frame, state: &mut AppState, body: Rect) {
    state.settings_nav_area = Rect::default();
    if body.width == 0 || body.height == 0 {
        return;
    }
    let width = (body.width / 2).max(1).min(body.width);
    let area = Rect {
        x: body.x + body.width.saturating_sub(width),
        y: body.y,
        width,
        height: body.height,
    };
    f.render_widget(Clear, area);
    let block = Block::default()
        .title(" Settings ")
        .borders(Borders::ALL)
        .border_style(state.theme.active_border)
        .title_style(
            Style::default()
                .fg(state.theme.colors.modal_header)
                .add_modifier(Modifier::BOLD),
        )
        .style(state.theme.modal);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    if state.settings_nav as usize >= SETTINGS_PAGES.len() {
        state.settings_nav = 0;
    }

    let nav_w = {
        let spare = inner.width.saturating_sub(8);
        if spare == 0 {
            1.min(inner.width)
        } else {
            14.min(spare).max(1)
        }
    };
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(nav_w), Constraint::Min(1)])
        .split(inner);
    let nav = cols[0];
    let page = cols[1];
    state.settings_nav_area = nav;
    paint_nav(f, state, nav);

    let section = SETTINGS_PAGES
        .get(state.settings_nav as usize)
        .copied()
        .unwrap_or("Themes");
    let title_h = u16::from(page.height > 0);
    let title_area = Rect {
        x: page.x,
        y: page.y,
        width: page.width,
        height: title_h,
    };
    let content = Rect {
        x: page.x,
        y: page.y.saturating_add(title_h),
        width: page.width,
        height: page.height.saturating_sub(title_h),
    };
    if title_h > 0 {
        let shown: String = section.chars().take(page.width as usize).collect();
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                shown,
                Style::default()
                    .fg(state.theme.colors.modal_header)
                    .add_modifier(Modifier::BOLD),
            )))
            .style(state.theme.modal),
            title_area,
        );
    }

    if state.settings_nav == SETTINGS_HELP {
        let lines = help::lines(state, content.width.max(1) as usize);
        paint_lines(f, state, content, lines, None);
        return;
    }
    paint_themes(f, state, content);
}

fn paint_nav(f: &mut Frame, state: &AppState, nav: Rect) {
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(state.theme.border)
        .style(state.theme.body);
    let inner = block.inner(nav);
    f.render_widget(block, nav);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let width = inner.width as usize;
    let mut lines = Vec::new();
    for (index, label) in SETTINGS_PAGES.iter().enumerate() {
        let on = state.settings_nav == index as u8;
        let marker = if on { ">" } else { " " };
        let text: String = format!("{marker} {label}").chars().take(width).collect();
        let style = if on {
            state.theme.selected
        } else {
            state.theme.modal_text
        };
        lines.push(Line::from(Span::styled(text, style)));
    }
    f.render_widget(Paragraph::new(lines).style(state.theme.body), inner);
}

fn paint_themes(f: &mut Frame, state: &mut AppState, content: Rect) {
    let editor = state.theme_editor.clone();
    let picker = state.theme_picker.clone();
    let box_w = (content.width as usize).max(2);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut selected_line = None;
    let rule = Style::default().fg(state.theme.colors.border_default);
    let header = Style::default()
        .fg(state.theme.colors.modal_header)
        .add_modifier(Modifier::BOLD);
    let hint = if let Some(editor) = &editor {
        let target = editor.save_target.label();
        if picker.is_some() {
            format!("Tab section   j/k move   t target   y save {target}   R reset   Esc back")
        } else {
            format!("Tab section   j/k move   t target   y save {target}   R reset   Esc revert")
        }
    } else {
        "Tab section   j/k move   Enter apply   e colors   y save   t target   Esc revert"
            .to_string()
    };
    let hint_style = Style::default().fg(state.theme.colors.text_secondary);
    lines.extend(wrap_hint(&hint, content.width as usize, hint_style));

    if let Some(editor) = &editor {
        if editor.editing_hex {
            lines.push(Line::from(format!("Hex: {}█", editor.hex_draft)));
        } else {
            lines.push(Line::from(format!(
                "Hex: {}   Enter edit",
                editor.hex_draft
            )));
        }
        let mut open = false;
        for row in theme_editor_rows(&editor.fields) {
            match row {
                ThemeEditorRow::Header(name) => {
                    if open {
                        lines.push(settings_bottom(box_w, rule));
                    }
                    lines.push(Line::from(""));
                    lines.push(settings_top(name, box_w, rule, header));
                    open = true;
                }
                ThemeEditorRow::Color(idx) => {
                    if idx == editor.selected {
                        selected_line = Some(lines.len());
                    }
                    let field = editor.fields[idx];
                    let hex = editor
                        .palette
                        .get(field.key)
                        .map(|color| color.to_hex())
                        .unwrap_or_else(|| "#000000".into());
                    let cursor = if idx == editor.selected { ">" } else { " " };
                    let style = if idx == editor.selected {
                        state.theme.selected
                    } else {
                        state.theme.modal_text
                    };
                    lines.push(settings_row(
                        &format!("{cursor} {:<22} {hex}", field.key),
                        box_w,
                        rule,
                        style,
                    ));
                }
            }
        }
        if open {
            lines.push(settings_bottom(box_w, rule));
        }
    } else if let Some(picker) = &picker {
        lines.push(Line::from(""));
        lines.push(settings_top("Themes", box_w, rule, header));
        for (idx, row) in picker.rows.iter().enumerate() {
            if idx == picker.selected {
                selected_line = Some(lines.len());
            }
            let (label, detail) = match row {
                PickerRow::Follow { name } => ("Follow desktop".to_string(), name.clone()),
                PickerRow::Theme(theme) => (theme.name.clone(), theme.mode.label().to_string()),
                PickerRow::Custom => ("Custom".to_string(), "built-in".to_string()),
            };
            let cursor = if idx == picker.selected { ">" } else { " " };
            let style = if idx == picker.selected {
                state.theme.selected
            } else {
                state.theme.modal_text
            };
            lines.push(settings_row(
                &format!("{cursor} {label:<22} {detail}"),
                box_w,
                rule,
                style,
            ));
        }
        lines.push(settings_bottom(box_w, rule));
    }

    paint_lines(f, state, content, lines, selected_line);
}

fn paint_lines(
    f: &mut Frame,
    state: &mut AppState,
    area: Rect,
    lines: Vec<Line<'static>>,
    selected_line: Option<usize>,
) {
    if area.width == 0 || area.height == 0 {
        state.help_scroll_max = 0;
        state.help_scroll = 0;
        return;
    }
    let view_h = area.height as usize;
    let max_scroll = lines.len().saturating_sub(view_h) as u16;
    state.help_scroll_max = max_scroll;
    if state.settings_follow {
        if let Some(sel) = selected_line {
            let top = state.help_scroll as usize;
            if sel < top {
                state.help_scroll = sel as u16;
            } else if sel >= top + view_h {
                state.help_scroll = (sel + 1 - view_h) as u16;
            }
        }
        state.settings_follow = false;
    }
    if state.help_scroll > max_scroll {
        state.help_scroll = max_scroll;
    }
    f.render_widget(
        Paragraph::new(lines)
            .style(state.theme.modal)
            .scroll((state.help_scroll, 0)),
        area,
    );
}

/// Break the key hint on the triple-space gaps so each phrase stays intact.
fn wrap_hint(hint: &str, width: usize, style: Style) -> Vec<Line<'static>> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut current = String::new();
    for part in hint.split("   ").filter(|part| !part.is_empty()) {
        if current.is_empty() {
            current = part.to_string();
            continue;
        }
        let next = current.chars().count() + 3 + part.chars().count();
        if next > width {
            lines.push(Line::from(Span::styled(std::mem::take(&mut current), style)));
            current = part.to_string();
        } else {
            current.push_str("   ");
            current.push_str(part);
        }
    }
    if !current.is_empty() {
        lines.push(Line::from(Span::styled(current, style)));
    }
    if lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines
}

fn settings_top(title: &str, width: usize, edge: Style, title_style: Style) -> Line<'static> {
    let width = width.max(2);
    let inner = width - 2;
    let mut label = format!(" {title} ");
    if label.chars().count() > inner {
        label = label.chars().take(inner).collect();
    }
    let pad = inner - label.chars().count();
    Line::from(vec![
        Span::styled("┌", edge),
        Span::styled(label, title_style),
        Span::styled(format!("{}┐", "─".repeat(pad)), edge),
    ])
}

fn settings_bottom(width: usize, edge: Style) -> Line<'static> {
    let width = width.max(2);
    Line::from(Span::styled(format!("└{}┘", "─".repeat(width - 2)), edge))
}

fn settings_row(body: &str, width: usize, edge: Style, fill: Style) -> Line<'static> {
    let width = width.max(2);
    let inner = width - 2;
    let mut text: String = body.chars().take(inner).collect();
    let pad = inner - text.chars().count();
    text.push_str(&" ".repeat(pad));
    Line::from(vec![
        Span::styled("│", edge),
        Span::styled(text, fill),
        Span::styled("│", edge),
    ])
}
