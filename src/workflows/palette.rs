use crate::catalog::{
    CatalogEntry, command_key, demo_catalog, ensure_lando_catalog, is_lando_init, visible,
};
use crate::config::push_history;
use crate::models::Framework;
use crate::plan::{CommandPlan, PlanTarget, SafetyTier, StagedPlan, ToolKind};
use crate::safety::hint_from_name;
use crate::state::{AppState, Modal, PaletteForm, TreeSel};
use crate::toast::ToastLevel;
use std::path::PathBuf;
use std::time::Duration;

pub const RAW_PALETTE_WARNING: &str = "raw palette — no backup-first / diffstat";

const TOGGLE_OPTS: &[&str] = &[
    "--cc",
    "--updatedb",
    "--sync-content",
    "--delete-branch",
    "--db-only",
    "--files-only",
];

const DESTRUCTIVE_NEEDLES: &[&str] = &[
    "wipe",
    "delete",
    "remove",
    "destroy",
    "restore",
    "clone-content",
    "sync-content",
    "rebuild",
    "pull",
];

const MUTATE_TIMEOUT: Duration = Duration::from_secs(600);
const READ_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Default)]
pub struct PaletteArgs {
    pub values: Vec<(String, String)>,
    pub toggles: Vec<String>,
    pub extra: Vec<String>,
    pub element: Option<String>,
}

impl PaletteArgs {
    pub fn value(&self, name: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(k, v)| k == name && !v.is_empty())
            .map(|(_, v)| v.as_str())
    }

    pub fn has(&self, flag: &str) -> bool {
        self.toggles.iter().any(|t| t == flag)
            || self
                .extra
                .iter()
                .any(|a| a == flag || a.starts_with(&format!("{flag}=")))
    }
}

#[derive(Debug)]
pub enum CatalogError {
    Message(String),
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Message(m) => f.write_str(m),
        }
    }
}

