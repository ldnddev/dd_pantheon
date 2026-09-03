use crate::state::AppState;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub fn draw(f: &mut Frame, state: &AppState, area: Rect, scroll: u16) {
    let mut lines: Vec<Line> = Vec::new();
    push_header(&mut lines, state, "Global");
    lines.push(line("F1", "Help"));
    lines.push(line("F2", "Theme"));
    lines.push(line("F3", "Doctor (refresh via job runner)"));
    lines.push(line("F4", "Cycle layout A → B → C"));
    lines.push(line("Ctrl+Q", "Quit"));
    lines.push(line("Tab / S-Tab", "Cycle panes"));
    lines.push(line("Esc", "Close modal / clear filter"));
    lines.push(line(":", "Palette (PR 13)"));
    lines.push(line("Ctrl+L", "Login (machine token)"));
    lines.push(line("Ctrl+C", "Cancel running job"));

    push_header(&mut lines, state, "Tree");
    lines.push(line("j/k ↑↓", "Move"));
    lines.push(line("h/l", "Expand / collapse"));
    lines.push(line("g / G", "Top / bottom"));
    lines.push(line("Enter", "Expand site / keep env selected"));
    lines.push(line("r", "Refresh inventory + metrics + backups (live)"));
    lines.push(line("b", "Stage backup:create (env row; Enter runs)"));
    lines.push(line(
        "e",
        "Deploy: git-mode on dev, backup-first test, LiveGate live",
    ));
    lines.push(line(
        "c",
        "Stage env:clear-cache (tree/inspector; not preview)",
    ));
    lines.push(line("s / S", "lando start / stop (local bound)"));
    lines.push(line("/", "Filter"));
    lines.push(line("T", "Tag filter picker (again clears)"));
    lines.push(line("a", "Add tag"));
    lines.push(line("n", "Create site wizard (org + name + upstream)"));
    lines.push(line(
        "(no W)",
        "Wipe is Actions only (backup-first; live is LiveGate)",
    ));
    lines.push(line(
        "Actions",
        "domains / HTTPS / lock (password redacted)",
    ));
    lines.push(line("x", "Remove selected tag chip (inspector)"));
    lines.push(line("[ / ]", "Cycle tag chips (inspector)"));

    push_header(&mut lines, state, "Inspector");
    lines.push(line("j/k", "Move Actions / scroll"));
    lines.push(line("Enter", "Stage action (demo: no spawn); wake is here"));
    lines.push(line("1-4", "C tabs: Info Metrics Local Actions"));
    lines.push(line(
        "d / w / Shift+M",
        "Metrics period (env row; m is CMS)",
    ));
    lines.push(line("m", "CMS form (PR 13) — never month"));

    push_header(&mut lines, state, "Preview / log");
    lines.push(line(
        "Enter",
        "Run plan (gates apply). --demo never spawns user plans",
    ));
    lines.push(line("y", "Copy redacted argv (preview focus)"));
    lines.push(line("c", "no-op in preview (not copy, not clear-cache)"));
    lines.push(line("j/k", "Scroll"));

    push_header(&mut lines, state, "Mouse");
    lines.push(line(
        "click",
        "Focus pane / select tree row / period labels",
    ));
    lines.push(line("wheel", "Scroll hovered pane"));

    push_header(&mut lines, state, "Layouts");
    lines.push(Line::from(
        "A Classic stack (default): sites|inspector, preview, log",
    ));
    lines.push(Line::from(
        "B Three-column: sites | inspector | preview/log",
    ));
    lines.push(Line::from(
        "C Tabbed inspector; log collapses to “Job log — idle”",
    ));

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
