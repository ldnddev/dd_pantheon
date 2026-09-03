use crate::catalog::{CatalogEntry, demo_catalog, visible};
use crate::config::push_history;
use crate::models::Framework;
use crate::plan::{CommandPlan, PlanTarget, SafetyTier, StagedPlan, ToolKind, WorkflowPlan};
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
    NeedsDiffstat {
        site: String,
        env: String,
        mode: String,
    },
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Message(m) => f.write_str(m),
            Self::NeedsDiffstat { site, env, .. } => {
                write!(
                    f,
                    "connection:set git requires a clean env:diffstat ({site}.{env})"
                )
            }
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

pub fn filter_catalog<'a>(catalog: &'a [CatalogEntry], query: &str) -> Vec<&'a CatalogEntry> {
    let mut hits: Vec<&CatalogEntry> = visible(catalog)
        .filter(|e| subsequence_match(&e.name, query) || subsequence_match(&e.description, query))
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
    if n.starts_with(q) {
        0
    } else if subsequence_match(&n, q) {
        1
    } else {
        2
    }
}

pub fn open(state: &mut AppState) {
    if state.demo && state.catalog.is_empty() {
        state.catalog = demo_catalog();
    }
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
            let on = (entry.name == "env:clone-content" && name == "--cc")
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
            state.current = Some(plan);
            // Palette never auto-runs.
        }
        Err(CatalogError::NeedsDiffstat { site, env, mode }) => {
            push_history(
                &mut state.config.config.history.palette,
                history_line(&name, &args),
            );
            state.config.mark_dirty();
            state.modal = None;
            state.pending_connection_set = Some((site.clone(), env.clone(), mode));
            crate::workflows::deploy::request_diffstat(state, &site, &env);
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
    match name {
        "env:deploy" => route_deploy(state, args),
        "connection:set" => route_connection_set(state, args),
        "env:clone-content" => route_clone(state, args),
        "env:wipe" => route_wipe(state, args),
        "backup:restore" => route_restore(state, args),
        "lando push" => route_lando_push(state, args),
        "lando pull" => route_lando_pull(state),
        "lando rebuild" => route_lando_rebuild(state),
        "lando destroy" => route_lando_destroy(state),
        "multidev:delete" => route_multidev_delete(state, args),
        "domain:remove" => route_domain_remove(state, args),
        _ => Ok(generic_plan(state, name, args)?),
    }
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

fn require_local(state: &AppState) -> Result<(PathBuf, Option<String>), CatalogError> {
    let site = state
        .selected_site()
        .ok_or_else(|| err("select a site with a local path"))?;
    let local = site
        .local
        .as_ref()
        .ok_or_else(|| err("no local path bound"))?;
    Ok((local.path.clone(), Some(site.name.clone())))
}

fn route_deploy(state: &AppState, args: &PaletteArgs) -> Result<StagedPlan, CatalogError> {
    let (site, env) = require_site_env(state, args)?;
    let updatedb = args.has("--updatedb") || crate::workflows::deploy::wants_updatedb(state, &site);
    let sync = env == "test";
    let deploy = crate::workflows::deploy::plan_deploy(
        state.tools.terminus_path(),
        &site,
        &env,
        crate::workflows::deploy::DEFAULT_NOTE,
        sync,
        updatedb,
    );
    if sync {
        let target = PlanTarget::Env {
            site: site.clone(),
            env: env.clone(),
        };
        let mut steps = crate::safety::backup_first(state.tools.terminus_path(), &target);
        steps.push(deploy);
        let safety = steps
            .iter()
            .map(|s| s.safety)
            .max()
            .unwrap_or(SafetyTier::Destructive);
        Ok(StagedPlan::Workflow {
            plan: WorkflowPlan {
                title: format!("deploy {site}.{env}"),
                why: format!("backup-first deploy to {site}.{env}"),
                safety,
                steps,
                stop_on_failure: true,
            },
            step: 0,
        })
    } else {
        Ok(StagedPlan::One(deploy))
    }
}

fn route_connection_set(state: &AppState, args: &PaletteArgs) -> Result<StagedPlan, CatalogError> {
    let (site, env) = require_site_env(state, args)?;
    let mode = args
        .value("mode")
        .map(|s| s.to_string())
        .or_else(|| args.extra.iter().find(|a| !a.starts_with('-')).cloned())
        .ok_or_else(|| err("fill mode (git or sftp)"))?;
    if mode == "git" {
        return Err(CatalogError::NeedsDiffstat { site, env, mode });
    }
    crate::workflows::deploy::plan_connection_set(
        state.tools.terminus_path(),
        &site,
        &env,
        &mode,
        false,
    )
    .map(StagedPlan::One)
    .map_err(|b| err(b.message))
}

fn route_clone(state: &AppState, args: &PaletteArgs) -> Result<StagedPlan, CatalogError> {
    let (mut origin_site, mut origin) = if let Some(raw) = args.value("site_env") {
        parse_site_env(raw).ok_or_else(|| err(format!("invalid origin `{raw}`")))?
    } else {
        match &state.selected {
            TreeSel::Env { site, env } if env != "live" => (site.clone(), "live".into()),
            TreeSel::Env { site, .. } => (site.clone(), "dev".into()),
            TreeSel::Site(site) => (site.clone(), "live".into()),
            TreeSel::None => return Err(err("select an environment")),
        }
    };
    let target = args
        .value("to_environment")
        .or_else(|| args.value("target_env"))
        .map(|s| {
            parse_site_env(s)
                .map(|(_, e)| e)
                .unwrap_or_else(|| s.to_string())
        })
        .or_else(|| match &state.selected {
            TreeSel::Env { env, .. } => Some(env.clone()),
            _ => None,
        })
        .ok_or_else(|| err("fill to_environment"))?;
    if origin_site.is_empty() {
        origin_site = state
            .selected_site()
            .map(|s| s.name.clone())
            .unwrap_or_default();
    }
    if origin == target {
        origin = if target == "live" {
            "dev".into()
        } else {
            "live".into()
        };
    }
    let cc = args.has("--cc");
    let db_only = args.has("--db-only");
    let files_only = args.has("--files-only");
    let updatedb =
        args.has("--updatedb") || crate::workflows::deploy::wants_updatedb(state, &origin_site);
    if db_only && files_only {
        return Err(err("pick db-only or files-only, not both"));
    }
    Ok(StagedPlan::Workflow {
        plan: crate::workflows::content::plan_clone_workflow(
            state.tools.terminus_path(),
            &origin_site,
            &origin,
            &target,
            cc,
            db_only,
            files_only,
            updatedb,
        ),
        step: 0,
    })
}

fn route_wipe(state: &AppState, args: &PaletteArgs) -> Result<StagedPlan, CatalogError> {
    let (site, env) = require_site_env(state, args)?;
    Ok(StagedPlan::Workflow {
        plan: crate::workflows::content::plan_wipe(state.tools.terminus_path(), &site, &env),
        step: 0,
    })
}

fn route_restore(state: &AppState, args: &PaletteArgs) -> Result<StagedPlan, CatalogError> {
    let (site, env) = require_site_env(state, args)?;
    let file = args
        .value("file")
        .map(|s| s.to_string())
        .or_else(|| {
            args.extra.iter().find_map(|a| {
                a.strip_prefix("--file=")
                    .map(|s| s.to_string())
                    .or_else(|| (a == "--file").then(|| String::new()))
            })
        })
        .filter(|s| !s.is_empty());
    Ok(StagedPlan::Workflow {
        plan: crate::workflows::backup::restore_workflow(
            state.tools.terminus_path(),
            &site,
            &env,
            file.as_deref(),
        ),
        step: 0,
    })
}

fn route_lando_push(state: &AppState, args: &PaletteArgs) -> Result<StagedPlan, CatalogError> {
    let (path, site) = require_local(state)?;
    let mut code = "dev";
    let mut database = "none";
    let mut files = "none";
    for a in &args.extra {
        if let Some(v) = a.strip_prefix("--code=") {
            code = v;
        } else if let Some(v) = a.strip_prefix("--database=") {
            database = v;
        } else if let Some(v) = a.strip_prefix("--files=") {
            files = v;
        }
    }
    Ok(StagedPlan::One(crate::workflows::local::plan_push(
        state.tools.lando_path(),
        path,
        site,
        code,
        database,
        files,
    )))
}

fn route_lando_pull(state: &AppState) -> Result<StagedPlan, CatalogError> {
    let (path, site) = require_local(state)?;
    Ok(StagedPlan::One(crate::workflows::local::plan_pull(
        state.tools.lando_path(),
        path,
        site,
        "none",
        "live",
        "live",
    )))
}

fn route_lando_rebuild(state: &AppState) -> Result<StagedPlan, CatalogError> {
    let (path, site) = require_local(state)?;
    Ok(StagedPlan::One(crate::workflows::local::plan_rebuild(
        state.tools.lando_path(),
        path,
        site,
    )))
}

fn route_lando_destroy(state: &AppState) -> Result<StagedPlan, CatalogError> {
    let (path, site) = require_local(state)?;
    Ok(StagedPlan::One(crate::workflows::local::plan_destroy(
        state.tools.lando_path(),
        path,
        site,
    )))
}

fn route_multidev_delete(state: &AppState, args: &PaletteArgs) -> Result<StagedPlan, CatalogError> {
    let (site, env) = require_site_env(state, args)?;
    if matches!(env.as_str(), "dev" | "test" | "live") {
        return Err(err(
            "multidev:delete is for Multidev envs, not dev/test/live",
        ));
    }
    Ok(StagedPlan::One(crate::workflows::multidev::plan_delete(
        state.tools.terminus_path(),
        &site,
        &env,
        args.has("--delete-branch"),
    )))
}

fn route_domain_remove(state: &AppState, args: &PaletteArgs) -> Result<StagedPlan, CatalogError> {
    let (site, env) = require_site_env(state, args)?;
    let domain = args
        .value("domain")
        .map(|s| s.to_string())
        .or_else(|| args.extra.iter().find(|a| !a.starts_with('-')).cloned())
        .ok_or_else(|| err("fill domain"))?;
    Ok(StagedPlan::One(
        crate::workflows::domains::plan_domain_remove(
            state.tools.terminus_path(),
            &site,
            &env,
            &domain,
        ),
    ))
}

fn generic_plan(
    state: &AppState,
    name: &str,
    args: &PaletteArgs,
) -> Result<StagedPlan, CatalogError> {
    let lando = name.starts_with("lando ");
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
            vec![name.to_string()],
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
}
