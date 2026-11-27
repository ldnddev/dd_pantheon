//! Rows both the window and the headless tests paint. Derived from `AppState`.

use dd_pantheon::catalog::CatalogKind;
use dd_pantheon::plan::StagedPlan;
use dd_pantheon::state::{AppState, FocusPane, Modal, TreeSel};
use dd_pantheon::workflows::palette::{self, lando_command_enabled};

pub fn cockpit(state: &AppState, footer: &str) -> Vec<String> {
    let mut lines = vec![
        "dd_pantheon".to_string(),
        header_line(state),
        format!("focus:{}", focus_name(state.focus)),
        format!("footer:{footer}"),
    ];
    if let Some(toast) = &state.toast {
        lines.push(format!("toast:{}", toast.message));
    }
    lines.push("tree:".into());
    lines.extend(tree_lines(state));
    lines.push("inspector:".into());
    lines.extend(inspector_lines(state));
    lines.push("preview:".into());
    lines.extend(preview_lines(state));
    lines.push("log:".into());
    lines.extend(log_lines(state));
    if let Some(modal) = modal_lines(state) {
        lines.push("modal:".into());
        lines.extend(modal);
    }
    lines
}

pub fn header_line(state: &AppState) -> String {
    let mut parts = Vec::new();
    if state.demo {
        parts.push("DEMO".to_string());
    }
    let status = header_status(state);
    if !status.is_empty() {
        parts.push(status);
    }
    if parts.is_empty() {
        "header".to_string()
    } else {
        format!("header:{}", parts.join(" "))
    }
}

pub fn header_status(state: &AppState) -> String {
    match &state.selected {
        TreeSel::Env { site, env } => format!("{site}.{env}"),
        TreeSel::Site(site) => site.clone(),
        TreeSel::None => String::new(),
    }
}

pub fn focus_name(focus: FocusPane) -> &'static str {
    match focus {
        FocusPane::Tree => "tree",
        FocusPane::Inspector => "inspector",
        FocusPane::Preview => "preview",
        FocusPane::Log => "log",
    }
}

pub fn tree_lines(state: &AppState) -> Vec<String> {
    let selected = state.tree_state.selected();
    if state.tree_rows.is_empty() {
        return vec![empty_tree(state).to_string()];
    }
    state
        .tree_rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let mark = if selected == Some(index) { ">" } else { " " };
            let frozen = state.site(&row.site).is_some_and(|site| site.frozen);
            if let Some(env) = &row.env {
                let found = state
                    .envs
                    .get(&row.site)
                    .and_then(|envs| envs.iter().find(|item| item.id == *env));
                let mut status = String::new();
                if let Some(mode) = found.map(|item| item.connection_mode.label()) {
                    status.push_str("  ");
                    status.push_str(mode);
                }
                if found.is_some_and(|item| item.locked) {
                    status.push_str("  lock");
                }
                format!("{mark}     ▪ {env}{status}")
            } else {
                let glyph = if row.expanded { "▾" } else { "▸" };
                let status = if frozen { "  frozen" } else { "" };
                format!("{mark} {glyph} {}{status}", row.site)
            }
        })
        .collect()
}

fn empty_tree(state: &AppState) -> &'static str {
    if state.demo {
        return "no sites";
    }
    if !state.tools_enabled {
        return "terminus not on PATH";
    }
    match state.auth {
        dd_pantheon::state::AuthState::LoggedOut | dd_pantheon::state::AuthState::Unknown => {
            "not logged in — Ctrl+L"
        }
        dd_pantheon::state::AuthState::LoggedIn { .. } => {
            if state.inflight_readonly.contains_key("site:list") {
                "loading sites…"
            } else {
                "no sites — r to refresh, n to create"
            }
        }
    }
}

