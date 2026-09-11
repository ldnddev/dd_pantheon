use crate::models::OrgRef;
use crate::plan::CommandPlan;
use crate::state::{
    AppState, BackupPickKind, CmsFocus, CmsForm, CreateField, PaletteForm, SiteCreateForm,
};
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub fn draw_login(
    f: &mut Frame,
    theme: &Theme,
    area: Rect,
    token: &str,
    use_env_token: bool,
    env_available: bool,
) {
    let masked: String = if use_env_token && env_available {
        "(env TERMINUS_MACHINE_TOKEN)".into()
    } else {
        "•".repeat(token.chars().count())
    };
    let mut lines = vec![
        Line::from(Span::styled(
            "Login",
            theme.modal_header.add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Machine token (never written to config)."),
        Line::from(Span::styled(
            "token visible in ps until login exits",
            theme.warning_style,
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("token  ", theme.modal_label),
            Span::styled(masked, theme.input_text_focus),
        ]),
    ];
    if env_available {
        let mark = if use_env_token { "[x]" } else { "[ ]" };
        lines.push(Line::from(format!(
            "{mark} use env token  (Space to toggle)"
        )));
        lines.push(Line::from(Span::styled("env token detected", theme.info)));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Enter login   Esc cancel"));
    let p = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title("Login")
            .borders(Borders::ALL)
            .border_style(theme.input_border_focus)
            .style(theme.modal),
    );
    f.render_widget(p, area);
}

pub fn draw_destructive(f: &mut Frame, state: &AppState, area: Rect, plan: &CommandPlan) {
    let theme = &state.theme;
    let lines = vec![
        Line::from(Span::styled(
            format!(
                "DESTRUCTIVE — {}",
                plan.argv.first().cloned().unwrap_or_default()
            ),
            theme.modal_header.add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(plan.redacted_shell_line(), theme.modal_text)),
        Line::from(""),
        Line::from(vec![
            Span::styled("target  ", theme.modal_label),
            Span::raw(plan.target.label()),
        ]),
        Line::from(vec![
            Span::styled("why     ", theme.modal_label),
            Span::raw(plan.why.clone()),
        ]),
        Line::from(""),
        Line::from("Enter/y confirm   Esc cancel"),
    ];
    let p = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title("Confirm")
            .borders(Borders::ALL)
            .border_style(theme.error)
            .style(theme.modal),
    );
    f.render_widget(p, area);
}

pub fn draw_livegate(
    f: &mut Frame,
    theme: &Theme,
    area: Rect,
    plan: &CommandPlan,
    expected: &str,
    typed: &str,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(6), Constraint::Length(3)])
        .split(area);
    let lines = vec![
        Line::from(Span::styled(
            format!("LIVEGATE — type: {expected}"),
            theme.modal_header.add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(plan.redacted_shell_line()),
        Line::from(""),
        Line::from(vec![
            Span::styled("target  ", theme.modal_label),
            Span::raw(plan.target.label()),
        ]),
        Line::from(vec![
            Span::styled("why     ", theme.modal_label),
            Span::raw(plan.why.clone()),
        ]),
        Line::from(""),
        Line::from("Esc cancel. y is a character, not confirm."),
    ];
    let p = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title("LiveGate")
            .borders(Borders::ALL)
            .border_style(theme.error)
            .style(theme.modal),
    );
    f.render_widget(p, chunks[0]);
    let input = Paragraph::new(typed).block(
        Block::default()
            .title("type gate word")
            .borders(Borders::ALL)
            .border_style(theme.input_border_focus)
            .style(
                Style::default()
                    .fg(theme.colors.input_text_focus)
                    .bg(theme.colors.modal_background),
            ),
    );
    f.render_widget(input, chunks[1]);
}

pub fn draw_diffstat_dirty(
    f: &mut Frame,
    theme: &Theme,
    area: Rect,
    site: &str,
    env: &str,
    files: &[String],
) {
    let mut lines = vec![
        Line::from(Span::styled(
            format!("dirty env:diffstat — {site}.{env}"),
            theme.modal_header.add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "connection:set git is blocked until this is clean.",
            theme.warning_style,
        )),
        Line::from(format!("{} uncommitted file(s):", files.len())),
    ];
    for name in files.iter().take(8) {
        lines.push(Line::from(Span::styled(
            format!("  {name}"),
            theme.modal_text,
        )));
    }
    if files.len() > 8 {
        lines.push(Line::from(Span::styled(
            format!("  … {} more", files.len() - 8),
            theme.secondary,
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("c/Enter  commit via env:commit"));
    lines.push(Line::from("Esc      abort (no discard)"));
    let p = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title("Diffstat")
            .borders(Borders::ALL)
            .border_style(theme.warning_style)
            .style(theme.modal),
    );
    f.render_widget(p, area);
}

pub fn draw_tag_add(f: &mut Frame, theme: &Theme, area: Rect, value: &str) {
    let p = Paragraph::new(format!("tag: {value}"))
        .style(theme.input_text_focus)
        .block(
            Block::default()
                .title("Add tag")
                .borders(Borders::ALL)
                .border_style(theme.input_border_focus)
                .style(theme.modal),
        );
    f.render_widget(p, area);
}

pub fn draw_org_picker(f: &mut Frame, theme: &Theme, area: Rect, orgs: &[OrgRef], selected: usize) {
    let mut lines = vec![
        Line::from(Span::styled(
            "Pick organization for tags",
            theme.modal_header.add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    for (i, org) in orgs.iter().enumerate() {
        let marker = if i == selected { "> " } else { "  " };
        let style = if i == selected {
            theme.active_label
        } else {
            theme.modal_text
        };
        lines.push(Line::from(Span::styled(
            format!("{marker}{}  ({})", org.org_name, org.org_id),
            style,
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Enter pick   Esc cancel"));
    let p = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title("Organization")
            .borders(Borders::ALL)
            .border_style(theme.active_border)
            .style(theme.modal),
    );
    f.render_widget(p, area);
}

pub fn draw_tag_picker(f: &mut Frame, theme: &Theme, area: Rect, tags: &[String], selected: usize) {
    let mut lines = vec![
        Line::from(Span::styled(
            "Filter tree by tag",
            theme.modal_header.add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    for (i, tag) in tags.iter().enumerate() {
        let marker = if i == selected { "> " } else { "  " };
        let style = if i == selected {
            theme.active_label
        } else {
            theme.modal_text
        };
        lines.push(Line::from(Span::styled(format!("{marker}{tag}"), style)));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Enter pin   Esc cancel   T again clears"));
    let p = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title("Tags")
            .borders(Borders::ALL)
            .border_style(theme.active_border)
            .style(theme.modal),
    );
    f.render_widget(p, area);
}

pub fn draw_site_create(f: &mut Frame, theme: &Theme, area: Rect, form: &SiteCreateForm) {
    let org = form
        .orgs
        .get(form.org_idx)
        .map(|o| o.org_name.as_str())
        .unwrap_or("(none)");
    let up = form
        .upstreams
        .get(form.upstream_idx)
        .map(|u| u.label.as_str())
        .unwrap_or("(none)");
    let bind = if form.bind_local { "[x]" } else { "[ ]" };
    let mut lines = vec![
        Line::from(Span::styled(
            "Create site",
            theme.modal_header.add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        field(theme, form.focus == CreateField::Name, "name", &form.name),
        field(
            theme,
            form.focus == CreateField::Label,
            "label",
            &form.label,
        ),
        field(theme, form.focus == CreateField::Org, "org", org),
        field(theme, form.focus == CreateField::Upstream, "upstream", up),
        field(
            theme,
            form.focus == CreateField::Bind,
            "bind",
            &format!("{bind} local path"),
        ),
        field(
            theme,
            form.focus == CreateField::Path,
            "path",
            &form.local_path,
        ),
        Line::from(""),
        Line::from(Span::styled(
            "Tab fields  j/k org/upstream  Space bind  Enter create  Esc",
            theme.secondary,
        )),
    ];
    if form.orgs.is_empty() {
        lines.push(Line::from(Span::styled(
            "loading orgs… or none available",
            theme.warning_style,
        )));
    }
    if form.upstreams.is_empty() {
        lines.push(Line::from(Span::styled(
            "loading upstreams… or none available",
            theme.warning_style,
        )));
    }
    let p = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title("site:create")
            .borders(Borders::ALL)
            .border_style(theme.input_border_focus)
            .style(theme.modal),
    );
    f.render_widget(p, area);
}

pub fn draw_domain_add(f: &mut Frame, theme: &Theme, area: Rect, value: &str) {
    let p = Paragraph::new(vec![
        Line::from(Span::styled(
            "Add domain",
            theme.modal_header.add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("domain  ", theme.modal_label),
            Span::styled(value.to_string(), theme.input_text_focus),
        ]),
        Line::from(""),
        Line::from("Enter add   Esc cancel"),
    ])
    .wrap(Wrap { trim: false })
    .block(
        Block::default()
            .title("domain:add")
            .borders(Borders::ALL)
            .border_style(theme.input_border_focus)
            .style(theme.modal),
    );
    f.render_widget(p, area);
}

pub fn draw_domain_remove(
    f: &mut Frame,
    theme: &Theme,
    area: Rect,
    domains: &[String],
    selected: usize,
) {
    let mut lines = vec![
        Line::from(Span::styled(
            "Remove domain (Destructive)",
            theme.modal_header.add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    for (i, d) in domains.iter().enumerate() {
        let marker = if i == selected { "> " } else { "  " };
        let style = if i == selected {
            theme.active_label
        } else {
            theme.modal_text
        };
        lines.push(Line::from(Span::styled(format!("{marker}{d}"), style)));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Enter remove   Esc cancel"));
    let p = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title("domain:remove")
            .borders(Borders::ALL)
            .border_style(theme.error)
            .style(theme.modal),
    );
    f.render_widget(p, area);
}

pub fn draw_https_set(
    f: &mut Frame,
    theme: &Theme,
    area: Rect,
    cert: &str,
    key: &str,
    intermediate: &str,
    focus: u8,
) {
    let path_line = |focused: bool, label: &str, path: &str| {
        let marker = if focused { ">" } else { " " };
        Line::from(vec![
            Span::styled(format!("{marker} {label:<8}"), theme.modal_label),
            Span::styled(path.to_string(), theme.file),
        ])
    };
    let lines = vec![
        Line::from(Span::styled(
            "Set HTTPS (certificate file paths, not contents)",
            theme.modal_header.add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        path_line(focus == 0, "cert", cert),
        path_line(focus == 1, "key", key),
        path_line(focus == 2, "chain", intermediate),
        Line::from(""),
        Line::from(Span::styled(
            "Tab fields  Enter set  Esc cancel",
            theme.secondary,
        )),
    ];
    let p = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title("https:set")
            .borders(Borders::ALL)
            .border_style(theme.input_border_focus)
            .style(theme.modal),
    );
    f.render_widget(p, area);
}

pub fn draw_lock_enable(
    f: &mut Frame,
    theme: &Theme,
    area: Rect,
    username: &str,
    password: &str,
    focus: u8,
) {
    let masked: String = "•".repeat(password.chars().count());
    let user_style = if focus == 0 {
        theme.input_text_focus
    } else {
        theme.modal_text
    };
    let pass_style = if focus == 1 {
        theme.input_text_focus
    } else {
        theme.modal_text
    };
    let lines = vec![
        Line::from(Span::styled(
            "Enable HTTP basic auth",
            theme.modal_header.add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "password is redacted in preview and the job log",
            theme.warning_style,
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                if focus == 0 { "> user  " } else { "  user  " },
                theme.modal_label,
            ),
            Span::styled(username.to_string(), user_style),
        ]),
        Line::from(vec![
            Span::styled(
                if focus == 1 { "> pass  " } else { "  pass  " },
                theme.modal_label,
            ),
            Span::styled(masked, pass_style),
        ]),
        Line::from(""),
        Line::from("Tab fields  Enter enable  Esc cancel"),
    ];
    let p = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title("lock:enable")
            .borders(Borders::ALL)
            .border_style(theme.input_border_focus)
            .style(theme.modal),
    );
    f.render_widget(p, area);
}

pub fn draw_multidev_create(
    f: &mut Frame,
    theme: &Theme,
    area: Rect,
    site: &str,
    name: &str,
    sources: &[String],
    source_idx: usize,
) {
    let source = sources
        .get(source_idx)
        .map(|s| s.as_str())
        .unwrap_or("live");
    let lines = vec![
        Line::from(Span::styled(
            format!("Create multidev on {site}"),
            theme.modal_header.add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("name    ", theme.modal_label),
            Span::styled(name.to_string(), theme.input_text_focus),
        ]),
        Line::from(vec![
            Span::styled("source  ", theme.modal_label),
            Span::raw(source.to_string()),
        ]),
        Line::from(""),
        Line::from("≤11 lowercase alnum/dashes. j/k source. Enter create. Esc cancel."),
    ];
    let p = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title("multidev:create")
            .borders(Borders::ALL)
            .border_style(theme.input_border_focus)
            .style(theme.modal),
    );
    f.render_widget(p, area);
}

pub fn draw_clone_content(
    f: &mut Frame,
    theme: &Theme,
    area: Rect,
    site: &str,
    target: &str,
    origins: &[String],
    origin_idx: usize,
    cc: bool,
    db_only: bool,
    files_only: bool,
    updatedb: bool,
) {
    let origin = origins
        .get(origin_idx)
        .map(|s| s.as_str())
        .unwrap_or("live");
    let mark = |on: bool| if on { "[x]" } else { "[ ]" };
    let lines = vec![
        Line::from(Span::styled(
            format!("Clone content onto {site}.{target}"),
            theme.modal_header.add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "backup-first; live target is LiveGate",
            theme.warning_style,
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("origin   ", theme.modal_label),
            Span::raw(origin.to_string()),
        ]),
        Line::from(format!("  {} --cc        (c)", mark(cc))),
        Line::from(format!("  {} --db-only   (d)", mark(db_only))),
        Line::from(format!("  {} --files-only (f)", mark(files_only))),
        Line::from(format!("  {} --updatedb  (u)", mark(updatedb))),
        Line::from(""),
        Line::from("j/k origin  Enter stage  Esc cancel"),
    ];
    let p = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title("env:clone-content")
            .borders(Borders::ALL)
            .border_style(theme.warning_style)
            .style(theme.modal),
    );
    f.render_widget(p, area);
}

pub fn draw_deploy_note(
    f: &mut Frame,
    theme: &Theme,
    area: Rect,
    env: &str,
    note: &str,
    sync_content: bool,
    updatedb: bool,
    focus: u8,
) {
    let mark = |on: bool| if on { "[x]" } else { "[ ]" };
    let mut lines = vec![
        Line::from(Span::styled(
            format!("Deploy to {env}"),
            theme.modal_header.add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        field(theme, focus == 0, "note", note),
    ];
    if env == "test" {
        lines.push(Line::from(Span::styled(
            format!(
                "{} {} --sync-content  (Space; backup-first)",
                if focus == 1 { ">" } else { " " },
                mark(sync_content)
            ),
            if focus == 1 {
                theme.input_text_focus
            } else {
                theme.modal_text
            },
        )));
    }
    let ud_focus = if env == "test" { 2 } else { 1 };
    lines.push(Line::from(Span::styled(
        format!(
            "{} {} --updatedb     (Space; Drupal)",
            if focus == ud_focus { ">" } else { " " },
            mark(updatedb)
        ),
        if focus == ud_focus {
            theme.input_text_focus
        } else {
            theme.modal_text
        },
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Tab fields  Enter deploy  Esc cancel",
        theme.secondary,
    )));
    let p = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title("env:deploy")
            .borders(Borders::ALL)
            .border_style(theme.input_border_focus)
            .style(theme.modal),
    );
    f.render_widget(p, area);
}

pub fn draw_backup_pick(
    f: &mut Frame,
    theme: &Theme,
    area: Rect,
    files: &[String],
    selected: usize,
    kind: BackupPickKind,
) {
    let destructive = matches!(kind, BackupPickKind::Restore);
    let mut lines = vec![
        Line::from(Span::styled(
            kind.label().to_string(),
            theme.modal_header.add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    for (i, file) in files.iter().enumerate() {
        let marker = if i == selected { "> " } else { "  " };
        let style = if i == selected {
            theme.active_label
        } else {
            theme.modal_text
        };
        lines.push(Line::from(Span::styled(format!("{marker}{file}"), style)));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("j/k select  Enter  Esc cancel"));
    let border = if destructive {
        theme.error
    } else {
        theme.input_border_focus
    };
    let title = match kind {
        BackupPickKind::Restore => "backup:restore",
        BackupPickKind::Get => "backup:get",
    };
    let p = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border)
            .style(theme.modal),
    );
    f.render_widget(p, area);
}

pub fn draw_filter(f: &mut Frame, state: &AppState, area: Rect, query: &str, selected: usize) {
    let theme = &state.theme;
    let hits = state.filter_hits(query);
    let block = Block::default()
        .title("Filter sites")
        .borders(Borders::ALL)
        .border_style(theme.input_border_focus)
        .style(theme.modal);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(inner);

    let input = Paragraph::new(Line::from(vec![
        Span::styled("/", theme.modal_label),
        Span::styled(query.to_string(), theme.input_text_focus),
    ]));
    f.render_widget(input, chunks[0]);

    let mut lines: Vec<Line> = Vec::new();
    if hits.is_empty() {
        lines.push(Line::from(Span::styled(
            "no sites match name or tag",
            theme.warning_style,
        )));
    } else {
        let start = selected.saturating_sub(8);
        for (i, hit) in hits.iter().enumerate().skip(start).take(16) {
            let marker = if i == selected { "> " } else { "  " };
            let style = if i == selected {
                theme.active_label
            } else {
                theme.modal_text
            };
            let name = match &hit.env {
                Some(env) => format!("{}.{}", hit.site, env),
                None => hit.site.clone(),
            };
            let mut spans = vec![
                Span::styled(marker.to_string(), style),
                Span::styled(name, style),
            ];
            for tag in hit.tags.iter().take(4) {
                spans.push(Span::styled(format!(" [{tag}]"), theme.modal_label));
            }
            lines.push(Line::from(spans));
        }
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[1]);
    let hint = format!(
        "{} matches   type name or tag   ↑↓ select   Enter jump   Esc close",
        hits.len()
    );
    f.render_widget(
        Paragraph::new(Span::styled(hint, theme.secondary)),
        chunks[2],
    );
}

pub fn draw_palette(
    f: &mut Frame,
    state: &AppState,
    area: Rect,
    query: &str,
    selected: usize,
    form: Option<&PaletteForm>,
) {
    let theme = &state.theme;
    if let Some(form) = form {
        draw_palette_form(f, theme, area, form);
        return;
    }
    let hits = crate::workflows::palette::matches_for(state, query);
    let mut lines = vec![
        Line::from(Span::styled(
            "Terminus / Lando commands",
            theme.modal_header.add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "type a command, tool, or related term (cache, deploy, wp, pull…)",
            theme.secondary,
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(": ", theme.modal_label),
            Span::styled(query.to_string(), theme.input_text_focus),
        ]),
        Line::from(""),
    ];
    if hits.is_empty() {
        lines.push(Line::from(Span::styled("no matches", theme.warning_style)));
    } else {
        let start = selected.saturating_sub(6);
        for (i, entry) in hits.iter().enumerate().skip(start).take(12) {
            let marker = if i == selected { "> " } else { "  " };
            let style = if i == selected {
                theme.active_label
            } else {
                theme.modal_text
            };
            let tool = entry.tool.binary_name();
            lines.push(Line::from(Span::styled(
                format!("{marker}{tool:<8} {:<22} {}", entry.name, entry.description),
                style,
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(format!("{} matches", hits.len())));
    }
    lines.push(Line::from(Span::styled(
        "type to filter  ↑↓  Enter form  Esc",
        theme.secondary,
    )));
    let p = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title("Palette")
            .borders(Borders::ALL)
            .border_style(theme.input_border_focus)
            .style(theme.modal),
    );
    f.render_widget(p, area);
}

fn draw_palette_form(f: &mut Frame, theme: &Theme, area: Rect, form: &PaletteForm) {
    let mut lines = vec![
        Line::from(Span::styled(
            form.entry.name.clone(),
            theme.modal_header.add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            form.entry.description.clone(),
            theme.secondary,
        )),
        Line::from(""),
    ];
    for (i, (name, value)) in form.args.iter().enumerate() {
        lines.push(field(theme, form.focused_arg() == Some(i), name, value));
    }
    for (i, (flag, on)) in form.toggles.iter().enumerate() {
        let mark = if *on { "[x]" } else { "[ ]" };
        let focused = form.focused_toggle() == Some(i);
        let style = if focused {
            theme.input_text_focus
        } else {
            theme.modal_text
        };
        let marker = if focused { ">" } else { " " };
        lines.push(Line::from(Span::styled(
            format!("{marker} {mark} {flag}  (Space)"),
            style,
        )));
    }
    if let Some(el) = &form.element {
        lines.push(field(theme, form.focus_element(), "--element", el));
    }
    lines.push(field(theme, form.focus_extra(), "extra", &form.extra));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Tab fields  Enter stage (does not run)  Esc back",
        theme.secondary,
    )));
    let p = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title("Palette form")
            .borders(Borders::ALL)
            .border_style(theme.input_border_focus)
            .style(theme.modal),
    );
    f.render_widget(p, area);
}

pub fn draw_cms(f: &mut Frame, theme: &Theme, area: Rect, form: &CmsForm) {
    let mark = |focused: bool| if focused { ">" } else { " " };
    let style = |focused: bool| {
        if focused {
            theme.input_text_focus
        } else {
            theme.modal_text
        }
    };
    let mut lines = vec![
        Line::from(Span::styled(
            "CMS command",
            theme.modal_header.add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "one form — remote Terminus or local Lando. Never auto-inserts -y.",
            theme.secondary,
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!(
                "{} target  {}  (Space)",
                mark(form.focus == CmsFocus::Target),
                form.target.label()
            ),
            style(form.focus == CmsFocus::Target),
        )),
        Line::from(Span::styled(
            format!(
                "{} cms     {}  (Space)",
                mark(form.focus == CmsFocus::Cms),
                form.cms.label()
            ),
            style(form.focus == CmsFocus::Cms),
        )),
        field(
            theme,
            form.focus == CmsFocus::Command,
            "command",
            &form.command,
        ),
    ];
    lines.push(Line::from(""));
    let hist_style = if form.focus == CmsFocus::History {
        theme.active_label
    } else {
        theme.secondary
    };
    lines.push(Line::from(Span::styled("history (↑↓)", hist_style)));
    for (i, line) in form.history.iter().take(8).enumerate() {
        let marker = if form.history_idx == Some(i) {
            "> "
        } else {
            "  "
        };
        lines.push(Line::from(Span::styled(
            format!("{marker}{line}"),
            theme.modal_text,
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Tab fields  Enter run  Esc cancel",
        theme.secondary,
    )));
    let p = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title("CMS")
            .borders(Borders::ALL)
            .border_style(theme.input_border_focus)
            .style(theme.modal),
    );
    f.render_widget(p, area);
}

fn field<'a>(theme: &'a Theme, focused: bool, key: &str, value: &str) -> Line<'a> {
    let marker = if focused { ">" } else { " " };
    let style = if focused {
        theme.input_text_focus
    } else {
        theme.modal_text
    };
    Line::from(vec![
        Span::styled(format!("{marker} {key:<9}"), theme.modal_label),
        Span::styled(value.to_string(), style),
    ])
}
