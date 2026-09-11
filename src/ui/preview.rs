use crate::plan::SafetyTier;
use crate::state::{AppState, FocusPane, PreviewButton, PreviewButtonHit};
use crate::ui::pane_block;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub fn draw(f: &mut Frame, state: &mut AppState, area: Rect) {
    state.preview_buttons.clear();
    let focused = state.focus == FocusPane::Preview;
    let (badge, target, why, shell, cwd) = match &state.current {
        Some(plan) => {
            let current = plan.current();
            (
                plan.safety(),
                plan.target_label(),
                plan.why().to_string(),
                current.map(|p| p.redacted_shell_line()).unwrap_or_default(),
                current
                    .and_then(|p| p.cwd.as_ref().map(|c| c.display().to_string()))
                    .unwrap_or_else(|| "(none)".into()),
            )
        }
        None => (
            SafetyTier::ReadOnly,
            "—".into(),
            "no plan staged".into(),
            String::new(),
            "(none)".into(),
        ),
    };

    let badge_style = match badge {
        SafetyTier::ReadOnly => state.theme.info,
        SafetyTier::Mutating => state.theme.warning_style,
        SafetyTier::Destructive | SafetyTier::LiveGate => {
            state.theme.error.add_modifier(Modifier::BOLD)
        }
    };
    let step = state
        .current
        .as_ref()
        .and_then(workflow_step_label)
        .map(|s| format!("  {s}"))
        .unwrap_or_default();
    let title = Line::from(vec![
        Span::raw("Command preview  "),
        Span::styled(badge.badge(), badge_style),
        Span::raw(format!("  {target}{step}")),
    ]);

    let block = pane_block(title, focused, &state.theme);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }

    if inner.height >= 7 {
        draw_roomy(f, state, inner, &shell, &why, &cwd);
    } else {
        draw_compact(f, state, inner, &shell, &why, &cwd);
    }
}

fn draw_roomy(f: &mut Frame, state: &mut AppState, area: Rect, shell: &str, why: &str, cwd: &str) {
    let cmd_h = if area.height >= 9 { 4 } else { 3 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(cmd_h),
            Constraint::Min(2),
            Constraint::Length(3),
        ])
        .split(area);
    draw_command_bar(f, state, chunks[0], shell);
    draw_rationale(f, state, chunks[1], why, cwd);
    draw_buttons(f, state, chunks[2]);
}

fn draw_compact(
    f: &mut Frame,
    state: &mut AppState,
    area: Rect,
    shell: &str,
    why: &str,
    cwd: &str,
) {
    let mut lines = vec![
        Line::from(Span::styled(
            format!("$ {shell}"),
            Style::default().fg(state.theme.colors.text_primary),
        )),
        Line::from(vec![
            Span::styled("cwd: ", state.theme.label),
            Span::styled(cwd.to_string(), state.theme.secondary),
        ]),
        Line::from(vec![
            Span::styled("rationale: ", state.theme.label),
            rationale_span(state, why),
        ]),
    ];
    push_workflow_steps(&mut lines, state);
    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((state.preview_scroll, 0));
    f.render_widget(p, area);
}

fn draw_command_bar(f: &mut Frame, state: &AppState, area: Rect, shell: &str) {
    let style = Style::default()
        .fg(state.theme.colors.text_primary)
        .bg(state.theme.colors.selected_background);
    let bar = Block::default()
        .borders(Borders::ALL)
        .border_style(state.theme.border)
        .style(style);
    let inner = bar.inner(area);
    f.render_widget(bar, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let cmd = if shell.is_empty() {
        "$  (no plan staged)".to_string()
    } else {
        format!("$ {shell}")
    };
    f.render_widget(
        Paragraph::new(cmd).style(style).wrap(Wrap { trim: false }),
        inner,
    );
}

fn draw_rationale(f: &mut Frame, state: &AppState, area: Rect, why: &str, cwd: &str) {
    let mut lines = vec![
        Line::from(Span::styled("rationale", state.theme.label)),
        Line::from(rationale_span(state, why)),
    ];
    if cwd != "(none)" {
        lines.push(Line::from(vec![
            Span::styled("cwd: ", state.theme.label),
            Span::styled(cwd.to_string(), state.theme.secondary),
        ]));
    }
    push_workflow_steps(&mut lines, state);
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .scroll((state.preview_scroll, 0)),
        area,
    );
}

const BUTTON_GAP: u16 = 1;
const BUTTON_PAD: u16 = 1;

fn button_width(label: &str) -> u16 {
    // left/right border + one space of padding each side + label
    (label.chars().count() as u16)
        .saturating_add(BUTTON_PAD.saturating_mul(2))
        .saturating_add(2)
}