pub fn inspector_lines(state: &AppState) -> Vec<String> {
    let mut lines = Vec::new();
    let mut title = match &state.selected {
        TreeSel::Env { site, env } => format!("Inspector: {site} [{env}]"),
        TreeSel::Site(site) => format!("Inspector: {site}"),
        TreeSel::None => "Inspector".to_string(),
    };
    if state.demo {
        title.push_str("  DEMO");
    }
    lines.push(title);
    if !state.demo {
        match state.auth {
            dd_pantheon::state::AuthState::LoggedOut => {
                lines.push("not logged in — Ctrl+L to login".into());
            }
            dd_pantheon::state::AuthState::Unknown if !state.tools_enabled => {
                lines.push("terminus not on PATH".into());
            }
            _ => {}
        }
    }
    if let Some(site) = state.selected_site() {
        let framework = site
            .upstream_label
            .as_deref()
            .filter(|label| !label.is_empty())
            .unwrap_or_else(|| site.framework.display_name());
        lines.push(format!("framework {framework}"));
        if site.frozen {
            lines.push("status frozen".into());
        }
        if site.orgs.is_empty() {
            lines.push("tags require an organization".into());
        } else {
            let orgs = site
                .orgs
                .iter()
                .map(|org| org.org_name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("org {orgs}"));
        }
        if site.tags.is_empty() {
            lines.push("tags —".into());
        } else {
            lines.push(tag_line(state, site));
        }
        if let Some(plan) = &site.plan_name {
            lines.push(format!("plan {plan}"));
        }
        if let Some(up) = site.upstream.as_ref() {
            let same_as_framework = up.eq_ignore_ascii_case(site.framework.label())
                || up.eq_ignore_ascii_case(site.framework.display_name());
            let same_as_label = site
                .upstream_label
                .as_ref()
                .is_some_and(|label| label.eq_ignore_ascii_case(up));
            if !same_as_framework && !same_as_label {
                lines.push(format!("upstream {up}"));
            }
        }
        if let Some(region) = &site.region {
            lines.push(format!("region {region}"));
        }
        lines.push(format!("id {}", site.id));
    }
    if let Some(env) = state.selected_env() {
        lines.push(format!(
            "url {}",
            env.domain
                .as_deref()
                .map(display_url)
                .unwrap_or_else(|| "—".into())
        ));
        lines.push(format!("mode {}", env.connection_mode.label()));
        if env.locked {
            lines.push("lock locked".into());
        }
        if let Some(php) = &env.php_version {
            lines.push(format!("php {php}"));
        }
        if let Some(runtime) = &env.php_runtime_generation {
            lines.push(format!("runtime {runtime}"));
        }
        if let Some(drush) = &env.drush_version {
            lines.push(format!("drush {drush}"));
        }
        if let Some(created) = &env.created {
            lines.push(format!("created {created}"));
        }
        let backups = state.selected_backups();
        if backups.is_empty() {
            lines.push("backups none yet".into());
        } else {
            lines.push(format!("backups {}", backups.len().min(5)));
            for backup in backups.iter().take(5) {
                lines.push(format!(
                    "  {}  {}  {}  exp {}",
                    backup.file, backup.size, backup.date, backup.expiry
                ));
            }
        }
        let domains: Vec<&str> = state
            .selected_domains()
            .iter()
            .take(4)
            .map(|domain| domain.id.as_str())
            .collect();
        if domains.is_empty() {
            lines.push("domains —".into());
        } else {
            lines.push(format!("domains {}", domains.join("  ")));
        }
        let https = state
            .selected_https()
            .first()
            .map(|row| {
                if row.status_message.is_empty() {
                    row.status.clone()
                } else {
                    format!("{} ({})", row.status, row.status_message)
                }
            })
            .unwrap_or_else(|| "—".into());
        lines.push(format!("https {https}"));
        let lock = match state.selected_lock() {
            Some(lock) if lock.locked => {
                format!("on  user {}", lock.username.as_deref().unwrap_or("—"))
            }
            Some(_) => "off".into(),
            None => {
                if env.locked {
                    "on".into()
                } else {
                    "off".into()
                }
            }
        };
        lines.push(format!("lock {lock}"));
    } else if matches!(state.selected, TreeSel::Site(_)) {
        lines.push("select an environment for metrics".into());
    }
    if let Some(local) = state.selected_site().and_then(|site| site.local.as_ref()) {
        let running = match local.running {
            Some(true) => "running",
            Some(false) => "stopped",
            None => "?",
        };
        lines.push(format!("local {}  {running}", local.path.display()));
    }
    let period = state.metrics_period;
    let (visits, pages, cache) = match state
        .selected_metrics()
        .and_then(|series| series.points.last())
    {
        Some(point) => (
            compact_count(point.visits),
            compact_count(point.pages_served),
            format!("{:.0}%", point.cache_hit_ratio * 100.0),
        ),
        None => ("—".into(), "—".into(), "—".into()),
    };
    lines.push(format!(
        "metrics ({} / {} / {}) visits {visits} pages {pages} cache {cache}  lagged",
        mark_period(period, dd_pantheon::models::MetricsPeriod::Day),
        mark_period(period, dd_pantheon::models::MetricsPeriod::Week),
        mark_period(period, dd_pantheon::models::MetricsPeriod::Month),
    ));
    if let Some(series) = state.selected_metrics() {
        let visits: Vec<u64> = series.points.iter().map(|point| point.visits).collect();
        if !visits.is_empty() {
            lines.push(format!("series {}", sparkline(&visits)));
        }
    } else if state.selected_env().is_some() {
        let msg = if dd_pantheon::workflows::metrics::metrics_inflight(state) {
            "loading metrics…"
        } else if state.metrics_error.is_none() {
            "no metrics yet"
        } else {
            ""
        };
        if !msg.is_empty() {
            lines.push(msg.into());
        }
    }
    if let Some(err) = &state.metrics_error {
        lines.push(format!("metrics-error {err}"));
    }
    for action in &state.actions {
        lines.push(format!("action:{} {}", action.id, action.label));
    }
    lines
}

