use crate::state::{AppState, FocusPane, TreeSel};
use crate::ui::loader;
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn draw_header(f: &mut Frame, state: &AppState, area: Rect) {
    let mut title = ">_ dd_pantheon".to_string();
    if state.demo && area.width >= 40 {
        title.push_str("  DEMO");
    }
    let mut title_spans = vec![Span::raw(title)];
    let busy = loader::spans(state);
    if !busy.is_empty() {
        title_spans.push(Span::raw(" "));
        title_spans.extend(busy);
    }
    let block = Block::default()
        .title(Line::from(title_spans))
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
    if state.job_running {
        draw_busy_footer(f, state, area);
    }
}

fn draw_busy_footer(f: &mut Frame, state: &AppState, area: Rect) {
    let mut spans = loader::spans(state);
    if spans.is_empty() {
        return;
    }
    if let Some(name) = live_job_label(state, area.width) {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            name,
            Style::default().fg(state.theme.colors.info),
        ));
    }
    spans.push(Span::raw(" "));
    f.render_widget(
        Paragraph::new(Line::from(spans))
            .alignment(Alignment::Right)
            .style(Style::default().bg(state.theme.colors.base_background)),
        area,
    );
}

/// Job name beside the footer mark. Clipped so the left key hint stays visible.
fn live_job_label(state: &AppState, width: u16) -> Option<String> {
    let job = state
        .jobs
        .values()
        .filter(|job| job.status.is_live())
        .min_by_key(|job| job.started_at)?;
    let name = job.plan.why.trim();
    let name = if name.is_empty() {
        job.plan.argv.first().map(|arg| arg.trim()).unwrap_or("")
    } else {
        name
    };
    if name.is_empty() {
        return None;
    }
    let reserve: usize = if width < 80 { 13 } else { 16 };
    let room = (width as usize).saturating_sub(reserve.saturating_add(9));
    let clipped = clip_label(name, room);
    if clipped.is_empty() { None } else { Some(clipped) }
}

fn clip_label(name: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if name.width() <= max {
        return name.to_string();
    }
    let mut out = String::new();
    for ch in name.chars() {
        let next = out.width() + ch.width().unwrap_or(0);
        if next + 1 > max {
            break;
        }
        out.push(ch);
    }
    if out.width() + 1 <= max {
        out.push('…');
    }
    out
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
        None if state.show_settings => ModalChrome::Generic,
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
    if let ModalChrome::LiveGate { expected, matched } = modal {
        // F1 and F2 do not open while the gate is collecting the word.
        let mut raw = livegate_footer(width, expected, matched);
        if job_running {
            raw.push_str("   Ctrl+C: Cancel");
        }
        return truncate_from_right(&raw, width as usize);
    }
    if matches!(modal, ModalChrome::Generic) {
        let line = if width < 80 {
            "F1:Settings  Esc:Close  ^Q:Quit"
        } else {
            "F1: Settings  Esc: Close  Ctrl+Q: Quit"
        };
        return truncate_from_right(line, width as usize);
    }
    idle_footer(width, focus, job_running)
}

fn idle_footer(width: u16, focus: FocusPane, job_running: bool) -> String {
    let narrow = width < 80;
    let (left, right) = if narrow {
        ("F1:Settings", "^Q:Quit")
    } else {
        ("F1: Settings", "Ctrl+Q: Quit")
    };
    let mut middle: Vec<&str> = Vec::new();
    if !narrow && width >= 120 {
        middle.push("F3: Doctor");
    }
    if narrow {
        middle.extend_from_slice(pane_tokens_narrow(focus));
    } else if width < 120 {
        middle.extend_from_slice(pane_tokens(focus, false));
    } else {
        middle.extend_from_slice(pane_tokens(focus, true));
    }
    if job_running {
        middle.push("Ctrl+C: Cancel");
    }
    fit_footer(width, left, &middle, right)
}

fn fit_footer(width: u16, left: &str, middle: &[&str], right: &str) -> String {
    let limit = width as usize;
    let mut mid: Vec<&str> = middle.to_vec();
    loop {
        let mut parts = Vec::with_capacity(mid.len() + 2);
        parts.push(left);
        parts.extend(mid.iter().copied());
        parts.push(right);
        let line = parts.join("  ");
        if line.width() <= limit || mid.is_empty() {
            return truncate_from_right(&line, limit);
        }
        mid.pop();
    }
}

