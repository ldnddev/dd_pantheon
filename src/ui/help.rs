use crate::state::AppState;
use ratatui::text::{Line, Span};

pub fn lines(state: &AppState, width: usize) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    push_header(&mut lines, state, "Global");
    push_entry(&mut lines, "F1 / F2", "Settings", width);
    push_entry(&mut lines, "Ctrl+Q", "Quit", width);
    push_entry(&mut lines, "F3", "Doctor (refresh via job runner)", width);
    push_entry(&mut lines, "?", "Settings, Help page", width);
    push_entry(&mut lines, "Tab / S-Tab", "Cycle panes (sheet: change page)", width);
    push_entry(&mut lines, "Esc", "Close settings / clear filter", width);
    push_entry(
        &mut lines,
        "/",
        "Filter sites by name or tag (modal list; Enter jumps)",
        width,
    );
    push_entry(
        &mut lines,
        ":",
        "Terminus/Lando commands as selected (type `lando` to filter; non-init Lando is grayed without a Pantheon .lando.yml)",
        width,
    );
    push_entry(&mut lines, "Ctrl+K", "Palette", width);
    push_entry(&mut lines, "Ctrl+L", "Login (machine token)", width);
    push_entry(&mut lines, "Actions", "Logout when logged in (Mutating)", width);
    push_entry(&mut lines, "Ctrl+C", "Cancel running job", width);

    push_header(&mut lines, state, "Tree");
    push_entry(&mut lines, "j/k ↑↓", "Move", width);
    push_entry(&mut lines, "PgUp/PgDn", "Page", width);
    push_entry(&mut lines, "h/l", "Expand / collapse", width);
    push_entry(&mut lines, "g / G  Home/End", "Top / bottom", width);
    push_entry(&mut lines, "Enter", "Expand site / keep env selected", width);
    push_entry(
        &mut lines,
        "r",
        "Refresh inventory + metrics + backups (live)",
        width,
    );
    push_entry(&mut lines, "b", "Stage backup:create (env row; Enter runs)", width);
    push_entry(
        &mut lines,
        ":",
        "Restore backup / backup URL via palette (get prints URL, no download)",
        width,
    );
    push_entry(
        &mut lines,
        "e",
        "Deploy: git-mode on dev; test/live open a --note form (sync-content on test)",
        width,
    );
    push_entry(
        &mut lines,
        "c",
        "Stage env:clear-cache (tree/inspector; not preview)",
        width,
    );
    push_entry(&mut lines, "s / S", "lando start / stop (local bound)", width);
    push_entry(&mut lines, "/", "Filter sites (name / tag)", width);
    push_entry(&mut lines, "T", "Tag filter picker (again clears)", width);
    push_entry(&mut lines, "a", "Add tag", width);
    push_entry(&mut lines, "n", "Create site wizard (org + name + upstream)", width);
    push_entry(
        &mut lines,
        "(no W)",
        "Wipe is palette-only (backup-first; live is LiveGate)",
        width,
    );
    push_entry(
        &mut lines,
        ":",
        "domains / HTTPS / lock via palette (password redacted)",
        width,
    );
    push_entry(&mut lines, "x", "Remove selected tag chip (inspector)", width);
    push_entry(&mut lines, "[ / ]", "Cycle tag chips (inspector)", width);

    push_header(&mut lines, state, "Inspector");
    push_entry(&mut lines, "j/k  PgUp/PgDn", "Scroll inspector", width);
    push_entry(&mut lines, "g / G  Home/End", "Inspector top / bottom", width);
    push_entry(&mut lines, "Enter", "Run the staged plan (same as preview)", width);
    push_entry(
        &mut lines,
        "d / w / Shift+M",
        "Metrics period (env row; m is CMS)",
        width,
    );
    push_entry(&mut lines, "m", "CMS form (remote/local; never month)", width);

    push_header(&mut lines, state, "Preview / log");
    push_entry(
        &mut lines,
        "Enter",
        "PROCESS COMMAND — run plan (gates apply; --demo never spawns)",
        width,
    );
    push_entry(
        &mut lines,
        "y",
        "COPY TO CLIPBOARD (redacted argv; preview focus)",
        width,
    );
    push_entry(
        &mut lines,
        "Esc",
        "CANCEL staged plan when filter is already clear",
        width,
    );
    push_entry(&mut lines, "c", "no-op in preview (not copy, not clear-cache)", width);
    push_entry(&mut lines, "j/k", "Scroll", width);
    push_entry(
        &mut lines,
        "Enter / e",
        "Expand job log to a full-size modal (Esc closes)",
        width,
    );
    push_entry(&mut lines, "g / G", "Log top / bottom", width);
    push_entry(&mut lines, "PgUp / PgDn", "Log page", width);
    push_entry(
        &mut lines,
        "click",
        "Process / Copy / Cancel buttons in command preview",
        width,
    );
    push_entry(&mut lines, "click log", "Focus; click again to expand", width);

    push_header(&mut lines, state, "Mouse");
    push_entry(
        &mut lines,
        "click",
        "Focus pane / select tree row / period labels",
        width,
    );
    push_entry(&mut lines, "wheel", "Scroll hovered pane", width);
    push_entry(
        &mut lines,
        "click outside",
        "Close Doctor, Filter, Palette, log overlay",
        width,
    );
    lines
}

fn push_header(lines: &mut Vec<Line<'static>>, state: &AppState, title: &str) {
    if !lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        title.to_string(),
        state.theme.modal_header,
    )));
}

fn push_entry(lines: &mut Vec<Line<'static>>, key: &str, desc: &str, width: usize) {
    let prefix = format!("  {key:<16}");
    let prefix_w = prefix.chars().count();
    if width <= prefix_w.saturating_add(4) {
        lines.push(Line::from(format!("{prefix}{desc}")));
        return;
    }
    let budget = width - prefix_w;
    let mut rest = desc;
    let mut first = true;
    loop {
        if rest.is_empty() {
            if first {
                lines.push(Line::from(prefix));
            }
            break;
        }
        let take = if first {
            budget
        } else {
            width.saturating_sub(2).max(1)
        };
        let (chunk, next) = split_chars(rest, take);
        if chunk.is_empty() {
            break;
        }
        if first {
            lines.push(Line::from(format!("{prefix}{chunk}")));
            first = false;
        } else {
            lines.push(Line::from(format!("  {chunk}")));
        }
        rest = next;
    }
}

fn split_chars(text: &str, n: usize) -> (&str, &str) {
    if n == 0 {
        return ("", text);
    }
    let mut count = 0;
    for (i, _) in text.char_indices() {
        if count == n {
            return (&text[..i], &text[i..]);
        }
        count += 1;
    }
    (text, "")
}
