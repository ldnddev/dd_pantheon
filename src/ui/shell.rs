use crate::state::{AppState, FocusPane, TreeSel};
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
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(state.theme.active_border)
        .style(state.theme.app_shell);
    let inner = block.inner(area);
    f.render_widget(Paragraph::new("").block(block), area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let status = header_status(state);
    let status_w = status.width() as u16;
    let quote_budget = inner.width.saturating_sub(if status_w == 0 {
        0
    } else {
        status_w.saturating_add(1)
    }) as usize;
    let quote = truncate_from_right(&state.header_copy, quote_budget);
    f.render_widget(Paragraph::new(quote).style(state.theme.app_shell), inner);
    if status_w > 0 && inner.width >= status_w {
        let rect = Rect::new(
            inner.x + inner.width.saturating_sub(status_w),
            inner.y + inner.height.saturating_sub(1) / 2,
            status_w,
            1,
        );
        let style = if state.job_running {
            state.theme.warning_style
        } else {
            state.theme.secondary
        };
        f.render_widget(
            Paragraph::new(Span::styled(status, style)).style(state.theme.app_shell),
            rect,
        );
    }
}

fn header_status(state: &AppState) -> String {
    let mut parts: Vec<String> = Vec::new();
    match &state.selected {
        TreeSel::Env { site, env } => parts.push(format!("{site}.{env}")),
        TreeSel::Site(site) => parts.push(site.clone()),
        TreeSel::None => {}
    }
    if let Some(frame) = loader::current(state) {
        parts.push(frame.to_string());
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" {} ", parts.join("  "))
    }
}

pub fn draw_footer(f: &mut Frame, state: &AppState, area: Rect) {
    let keys = footer_line(area.width, state);
    let bar = Paragraph::new(Line::from(keys)).style(state.theme.app_shell);
    f.render_widget(bar, area);
}

pub fn footer_keys(width: u16) -> String {
    footer_line_raw(width, FocusPane::Tree, ModalChrome::None, false)
}

fn footer_line(width: u16, state: &AppState) -> String {
    let chrome = match &state.modal {
        Some(crate::state::Modal::LiveGate {
            expected, typed, ..
        }) => ModalChrome::LiveGate {
            expected,
            matched: !expected.is_empty() && typed == expected,
        },
        Some(_) => ModalChrome::Generic,
        None => ModalChrome::None,
    };
    footer_line_raw(width, state.focus, chrome, state.job_running)
}

#[derive(Clone, Copy)]
enum ModalChrome<'a> {
    None,
    Generic,
    LiveGate { expected: &'a str, matched: bool },
}

fn footer_line_raw(
    width: u16,
    focus: FocusPane,
    modal: ModalChrome<'_>,
    job_running: bool,
) -> String {
    let mut raw = if let ModalChrome::LiveGate { expected, matched } = modal {
        // F1 and F2 do not open while the gate is collecting the word.
        livegate_footer(width, expected, matched)
    } else if width < 80 {
        "F1:Help  F2:Theme  C-q:Quit  /:Filter".to_string()
    } else if width < 120 {
        format!(
            "F1: Help   F2: Theme   Ctrl+Q: Quit   {}",
            pane_hint_medium(focus, modal, job_running)
        )
    } else {
        format!(
            "F1: Help   F2: Theme   Ctrl+Q: Quit   F3: Doctor   {}",
            pane_hint_wide(focus, modal, job_running)
        )
    };
    if job_running && matches!(modal, ModalChrome::LiveGate { .. }) {
        raw.push_str("   Ctrl+C: Cancel");
    }
    truncate_from_right(&raw, width as usize)
}

fn livegate_footer(width: u16, expected: &str, matched: bool) -> String {
    let word = if expected.is_empty() {
        "the word"
    } else {
        expected
    };
    if width < 80 {
        if matched {
            "Esc:Cancel  Enter:Run  C-q:Quit".to_string()
        } else {
            format!("Esc:Cancel  type {word}  C-q:Quit")
        }
    } else if matched {
        "Esc: Cancel   Enter: Run   Ctrl+Q: Quit".to_string()
    } else {
        let full =
            format!("Esc: Cancel   type {word}   Enter runs only when it matches   Ctrl+Q: Quit");
        if full.width() as u16 <= width {
            full
        } else {
            format!("Esc: Cancel   type {word}   Ctrl+Q: Quit")
        }
    }
}