fn pane_tokens_narrow(focus: FocusPane) -> &'static [&'static str] {
    match focus {
        FocusPane::Tree => &["j/k:Nav", "/:Filter"],
        FocusPane::Inspector => &["j/k:Scroll", "Enter:Run"],
        FocusPane::Preview => &["Enter:Process", "y:Copy"],
        FocusPane::Log => &["j/k:Scroll", "Enter:Expand"],
    }
}

fn pane_tokens(focus: FocusPane, wide: bool) -> &'static [&'static str] {
    match (wide, focus) {
        (false, FocusPane::Tree) => &["j/k: Nav", "Enter: Run", "/: Filter", ":: Pal"],
        (false, FocusPane::Inspector) => &["j/k: Scroll", "Enter: Run", "m: CMS"],
        (false, FocusPane::Preview) => &["Enter: Process", "y: Copy", "Esc: Unstage"],
        (false, FocusPane::Log) => &["j/k: Scroll", "Enter: Expand", "G: Follow"],
        (true, FocusPane::Tree) => &[
            "j/k: Nav",
            "Tab: Pane",
            "Enter: Run",
            "/: Filter",
            ":: Palette",
            "r: Refresh",
            "(mouse: click/scroll)",
        ],
        (true, FocusPane::Inspector) => &[
            "j/k: Scroll",
            "Enter: Run",
            "m: CMS",
            "[ ]: Tags",
            "d/w/M: Period",
            "(mouse: click/scroll)",
        ],
        (true, FocusPane::Preview) => &[
            "Enter: Process",
            "y: Copy",
            "Esc: Unstage",
            "click Process/Copy/Cancel",
        ],
        (true, FocusPane::Log) => &[
            "j/k: Scroll",
            "Enter: Expand",
            "g/G: Top/Follow",
            "PgUp/PgDn",
            "(mouse: click/scroll)",
        ],
    }
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
        let s = truncate_from_right("F1:Settings  ^Q:Quit", 12);
        assert_eq!(s, "F1:Settings ");
        assert!(s.starts_with("F1:Settings"));
        assert_eq!(s.width(), 12);
    }

    #[test]
    fn idle_footer_starts_with_settings_and_keeps_quit() {
        let narrow = footer_keys(70);
        assert!(narrow.starts_with("F1:Settings"), "{narrow}");
        assert!(narrow.contains("^Q:Quit"), "{narrow}");
        assert!(narrow.contains("j/k:Nav"), "{narrow}");
        assert!(!narrow.contains("F1:Help") && !narrow.contains("F2"), "{narrow}");

        let medium = footer_keys(100);
        assert!(medium.starts_with("F1: Settings"), "{medium}");
        assert!(medium.contains("Ctrl+Q: Quit"), "{medium}");
        assert!(!medium.contains("F3"), "{medium}");
        let hint = medium.find("j/k").expect("pane hint");
        let quit = medium.find("Ctrl+Q: Quit").expect("quit");
        assert!(hint < quit, "{medium}");

        let wide = footer_keys(140);
        assert!(wide.starts_with("F1: Settings"), "{wide}");
        assert!(wide.contains("F3: Doctor"), "{wide}");
        assert!(wide.contains("Ctrl+Q: Quit"), "{wide}");
        let f3 = wide.find("F3: Doctor").unwrap();
        let hint = wide.find("j/k").unwrap();
        let quit = wide.find("Ctrl+Q: Quit").unwrap();
        assert!(f3 < hint && hint < quit, "{wide}");
    }

    #[test]
    fn sheet_and_modal_footer_names_f1_esc_and_quit() {
        let mut state = demo_state();
        state.show_settings = true;
        assert_eq!(footer_line(70, &state), "F1:Settings  Esc:Close  ^Q:Quit");
        assert_eq!(
            footer_line(100, &state),
            "F1: Settings  Esc: Close  Ctrl+Q: Quit"
        );

        state.show_settings = false;
        state.modal = Some(crate::state::Modal::Doctor { scroll: 0 });
        assert_eq!(footer_line(64, &state), "F1:Settings  Esc:Close  ^Q:Quit");
        assert_eq!(
            footer_line(90, &state),
            "F1: Settings  Esc: Close  Ctrl+Q: Quit"
        );
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