fn mark_period(
    current: dd_pantheon::models::MetricsPeriod,
    period: dd_pantheon::models::MetricsPeriod,
) -> String {
    if current == period {
        format!("[{}]", period.key_label())
    } else {
        period.key_label().to_string()
    }
}

pub fn preview_lines(state: &AppState) -> Vec<String> {
    let Some(staged) = &state.current else {
        return vec![
            "READ-ONLY  —".into(),
            "no plan staged".into(),
            "$".into(),
            "PROCESS COMMAND".into(),
            "COPY TO CLIPBOARD".into(),
            "CANCEL".into(),
        ];
    };
    let current = staged.current();
    let shell = current
        .map(|plan| plan.redacted_shell_line())
        .unwrap_or_default();
    let cwd = current
        .and_then(|plan| plan.cwd.as_ref().map(|path| path.display().to_string()))
        .unwrap_or_else(|| "(none)".into());
    let mut lines = vec![
        format!(
            "{}  {}{}",
            staged.safety().badge(),
            staged.target_label(),
            step_suffix(staged)
        ),
        staged.why().to_string(),
        format!("$ {shell}"),
        format!("cwd: {cwd}"),
    ];
    if let StagedPlan::Workflow { plan, step } = staged {
        for (index, item) in plan.steps.iter().enumerate() {
            let mark = if index == *step { ">" } else { " " };
            lines.push(format!(
                "{mark} {}: {}",
                index + 1,
                item.redacted_shell_line()
            ));
        }
    }
    lines.push("PROCESS COMMAND".into());
    lines.push("COPY TO CLIPBOARD".into());
    lines.push("CANCEL".into());
    lines
}

fn step_suffix(plan: &StagedPlan) -> String {
    match plan {
        StagedPlan::Workflow { plan, step } if !plan.steps.is_empty() => {
            format!("  step {}/{}", step + 1, plan.steps.len())
        }
        _ => String::new(),
    }
}

pub fn log_lines(state: &AppState) -> Vec<String> {
    if state.log_lines.is_empty() {
        return vec!["(empty)".into()];
    }
    let start = state.log_scroll as usize;
    let rows = state.log_visible_rows() as usize;
    state
        .log_lines
        .iter()
        .skip(start)
        .take(rows.max(1))
        .cloned()
        .collect()
}