fn pane_hint_medium(focus: FocusPane, modal: ModalChrome<'_>, job_running: bool) -> String {
    let body = match modal {
        ModalChrome::Generic => "Esc: Close   Enter: Confirm".to_string(),
        ModalChrome::LiveGate { .. } => String::new(),
        ModalChrome::None => match focus {
            FocusPane::Tree => "j/k: Nav   Enter: Run   /: Filter   :: Pal".to_string(),
            FocusPane::Inspector => "j/k: Scroll   Enter: Run   m: CMS".to_string(),
            FocusPane::Preview => "Enter: Process   y: Copy   Esc: Unstage".to_string(),
            FocusPane::Log => "j/k: Scroll   Enter: Expand   G: Follow".to_string(),
        },
    };
    if job_running {
        format!("{body}   Ctrl+C: Cancel")
    } else {
        body
    }
}

fn pane_hint_wide(focus: FocusPane, modal: ModalChrome<'_>, job_running: bool) -> String {
    let body = match modal {
        ModalChrome::Generic => {
            "Esc: Close   Enter: Confirm   click outside to dismiss".to_string()
        }
        ModalChrome::LiveGate { .. } => String::new(),
        ModalChrome::None => match focus {
            FocusPane::Tree => {
                "j/k: Nav   Tab: Pane   Enter: Run   /: Filter   :: Palette   r: Refresh   (mouse: click/scroll)"
                    .to_string()
            }
            FocusPane::Inspector => {
                "j/k: Scroll   Enter: Run   m: CMS   [ ]: Tags   d/w/M: Period   (mouse: click/scroll)"
                    .to_string()
            }
            FocusPane::Preview => {
                "Enter: Process   y: Copy   Esc: Unstage   click Process/Copy/Cancel".to_string()
            }
            FocusPane::Log => {
                "j/k: Scroll   Enter: Expand   g/G: Top/Follow   PgUp/PgDn   (mouse: click/scroll)"
                    .to_string()
            }
        },
    };
    if job_running {
        format!("{body}   Ctrl+C: Cancel")
    } else {
        body
    }
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

    #[test]
    fn livegate_footer_hides_enter_until_the_word_matches() {
        let unmatched = footer_line_raw(
            140,
            FocusPane::Tree,
            ModalChrome::LiveGate {
                expected: "live",
                matched: false,
            },
            false,
        );
        assert!(unmatched.contains("type live"), "{unmatched}");
        assert!(
            unmatched.contains("Enter runs only when it matches"),
            "{unmatched}"
        );
        assert!(unmatched.contains("Esc: Cancel"), "{unmatched}");
        assert!(!unmatched.contains("Enter: Confirm"), "{unmatched}");
        assert!(!unmatched.contains("Enter: Run"), "{unmatched}");
        assert!(!unmatched.contains("click outside"), "{unmatched}");
        assert!(!unmatched.contains("F1"), "{unmatched}");

        let matched = footer_line_raw(
            140,
            FocusPane::Preview,
            ModalChrome::LiveGate {
                expected: "database",
                matched: true,
            },
            false,
        );
        assert!(matched.contains("Enter: Run"), "{matched}");
        assert!(!matched.contains("Enter: Confirm"), "{matched}");
        assert!(!matched.contains("type database"), "{matched}");
    }

    #[test]
    fn livegate_footer_reads_the_open_modal() {
        let mut state = demo_state();
        state.modal = Some(crate::state::Modal::LiveGate {
            plan: gate_plan(),
            expected: "live".into(),
            typed: "li".into(),
        });
        let line = footer_line(140, &state);
        assert!(line.contains("type live"), "{line}");
        assert!(!line.contains("Enter: Confirm"), "{line}");

        state.modal = Some(crate::state::Modal::LiveGate {
            plan: gate_plan(),
            expected: "live".into(),
            typed: "live".into(),
        });
        let ready = footer_line(140, &state);
        assert!(ready.contains("Enter: Run"), "{ready}");
    }

    fn demo_state() -> AppState {
        let dir = std::env::temp_dir().join(format!("dd_pantheon_footer_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        AppState::from_demo(
            crate::theme::Theme::default(),
            crate::config::ConfigStore::load(&dir),
            crate::fixtures::demo_data(),
        )
    }

    fn gate_plan() -> crate::plan::CommandPlan {
        crate::plan::CommandPlan {
            tool: crate::plan::ToolKind::Terminus,
            binary: std::path::PathBuf::from("terminus"),
            argv: vec!["env:deploy".into(), "acme-wp.live".into()],
            cwd: None,
            why: "deploy code to acme-wp.live".into(),
            safety: crate::plan::SafetyTier::LiveGate,
            target: crate::plan::PlanTarget::Env {
                site: "acme-wp".into(),
                env: "live".into(),
            },
            dry_run: false,
            timeout: None,
            expects_json: false,
            extra_env: Vec::new(),
            redact: Vec::new(),
            confirm_with_yes: true,
        }
    }
}