fn outline_button(label: &str, fg: Color) -> Paragraph<'static> {
    let pad = " ".repeat(BUTTON_PAD as usize);
    let color = Style::default().fg(fg);
    Paragraph::new(format!("{pad}{label}{pad}"))
        .alignment(ratatui::layout::Alignment::Center)
        .style(color)
        .block(Block::default().borders(Borders::ALL).border_style(color))
}

fn draw_buttons(f: &mut Frame, state: &mut AppState, area: Rect) {
    let process_w = button_width("PROCESS COMMAND");
    let copy_w = button_width("COPY TO CLIPBOARD");
    let cancel_w = button_width("CANCEL");
    let needed = process_w
        .saturating_add(copy_w)
        .saturating_add(cancel_w)
        .saturating_add(BUTTON_GAP.saturating_mul(2));
    let (process_w, copy_w, cancel_w) = if needed > area.width && area.width > 0 {
        let scale = area.width as f64 / needed as f64;
        (
            ((process_w as f64 * scale).floor() as u16).max(3),
            ((copy_w as f64 * scale).floor() as u16).max(3),
            ((cancel_w as f64 * scale).floor() as u16).max(3),
        )
    } else {
        (process_w, copy_w, cancel_w)
    };
    let row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(process_w),
            Constraint::Length(BUTTON_GAP),
            Constraint::Length(copy_w),
            Constraint::Length(BUTTON_GAP),
            Constraint::Length(cancel_w),
            Constraint::Min(0),
        ])
        .split(area);

    f.render_widget(
        outline_button("PROCESS COMMAND", state.theme.colors.success),
        row[0],
    );
    state.preview_buttons.push(PreviewButtonHit {
        button: PreviewButton::Process,
        area: row[0],
    });

    f.render_widget(
        outline_button("COPY TO CLIPBOARD", state.theme.colors.warning),
        row[2],
    );
    state.preview_buttons.push(PreviewButtonHit {
        button: PreviewButton::Copy,
        area: row[2],
    });

    f.render_widget(outline_button("CANCEL", state.theme.colors.error), row[4]);
    state.preview_buttons.push(PreviewButtonHit {
        button: PreviewButton::Cancel,
        area: row[4],
    });
}

fn rationale_span(state: &AppState, why: &str) -> Span<'static> {
    if why.contains("raw palette") {
        Span::styled(why.to_string(), state.theme.warning_style)
    } else {
        Span::raw(why.to_string())
    }
}

fn push_workflow_steps(lines: &mut Vec<Line<'static>>, state: &AppState) {
    if let Some(crate::plan::StagedPlan::Workflow { plan, step }) = &state.current {
        for (i, p) in plan.steps.iter().enumerate() {
            let style = if i == *step {
                state.theme.active_label
            } else {
                state.theme.secondary
            };
            lines.push(Line::from(Span::styled(
                format!("  {}: {}", i + 1, p.redacted_shell_line()),
                style,
            )));
        }
    }
}

pub fn workflow_step_label(plan: &crate::plan::StagedPlan) -> Option<String> {
    match plan {
        crate::plan::StagedPlan::Workflow { plan, step } if !plan.steps.is_empty() => {
            Some(format!("step {}/{}", step + 1, plan.steps.len()))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{CommandPlan, PlanTarget, StagedPlan, ToolKind, WorkflowPlan};
    use std::path::PathBuf;

    fn dummy(cmd: &str) -> CommandPlan {
        CommandPlan {
            tool: ToolKind::Terminus,
            binary: PathBuf::from("terminus"),
            argv: vec![cmd.into()],
            cwd: None,
            why: "t".into(),
            safety: SafetyTier::Mutating,
            target: PlanTarget::None,
            dry_run: true,
            timeout: None,
            expects_json: false,
            extra_env: vec![],
            redact: vec![],
            confirm_with_yes: false,
        }
    }

    #[test]
    fn workflow_title_includes_step() {
        let staged = StagedPlan::Workflow {
            plan: WorkflowPlan {
                title: "w".into(),
                why: "w".into(),
                safety: SafetyTier::Destructive,
                steps: vec![dummy("backup:create"), dummy("env:wipe")],
                stop_on_failure: true,
            },
            step: 0,
        };
        assert_eq!(workflow_step_label(&staged).as_deref(), Some("step 1/2"));
    }

    #[test]
    fn buttons_share_padding_and_width_formula() {
        assert_eq!(button_width("PROCESS COMMAND"), 15 + 2 + 2);
        assert_eq!(button_width("COPY TO CLIPBOARD"), 17 + 2 + 2);
        assert_eq!(button_width("CANCEL"), 6 + 2 + 2);
        assert_eq!(BUTTON_GAP, 1);
        assert_eq!(BUTTON_PAD, 1);
    }
}
