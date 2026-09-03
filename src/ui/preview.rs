use crate::plan::SafetyTier;
use crate::state::{AppState, FocusPane};
use crate::ui::pane_block;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

pub fn draw(f: &mut Frame, state: &AppState, area: Rect) {
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
        Span::raw("Preview  "),
        Span::styled(badge.badge(), badge_style),
        Span::raw(format!("  target {target}{step}")),
    ]);

    let mut lines = vec![
        Line::from(Span::styled(
            format!("$ {shell}"),
            Style::default().fg(state.theme.colors.text_primary),
        )),
        Line::from(vec![
            Span::styled("cwd: ", state.theme.label),
            Span::styled(cwd, state.theme.secondary),
        ]),
        Line::from(vec![
            Span::styled("why: ", state.theme.label),
            if why.contains("raw palette") {
                Span::styled(why, state.theme.warning_style)
            } else {
                Span::raw(why)
            },
        ]),
    ];
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

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((state.preview_scroll, 0))
        .block(pane_block(title, focused, &state.theme));
    f.render_widget(p, area);
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
}
