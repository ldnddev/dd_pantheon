use crate::state::AppState;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub fn draw(f: &mut Frame, state: &AppState, area: Rect, scroll: u16) {
    let mut lines: Vec<Line> = Vec::new();
    push_header(&mut lines, state, "Global");
    lines.push(line("F1", "Help"));
    lines.push(line("F2", "Theme editor (Tab local/global, Y save)"));
    lines.push(line("F3", "Doctor (refresh via job runner)"));
    lines.push(line("Ctrl+Q", "Quit"));
    lines.push(line("Tab / S-Tab", "Cycle panes"));
    lines.push(line("Esc", "Close modal / clear filter"));
    lines.push(line(
        "/",
        "Filter sites by name or tag (modal list; Enter jumps)",
    ));
    lines.push(line(
        ":",
        "Terminus/Lando commands as selected (type `lando` to filter; non-init Lando is grayed without a Pantheon .lando.yml)",
    ));
    lines.push(line("Ctrl+K", "Palette"));
    lines.push(line("Ctrl+L", "Login (machine token)"));
    lines.push(line("Actions", "Logout when logged in (Mutating)"));
    lines.push(line("Ctrl+C", "Cancel running job"));

    push_header(&mut lines, state, "Tree");
    lines.push(line("j/k ↑↓", "Move"));
    lines.push(line("h/l", "Expand / collapse"));
    lines.push(line("g / G", "Top / bottom"));
    lines.push(line("Enter", "Expand site / keep env selected"));
    lines.push(line("r", "Refresh inventory + metrics + backups (live)"));
    lines.push(line("b", "Stage backup:create (env row; Enter runs)"));
    lines.push(line(
        ":",
        "Restore backup / backup URL via palette (get prints URL, no download)",
    ));
    lines.push(line(
        "e",
        "Deploy: git-mode on dev; test/live open a --note form (sync-content on test)",
    ));
    lines.push(line(
        "c",
        "Stage env:clear-cache (tree/inspector; not preview)",
    ));
    lines.push(line("s / S", "lando start / stop (local bound)"));
    lines.push(line("/", "Filter sites (name / tag)"));
    lines.push(line("T", "Tag filter picker (again clears)"));
    lines.push(line("a", "Add tag"));
    lines.push(line("n", "Create site wizard (org + name + upstream)"));
    lines.push(line(
        "(no W)",
        "Wipe is palette-only (backup-first; live is LiveGate)",
    ));
    lines.push(line(
        ":",
        "domains / HTTPS / lock via palette (password redacted)",
    ));
    lines.push(line("x", "Remove selected tag chip (inspector)"));
    lines.push(line("[ / ]", "Cycle tag chips (inspector)"));

    push_header(&mut lines, state, "Inspector");
    lines.push(line("j/k", "Scroll inspector"));
    lines.push(line("Enter", "Run the staged plan (same as preview)"));
    lines.push(line(
        "d / w / Shift+M",
        "Metrics period (env row; m is CMS)",
    ));
    lines.push(line("m", "CMS form (remote/local; never month)"));

    push_header(&mut lines, state, "Preview / log");
    lines.push(line(
        "Enter",
        "PROCESS COMMAND — run plan (gates apply; --demo never spawns)",
    ));
    lines.push(line(
        "y",
        "COPY TO CLIPBOARD (redacted argv; preview focus)",
    ));
    lines.push(line(
        "Esc",
        "CANCEL staged plan when filter is already clear",
    ));
    lines.push(line("c", "no-op in preview (not copy, not clear-cache)"));
    lines.push(line("j/k", "Scroll"));
    lines.push(line(
        "Enter / e",
        "Expand job log to a full-size modal (Esc closes)",
    ));
    lines.push(line("g / G", "Log top / bottom"));
    lines.push(line("PgUp / PgDn", "Log page"));
    lines.push(line(
        "click",
        "Process / Copy / Cancel buttons in command preview",
    ));
    lines.push(line("click log", "Focus; click again to expand"));

    push_header(&mut lines, state, "Mouse");
    lines.push(line(
        "click",
        "Focus pane / select tree row / period labels",
    ));
    lines.push(line("wheel", "Scroll hovered pane"));

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
        .block(
            Block::default()
                .title("F1 Help")
                .borders(Borders::ALL)
                .border_style(state.theme.active_border)
                .style(state.theme.modal),
        )
        .style(state.theme.modal_text);
    f.render_widget(p, area);
}

fn push_header(lines: &mut Vec<Line>, state: &AppState, title: &str) {
    if !lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        title.to_string(),
        state.theme.modal_header,
    )));
}

fn line<'a>(key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![Span::raw(format!("  {key:<16}")), Span::raw(desc)])
}