/// Every query character appears in order, case-insensitive (dd_dotstore subsequence).
pub fn subsequence_match(haystack: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let mut chars = haystack.chars().flat_map(char::to_lowercase);
    for q in query.chars().flat_map(char::to_lowercase) {
        loop {
            match chars.next() {
                Some(c) if c == q => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

/// Related terms so "cache", "wp", "visits" surface the matching command.
const RELATED: &[(&str, &[&str])] = &[
    ("deploy", &["env:deploy", "lando push"]),
    ("pull", &["lando pull"]),
    ("push", &["lando push"]),
    ("cache", &["env:clear-cache"]),
    ("cc", &["env:clear-cache"]),
    ("wipe", &["env:wipe"]),
    ("clone", &["env:clone-content"]),
    (
        "backup",
        &[
            "backup:create",
            "backup:restore",
            "backup:list",
            "backup:get",
        ],
    ),
    ("restore", &["backup:restore"]),
    (
        "multidev",
        &["multidev:create", "multidev:delete", "multidev:list"],
    ),
    ("wp", &["remote:wp"]),
    ("wordpress", &["remote:wp"]),
    ("drush", &["remote:drush"]),
    ("drupal", &["remote:drush"]),
    ("login", &["auth:login"]),
    ("logout", &["auth:logout"]),
    ("start", &["lando start"]),
    ("stop", &["lando stop"]),
    ("rebuild", &["lando rebuild"]),
    ("destroy", &["lando destroy"]),
    ("metrics", &["env:metrics"]),
    ("visits", &["env:metrics"]),
    ("views", &["env:metrics"]),
    ("domain", &["domain:add", "domain:remove", "domain:list"]),
    ("lock", &["lock:enable", "lock:disable"]),
    ("https", &["https:set"]),
    ("tag", &["tag:add", "tag:remove", "tag:list"]),
    ("cms", &["remote:wp", "remote:drush"]),
];

fn query_tokens(query: &str) -> Vec<String> {
    query
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_ascii_lowercase())
        .collect()
}

fn related_terms_for(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    RELATED
        .iter()
        .filter(|(_, cmds)| {
            cmds.iter().any(|c| {
                let c = c.to_ascii_lowercase();
                n == c || n.contains(&c) || c.contains(&n)
            })
        })
        .map(|(term, _)| *term)
        .collect::<Vec<_>>()
        .join(" ")
}

fn entry_blob(entry: &CatalogEntry) -> String {
    format!(
        "{} {} {} {}",
        entry.name,
        entry.description,
        entry.tool.binary_name(),
        related_terms_for(&entry.name)
    )
}

pub fn catalog_matches(entry: &CatalogEntry, query: &str) -> bool {
    let q = query.trim();
    if q.is_empty() {
        return true;
    }
    if subsequence_match(&entry.name, q) {
        return true;
    }
    let blob = entry_blob(entry).to_ascii_lowercase();
    let q_lc = q.to_ascii_lowercase();
    if blob.contains(&q_lc) {
        return true;
    }
    let tokens = query_tokens(q);
    !tokens.is_empty()
        && tokens
            .iter()
            .all(|t| blob.contains(t) || subsequence_match(&entry.name, t))
}

pub fn filter_catalog<'a>(catalog: &'a [CatalogEntry], query: &str) -> Vec<&'a CatalogEntry> {
    let mut hits: Vec<&CatalogEntry> = visible(catalog)
        .filter(|e| catalog_matches(e, query))
        .collect();
    let q = query.to_ascii_lowercase();
    hits.sort_by(|a, b| {
        rank(&a.name, &q)
            .cmp(&rank(&b.name, &q))
            .then_with(|| a.name.len().cmp(&b.name.len()))
            .then_with(|| a.name.cmp(&b.name))
    });
    hits
}

fn rank(name: &str, q: &str) -> u8 {
    if q.is_empty() {
        return 2;
    }
    let n = name.to_ascii_lowercase();
    if n.starts_with(q.trim()) {
        0
    } else if n.contains(q.trim()) || subsequence_match(&n, q) {
        1
    } else {
        2
    }
}

pub fn open(state: &mut AppState) {
    if state.demo && state.catalog.is_empty() {
        state.catalog = demo_catalog();
    }
    ensure_lando_catalog(&mut state.catalog);
    if state.catalog.is_empty() {
        state.show_toast(
            ToastLevel::Warning,
            "catalog empty — F3 doctor loads terminus list",
        );
    }
    state.modal = Some(Modal::Palette {
        query: String::new(),
        selected: 0,
        form: None,
    });
}

/// `lando init` is always available. Every other Lando command needs a
/// Pantheon-recipe `.lando.yml` in the working directory (or the selected
/// site's bound local path).
pub fn lando_command_enabled(state: &AppState, name: &str) -> bool {
    if !name.to_ascii_lowercase().starts_with("lando ") {
        return true;
    }
    if is_lando_init(name) {
        return true;
    }
    pantheon_lando_available(state)
}

pub fn pantheon_lando_available(state: &AppState) -> bool {
    if let Some(local) = state.selected_site().and_then(|s| s.local.as_ref()) {
        if local_is_pantheon(state, local) {
            return true;
        }
    }
    if let Some(path) = state.local_root_hint() {
        if peek_is_pantheon(&path) {
            return true;
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        if peek_is_pantheon(&cwd) {
            return true;
        }
    }
    false
}

fn local_is_pantheon(state: &AppState, local: &crate::models::LocalApp) -> bool {
    let recipe_pantheon = local
        .recipe
        .as_deref()
        .is_some_and(|r| r.eq_ignore_ascii_case("pantheon"));
    if recipe_pantheon && state.demo {
        return true;
    }
    peek_is_pantheon(&local.path)
}

fn peek_is_pantheon(path: &PathBuf) -> bool {
    crate::tools::lando::peek_lando_yml(path)
        .ok()
        .is_some_and(|p| p.is_pantheon())
}

pub fn form_from_entry(state: &AppState, entry: &CatalogEntry) -> PaletteForm {
    let args = entry
        .arguments
        .iter()
        .filter(|a| a.required)
        .map(|a| (a.name.clone(), prefill_arg(state, &entry.name, &a.name)))
        .collect::<Vec<_>>();
    let mut toggles = Vec::new();
    let mut element = None;
    for opt in &entry.options {
        let name = normalize_opt(&opt.name);
        if name == "--element" {
            element = Some(String::new());
            continue;
        }
        if TOGGLE_OPTS.contains(&name.as_str()) {
            let on = (command_key(&entry.name) == "env:clone-content" && name == "--cc")
                || (name == "--updatedb" && is_drupal(state));
            toggles.push((name, on));
        }
    }
    PaletteForm {
        entry: entry.clone(),
        args,
        toggles,
        element,
        extra: String::new(),
        focus: 0,
    }
}

fn normalize_opt(name: &str) -> String {
    if name.starts_with('-') {
        name.to_string()
    } else {
        format!("--{name}")
    }
}

fn prefill_arg(state: &AppState, command: &str, arg: &str) -> String {
    let command = command_key(command);
    match arg {
        "site_env" | "site_env_id" if command == "env:clone-content" => match &state.selected {
            TreeSel::Env { site, env } if env != "live" => format!("{site}.live"),
            TreeSel::Env { site, .. } => format!("{site}.dev"),
            TreeSel::Site(site) => format!("{site}.live"),
            TreeSel::None => String::new(),
        },
        "site_env" | "site_env_id" => match &state.selected {
            TreeSel::Env { site, env } => format!("{site}.{env}"),
            TreeSel::Site(site) => format!("{site}.dev"),
            TreeSel::None => String::new(),
        },
        "site_id" => state
            .selected_site()
            .map(|s| s.id.clone())
            .unwrap_or_default(),
        "site_name" => state
            .selected_site()
            .map(|s| s.name.clone())
            .unwrap_or_default(),
        "to_environment" | "target_env" => match &state.selected {
            TreeSel::Env { env, .. } => env.clone(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

fn is_drupal(state: &AppState) -> bool {
    state
        .selected_site()
        .is_some_and(|s| s.framework == Framework::Drupal)
}

pub fn args_from_form(form: &PaletteForm) -> Result<PaletteArgs, String> {
    let extra = if form.extra.trim().is_empty() {
        Vec::new()
    } else {
        shlex::split(&form.extra).ok_or_else(|| "could not parse extra argv".to_string())?
    };
    let element = form
        .element
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    Ok(PaletteArgs {
        values: form.args.clone(),
        toggles: form
            .toggles
            .iter()
            .filter(|(_, on)| *on)
            .map(|(n, _)| n.clone())
            .collect(),
        extra,
        element,
    })
}

pub fn history_line(name: &str, args: &PaletteArgs) -> String {
    let mut parts = vec![name.to_string()];
    for (_, v) in &args.values {
        if !v.is_empty() {
            parts.push(v.clone());
        }
    }
    parts.extend(args.toggles.iter().cloned());
    if let Some(el) = &args.element {
        parts.push(format!("--element={el}"));
    }
    parts.extend(args.extra.iter().cloned());
    parts.join(" ")
}

pub fn submit(state: &mut AppState, form: PaletteForm) {
    let name = form.entry.name.clone();
    let args = match args_from_form(&form) {
        Ok(a) => a,
        Err(err) => {
            state.show_toast(ToastLevel::Warning, err);
            state.modal = Some(Modal::Palette {
                query: name,
                selected: 0,
                form: Some(form),
            });
            return;
        }
    };
    match plan_from_catalog(state, &name, &args) {
        Ok(plan) => {
            push_history(
                &mut state.config.config.history.palette,
                history_line(&name, &args),
            );
            state.config.mark_dirty();
            state.modal = None;
            crate::debuglog::plan_staged(&plan);
            state.current = Some(plan);
            // Palette never auto-runs.
        }
        Err(CatalogError::Message(msg)) => {
            state.show_toast(ToastLevel::Warning, msg);
            state.modal = Some(Modal::Palette {
                query: String::new(),
                selected: 0,
                form: Some(form),
            });
        }
    }
}

pub fn plan_from_catalog(
    state: &AppState,
    name: &str,
    args: &PaletteArgs,
) -> Result<StagedPlan, CatalogError> {
    generic_plan(state, name, args)
}

fn err(msg: impl Into<String>) -> CatalogError {
    CatalogError::Message(msg.into())
}

fn require_site_env(
    state: &AppState,
    args: &PaletteArgs,
) -> Result<(String, String), CatalogError> {
    if let Some(raw) = args.value("site_env").or_else(|| args.value("site_env_id")) {
        return parse_site_env(raw).ok_or_else(|| err(format!("invalid site_env `{raw}`")));
    }
    match &state.selected {
        TreeSel::Env { site, env } => Ok((site.clone(), env.clone())),
        TreeSel::Site(site) => Ok((site.clone(), "dev".into())),
        TreeSel::None => Err(err("select an environment (or fill site_env)")),
    }
}

pub fn parse_site_env(raw: &str) -> Option<(String, String)> {
    let raw = raw.trim();
    let (site, env) = raw.split_once('.')?;
    if site.is_empty() || env.is_empty() || env.contains('.') {
        return None;
    }
    Some((site.to_string(), env.to_string()))
}

fn generic_plan(
    state: &AppState,
    name: &str,
    args: &PaletteArgs,
) -> Result<StagedPlan, CatalogError> {
    let lando = name.starts_with("lando ");
    let key = command_key(name);
    let (tool, binary, mut argv) = if lando {
        let cmd = name.trim_start_matches("lando ").trim();
        if cmd.is_empty() {
            return Err(err("empty lando command"));
        }
        (
            ToolKind::Lando,
            state.tools.lando_path(),
            vec![cmd.to_string()],
        )
    } else {
        (
            ToolKind::Terminus,
            state.tools.terminus_path(),
            vec![key.to_string()],
        )
    };
    for (key, val) in &args.values {
        if val.trim().is_empty() {
            return Err(err(format!("fill {key}")));
        }
        argv.push(val.clone());
    }
    argv.extend(args.toggles.iter().cloned());
    if let Some(el) = &args.element {
        argv.push(format!("--element={el}"));
    }
    argv.extend(args.extra.iter().cloned());

    let mut hay = name.to_ascii_lowercase();
    for a in &argv {
        hay.push(' ');
        hay.push_str(&a.to_ascii_lowercase());
    }
    let mut safety = hint_from_name(name);
    if DESTRUCTIVE_NEEDLES
        .iter()
        .any(|n| hay.contains(n) || name.to_ascii_lowercase().contains(n))
    {
        safety = SafetyTier::Destructive;
    }

    let (target, cwd) = if lando {
        match state.selected_site().and_then(|s| s.local.as_ref()) {
            Some(local) => (
                PlanTarget::Local {
                    path: local.path.clone(),
                    site: state.selected_site().map(|s| s.name.clone()),
                },
                Some(local.path.clone()),
            ),
            None => (PlanTarget::None, None),
        }
    } else if let Ok((site, env)) = require_site_env(state, args) {
        (
            PlanTarget::Env {
                site: site.clone(),
                env: env.clone(),
            },
            None,
        )
    } else if let Some(site) = state.selected_site() {
        (
            PlanTarget::Site {
                site: site.name.clone(),
            },
            None,
        )
    } else {
        (PlanTarget::None, None)
    };

    if tool == ToolKind::Terminus
        && target.is_live()
        && safety >= SafetyTier::Mutating
        && safety != SafetyTier::ReadOnly
    {
        safety = SafetyTier::LiveGate;
    }

    let confirm = safety >= SafetyTier::Mutating;
    let timeout = if safety == SafetyTier::ReadOnly {
        Some(READ_TIMEOUT)
    } else {
        Some(MUTATE_TIMEOUT)
    };
    let why = format!("{name} — {RAW_PALETTE_WARNING}");
    Ok(StagedPlan::One(CommandPlan {
        tool,
        binary,
        argv,
        cwd,
        why,
        safety,
        target,
        dry_run: false,
        timeout,
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: confirm,
    }))
}

pub fn matches_for<'a>(state: &'a AppState, query: &str) -> Vec<&'a CatalogEntry> {
    filter_catalog(&state.catalog, query)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subsequence_is_in_order_case_insensitive() {
        assert!(subsequence_match("env:deploy", "edp"));
        assert!(subsequence_match("env:clone-content", "ECC"));
        assert!(!subsequence_match("env:deploy", "pde"));
        assert!(subsequence_match("lando pull", ""));
        assert!(subsequence_match("Clear caches on an environment", "cache"));
    }

    #[test]
    fn related_terms_match_partial_and_tool() {
        let catalog = demo_catalog();
        let cache = filter_catalog(&catalog, "cache");
        assert!(
            cache.iter().any(|e| e.name == "terminus env:clear-cache"),
            "cache should find terminus env:clear-cache"
        );
        let wp = filter_catalog(&catalog, "wp");
        assert!(wp.iter().any(|e| e.name == "terminus remote:wp"));
        let lando_pull = filter_catalog(&catalog, "lando pull");
        assert!(lando_pull.iter().any(|e| e.name == "lando pull"));
        let deploy = filter_catalog(&catalog, "terminus deploy");
        assert!(deploy.iter().any(|e| e.name == "terminus env:deploy"));
        let wipe = filter_catalog(&catalog, "wipe");
        assert!(wipe.iter().any(|e| e.name == "terminus env:wipe"));
        assert!(
            !wipe.iter().any(|e| e.name == "terminus remote:wp"),
            "wipe must not subsequence-match WordPress descriptions"
        );
        let lando = filter_catalog(&catalog, "lando");
        assert!(
            lando.iter().all(|e| e.name.starts_with("lando ")),
            "lando filter should only hit lando-prefixed names"
        );
        assert!(lando.iter().any(|e| e.name == "lando init"));
        assert!(lando.iter().any(|e| e.name == "lando start"));
        let terminus = filter_catalog(&catalog, "terminus");
        assert!(
            terminus
                .iter()
                .filter(|e| e.tool == ToolKind::Terminus)
                .all(|e| e.name.starts_with("terminus ")),
            "terminus filter should list terminus-prefixed names"
        );
        assert!(terminus.iter().any(|e| e.name == "terminus env:deploy"));
    }
}