pub fn tree_title(state: &AppState) -> String {
    let filter = if state.filter.is_empty() {
        String::new()
    } else {
        format!("  /{}", state.filter)
    };
    let tag = state
        .tag_filter
        .as_ref()
        .map(|tag| format!("  tag:{tag}"))
        .unwrap_or_default();
    format!("Available sites ({}){filter}{tag}", state.sites.len())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagChip {
    pub name: String,
    pub close: bool,
}

pub fn tag_chips(state: &AppState) -> Vec<TagChip> {
    let Some(site) = state.selected_site() else {
        return Vec::new();
    };
    let focused = state.focus == FocusPane::Inspector;
    site.tags
        .iter()
        .map(|tag| TagChip {
            close: focused && state.selected_chip.as_deref() == Some(tag.name.as_str()),
            name: tag.name.clone(),
        })
        .collect()
}

pub fn clamp_scroll(scroll: u16, len: usize, rows: u16) -> u16 {
    let rows = rows.max(1) as usize;
    let max = len.saturating_sub(rows);
    scroll.min(max as u16)
}

pub fn page_lines(lines: &[String], scroll: u16, rows: u16) -> Vec<String> {
    let rows = rows.max(1) as usize;
    lines
        .iter()
        .skip(scroll as usize)
        .take(rows)
        .cloned()
        .collect()
}

pub fn modal_lines(state: &AppState) -> Option<Vec<String>> {
    let modal = state.modal.as_ref()?;
    Some(match modal {
        Modal::Doctor { .. } => doctor_lines(state),
        Modal::Login {
            token,
            use_env_token,
            env_available,
        } => login_lines(token, *use_env_token, *env_available),
        Modal::ConfirmDestructive { plan } => vec![
            "Confirm destructive".into(),
            plan.redacted_shell_line(),
            plan.why.clone(),
            "Enter or y runs. Esc cancels.".into(),
        ],
        Modal::LiveGate {
            plan,
            expected,
            typed,
        } => livegate_lines(plan, expected, typed),
        Modal::Filter { query, selected } => filter_lines(state, query, *selected),
        Modal::TagAdd { value } => vec![
            "Add tag".into(),
            format!("tag {value}"),
            "Enter add  Esc cancel".into(),
        ],
        Modal::OrgPicker {
            site,
            orgs,
            selected,
        } => {
            let mut lines = vec![format!("Pick org for {site}")];
            for (index, org) in orgs.iter().enumerate() {
                let mark = if index == *selected { ">" } else { " " };
                lines.push(format!("{mark} {}", org.org_name));
            }
            lines
        }
        Modal::TagPicker { tags, selected } => {
            let mut lines = vec!["Tag filter".into()];
            for (index, tag) in tags.iter().enumerate() {
                let mark = if index == *selected { ">" } else { " " };
                lines.push(format!("{mark} {tag}"));
            }
            if tags.is_empty() {
                lines.push("(no tags)".into());
            }
            lines
        }
        Modal::DiffstatDirty { site, env, files } => {
            let mut lines = vec![format!("Uncommitted files on {site}.{env}")];
            lines.extend(files.iter().cloned());
            lines.push("connection:set is blocked until these are committed".into());
            lines
        }
        Modal::SiteCreate { form } => site_create_lines(form),
        Modal::MultidevCreate {
            site,
            name,
            sources,
            source_idx,
        } => {
            let source = sources.get(*source_idx).map(String::as_str).unwrap_or("—");
            vec![
                format!("Create multidev on {site}"),
                format!("name {name}"),
                format!("from {source}"),
                "≤11 chars, lowercase alnum/dashes, not dev/test/live".into(),
            ]
        }
        Modal::CloneContent {
            site,
            target,
            origins,
            origin_idx,
            cc,
            db_only,
            files_only,
            updatedb,
        } => {
            let origin = origins.get(*origin_idx).map(String::as_str).unwrap_or("—");
            vec![
                format!("Clone content onto {site}.{target}"),
                format!("from {origin}"),
                format!(
                    "cc {}  db-only {}  files-only {}  updatedb {}",
                    flag(*cc),
                    flag(*db_only),
                    flag(*files_only),
                    flag(*updatedb)
                ),
            ]
        }
        Modal::DomainAdd { site, env, value } => {
            vec![
                format!("Add domain on {site}.{env}"),
                format!("domain {value}"),
            ]
        }
        Modal::DomainRemove {
            site,
            env,
            domains,
            selected,
        } => {
            let mut lines = vec![format!("Remove domain on {site}.{env}")];
            for (index, domain) in domains.iter().enumerate() {
                let mark = if index == *selected { ">" } else { " " };
                lines.push(format!("{mark} {domain}"));
            }
            lines
        }
        Modal::HttpsSet {
            site,
            env,
            cert,
            key,
            intermediate,
            focus,
        } => vec![
            format!("Set HTTPS on {site}.{env}"),
            format!("{} cert {cert}", caret(*focus == 0)),
            format!("{} key {key}", caret(*focus == 1)),
            format!("{} chain {intermediate}", caret(*focus == 2)),
            "paths, not file contents".into(),
        ],
        Modal::LockEnable {
            site,
            env,
            username,
            password,
            focus,
        } => vec![
            format!("Enable lock on {site}.{env}"),
            "password is redacted in preview and the job log".into(),
            format!("{} user {username}", caret(*focus == 0)),
            format!("{} pass {}", caret(*focus == 1), mask(password)),
        ],
        Modal::Palette {
            query,
            selected,
            form,
        } => palette_lines(state, query, *selected, form),
        Modal::Cms { form } => cms_lines(form),
        Modal::BackupPick {
            site,
            env,
            files,
            selected,
            kind,
        } => {
            let mut lines = vec![format!("{} {site}.{env}", kind.label())];
            for (index, file) in files.iter().enumerate() {
                let mark = if index == *selected { ">" } else { " " };
                lines.push(format!("{mark} {file}"));
            }
            if files.is_empty() {
                lines.push("(no backups)".into());
            }
            lines
        }
        Modal::DeployNote {
            site,
            env,
            note,
            sync_content,
            updatedb,
            focus,
        } => deploy_lines(site, env, note, *sync_content, *updatedb, *focus),
        Modal::Error { msg } => vec!["Error".into(), msg.clone(), "Enter or Esc closes".into()],
        Modal::LogExpand => {
            let mut lines = vec!["Job log".into()];
            lines.extend(log_lines(state));
            lines.push("Esc closes  Ctrl+C cancels".into());
            lines
        }
        Modal::QuitConfirm => vec![
            "Quit".into(),
            "A job is still running.".into(),
            "Enter cancels it and quits. Esc stays.".into(),
        ],
    })
}

fn doctor_lines(state: &AppState) -> Vec<String> {
    dd_pantheon::doctor::render(
        &state.tools,
        &state.auth,
        &state.catalog,
        &state.doctor_warnings,
    )
    .lines()
    .map(str::to_string)
    .collect()
}

fn login_lines(token: &str, use_env_token: bool, env_available: bool) -> Vec<String> {
    let masked = if use_env_token && env_available {
        "(env TERMINUS_MACHINE_TOKEN)".to_string()
    } else {
        mask(token)
    };
    let mut lines = vec![
        "Login".into(),
        "Machine token (never written to config).".into(),
        "token visible in ps until login exits".into(),
        format!("token {masked}"),
    ];
    if env_available {
        lines.push(format!(
            "{} use env token",
            if use_env_token { "[x]" } else { "[ ]" }
        ));
    }
    lines.push("Enter login   Esc cancel".into());
    lines
}

fn livegate_lines(
    plan: &dd_pantheon::plan::CommandPlan,
    expected: &str,
    typed: &str,
) -> Vec<String> {
    let matched = !expected.is_empty() && typed == expected;
    let word = if expected.is_empty() {
        "the word"
    } else {
        expected
    };
    let hint = if matched {
        "Enter runs this command. Esc cancels."
    } else if !typed.is_empty() && !expected.starts_with(typed) {
        "That does not match. Esc cancels."
    } else {
        "Enter does nothing until the word matches."
    };
    vec![
        "LiveGate".into(),
        format!("type {word} to run"),
        plan.redacted_shell_line(),
        plan.why.clone(),
        format!("typed {typed}"),
        hint.into(),
    ]
}

fn filter_lines(state: &AppState, query: &str, selected: usize) -> Vec<String> {
    let mut lines = vec![format!("Filter /{query}")];
    let hits = state.filter_hits(query);
    if hits.is_empty() {
        lines.push("(no matches)".into());
        return lines;
    }
    for (index, hit) in hits.iter().take(12).enumerate() {
        let mark = if index == selected { ">" } else { " " };
        let label = match &hit.env {
            Some(env) => format!("{} {env}", hit.site),
            None => hit.site.clone(),
        };
        lines.push(format!("{mark} {label}"));
    }
    lines
}

fn site_create_lines(form: &dd_pantheon::state::SiteCreateForm) -> Vec<String> {
    let org = form
        .orgs
        .get(form.org_idx)
        .map(|org| org.org_name.as_str())
        .unwrap_or("—");
    let upstream = form
        .upstreams
        .get(form.upstream_idx)
        .map(|item| item.label.as_str())
        .unwrap_or("—");
    vec![
        "Create site".into(),
        format!("name {}", form.name),
        format!("label {}", form.label),
        format!("org {org}"),
        format!("upstream {upstream}"),
        format!("bind {}", flag(form.bind_local)),
        format!("path {}", form.local_path),
        format!("focus {:?}", form.focus),
    ]
}

fn deploy_lines(
    site: &str,
    env: &str,
    note: &str,
    sync_content: bool,
    updatedb: bool,
    focus: u8,
) -> Vec<String> {
    let mut lines = vec![
        format!("Deploy to {site}.{env}"),
        format!("{} note {note}", caret(focus == 0)),
    ];
    if env == "test" {
        lines.push(format!(
            "{} {} --sync-content",
            caret(focus == 1),
            flag(sync_content)
        ));
    }
    let updatedb_focus = if env == "test" { 2 } else { 1 };
    lines.push(format!(
        "{} {} --updatedb",
        caret(focus == updatedb_focus),
        flag(updatedb)
    ));
    lines.push("Tab fields  Enter deploy  Esc cancel".into());
    lines
}

fn cms_lines(form: &dd_pantheon::state::CmsForm) -> Vec<String> {
    vec![
        "CMS".into(),
        format!("target {}", form.target.label()),
        format!("cms {}", form.cms.label()),
        format!("command {}", form.command),
        format!("focus {:?}", form.focus),
        "m is CMS. Month is Shift+M.".into(),
    ]
}

fn palette_lines(
    state: &AppState,
    query: &str,
    selected: usize,
    form: &Option<dd_pantheon::state::PaletteForm>,
) -> Vec<String> {
    if let Some(form) = form {
        let mut lines = vec![format!("Palette form {}", form.entry.name)];
        for (index, (name, value)) in form.args.iter().enumerate() {
            lines.push(format!("{} {name} {value}", caret(form.focus == index)));
        }
        for (index, (name, on)) in form.toggles.iter().enumerate() {
            let at = form.args.len() + index;
            lines.push(format!(
                "{} {} {}",
                caret(form.focus == at),
                flag(*on),
                name
            ));
        }
        if let Some(element) = &form.element {
            let at = form.args.len() + form.toggles.len();
            lines.push(format!("{} element {element}", caret(form.focus == at)));
        }
        lines.push(format!("extra {}", form.extra));
        lines.push(form.entry.description.clone());
        return lines;
    }
    let matches = palette::matches_for(state, query);
    let mut lines = vec![format!(
        "Palette /{query}  {} matches",
        matches
            .iter()
            .filter(|entry| entry.kind != CatalogKind::Hidden)
            .count()
    )];
    for (index, entry) in matches.iter().take(16).enumerate() {
        if entry.kind == CatalogKind::Hidden {
            continue;
        }
        let mark = if index == selected { ">" } else { " " };
        let enabled = lando_command_enabled(state, &entry.name);
        let tail = if enabled { "" } else { "  (unavailable)" };
        lines.push(format!("{mark} {}{tail}", entry.name));
    }
    if matches.is_empty() {
        lines.push("(no catalog match)".into());
    }
    lines
}

fn tag_line(state: &AppState, site: &dd_pantheon::models::Site) -> String {
    let focused = state.focus == FocusPane::Inspector;
    let chips = site
        .tags
        .iter()
        .map(|tag| {
            let selected = state.selected_chip.as_deref() == Some(tag.name.as_str());
            if focused && selected {
                format!("[{} ×]", tag.name)
            } else {
                format!("[{}]", tag.name)
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("tags {chips}")
}

fn display_url(domain: &str) -> String {
    if domain.starts_with("http://") || domain.starts_with("https://") {
        domain.to_string()
    } else {
        format!("https://{domain}")
    }
}

fn sparkline(values: &[u64]) -> String {
    const BARS: &[char] = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let max = values.iter().copied().max().unwrap_or(1).max(1);
    values
        .iter()
        .map(|value| {
            let idx = (*value as u128 * (BARS.len() as u128 - 1) / max as u128) as usize;
            BARS[idx.min(BARS.len() - 1)]
        })
        .collect()
}

fn caret(on: bool) -> &'static str {
    if on { ">" } else { " " }
}

fn flag(on: bool) -> &'static str {
    if on { "[x]" } else { "[ ]" }
}

fn mask(text: &str) -> String {
    "•".repeat(text.chars().count())
}

#[cfg(test)]
mod tests {
    #[test]
    fn page_lines_follows_the_scroll() {
        let lines: Vec<String> = (0..5).map(|n| n.to_string()).collect();
        assert_eq!(super::clamp_scroll(9, 5, 2), 3);
        assert_eq!(
            super::page_lines(&lines, 3, 2),
            vec!["3".to_string(), "4".to_string()]
        );
    }
}

fn compact_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
