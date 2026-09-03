use crate::models::OrgRef;
use crate::plan::CommandPlan;
use crate::state::{AppState, CreateField, SiteCreateForm};
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
