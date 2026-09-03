use crate::jobs::JobKind;
use crate::models::{ActionItem, LocalApp};
use crate::plan::{CommandPlan, PlanTarget, SafetyTier, StagedPlan};
use crate::state::{AppState, TreeSel};
use crate::toast::ToastLevel;
use crate::tools::lando::{self, LandoPeek};
use crate::workflows::start_job;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const DEBOUNCE: Duration = Duration::from_millis(150);
const READ_TIMEOUT: Duration = Duration::from_secs(60);

pub fn plan_cmd(
    lando: PathBuf,
    path: PathBuf,
    site: Option<String>,
    cmd: &str,
    safety: SafetyTier,
    why: impl Into<String>,
) -> CommandPlan {
    lando::plan(
        lando,
        vec![cmd.into()],
        why,
        safety,
        PlanTarget::Local { path, site },
    )
}

pub fn plan_start(lando: PathBuf, path: PathBuf, site: Option<String>) -> CommandPlan {
    plan_cmd(
        lando,
        path.clone(),
        site,
        "start",
        SafetyTier::Mutating,
        format!("lando start in {}", path.display()),
    )
}

pub fn plan_stop(lando: PathBuf, path: PathBuf, site: Option<String>) -> CommandPlan {
    plan_cmd(
        lando,
        path.clone(),
        site,
        "stop",
        SafetyTier::Mutating,
        format!("lando stop in {}", path.display()),
    )
}

pub fn plan_restart(lando: PathBuf, path: PathBuf, site: Option<String>) -> CommandPlan {
    plan_cmd(
        lando,
        path.clone(),
        site,
        "restart",
        SafetyTier::Mutating,
        format!("lando restart in {}", path.display()),
    )
}

pub fn plan_info(lando: PathBuf, path: PathBuf, site: Option<String>) -> CommandPlan {
    let mut plan = plan_cmd(
        lando,
        path.clone(),
        site,
        "info",
        SafetyTier::ReadOnly,
        format!("lando info in {}", path.display()),
    );
    plan.argv.push("--format=json".into());
    plan.timeout = Some(READ_TIMEOUT);
    plan.expects_json = true;
    plan.confirm_with_yes = false;
    plan
}

pub fn plan_logs(lando: PathBuf, path: PathBuf, site: Option<String>) -> CommandPlan {
    let mut plan = plan_cmd(
        lando,
        path.clone(),
        site,
        "logs",
        SafetyTier::ReadOnly,
        format!("lando logs in {}", path.display()),
    );
    plan.timeout = Some(READ_TIMEOUT);
    plan.confirm_with_yes = false;
    plan
}

pub fn plan_list(lando: PathBuf) -> CommandPlan {
    let mut plan = lando::plan(
        lando,
        vec!["list".into(), "--format=json".into()],
        "list running lando apps",
        SafetyTier::ReadOnly,
        PlanTarget::None,
    );
    plan.timeout = Some(READ_TIMEOUT);
    plan.expects_json = true;
    plan.confirm_with_yes = false;
    plan
}

pub fn plan_rebuild(lando: PathBuf, path: PathBuf, site: Option<String>) -> CommandPlan {
    plan_cmd(
        lando,
        path.clone(),
        site,
        "rebuild",
        SafetyTier::Destructive,
        format!("rebuild app in {} (preserves data)", path.display()),
    )
}

pub fn plan_destroy(lando: PathBuf, path: PathBuf, site: Option<String>) -> CommandPlan {
    plan_cmd(
        lando,
        path.clone(),
        site,
        "destroy",
        SafetyTier::Destructive,
        format!("destroy local app in {} (deletes local DB)", path.display()),
    )
}

pub fn plan_poweroff(lando: PathBuf) -> CommandPlan {
    lando::plan(
        lando,
        vec!["poweroff".into()],
        "stop all lando containers (global)",
        SafetyTier::Mutating,
        PlanTarget::None,
    )
}

pub fn plan_pull(
    lando: PathBuf,
    path: PathBuf,
    site: Option<String>,
    code: &str,
    database: &str,
    files: &str,
) -> CommandPlan {
    lando::plan(
        lando,
        vec![
            "pull".into(),
            format!("--code={code}"),
            format!("--database={database}"),
            format!("--files={files}"),
        ],
        format!(
            "pull into {} (overwrites local DB/files unless none)",
            path.display()
        ),
        SafetyTier::Destructive,
        PlanTarget::Local { path, site },
    )
}

pub fn plan_push(
    lando: PathBuf,
    path: PathBuf,
    site: Option<String>,
    code: &str,
    database: &str,
    files: &str,
) -> CommandPlan {
    let db_on = database != "none";
    let safety = if db_on {
        SafetyTier::LiveGate
    } else {
        SafetyTier::Mutating
    };
    let why = if db_on {
        format!(
            "push from {} including database={database} (LiveGate: type database)",
            path.display()
        )
    } else {
        format!("code-only push from {} (database=none)", path.display())
    };
    lando::plan(
        lando,
        vec![
            "push".into(),
            format!("--code={code}"),
            format!("--database={database}"),
            format!("--files={files}"),
        ],
        why,
        safety,
        PlanTarget::Local { path, site },
    )
}

fn local_target(state: &AppState) -> Option<(PathBuf, Option<String>, bool)> {
    let site = state.selected_site()?;
    let local = site.local.as_ref()?;
    let pantheon = local
        .recipe
        .as_deref()
        .is_some_and(|r| r.eq_ignore_ascii_case("pantheon"));
    Some((local.path.clone(), Some(site.name.clone()), pantheon))
}

fn require_local(state: &mut AppState) -> Option<(PathBuf, Option<String>, bool)> {
    let Some(t) = local_target(state) else {
        state.show_toast(ToastLevel::Warning, "no local path bound");
        return None;
    };
    Some(t)
}

pub fn stage_start(state: &mut AppState) -> bool {
    let Some((path, site, _)) = require_local(state) else {
        return false;
    };
    state.current = Some(StagedPlan::One(plan_start(
        state.tools.lando_path(),
        path,
        site,
    )));
    true
}

pub fn stage_stop(state: &mut AppState) -> bool {
    let Some((path, site, _)) = require_local(state) else {
        return false;
    };
    state.current = Some(StagedPlan::One(plan_stop(
        state.tools.lando_path(),
        path,
        site,
    )));
    true
}

pub fn stage_restart(state: &mut AppState) -> bool {
    let Some((path, site, _)) = require_local(state) else {
        return false;
    };
    state.current = Some(StagedPlan::One(plan_restart(
        state.tools.lando_path(),
        path,
        site,
    )));
    true
}

pub fn stage_logs(state: &mut AppState) -> bool {
    let Some((path, site, _)) = require_local(state) else {
        return false;
    };
    state.current = Some(StagedPlan::One(plan_logs(
        state.tools.lando_path(),
        path,
        site,
    )));
    true
}

pub fn stage_rebuild(state: &mut AppState) -> bool {
    let Some((path, site, _)) = require_local(state) else {
        return false;
    };
    state.current = Some(StagedPlan::One(plan_rebuild(
        state.tools.lando_path(),
        path,
        site,
    )));
    true
}

pub fn stage_destroy(state: &mut AppState) -> bool {
    let Some((path, site, _)) = require_local(state) else {
        return false;
    };
    state.current = Some(StagedPlan::One(plan_destroy(
        state.tools.lando_path(),
        path,
        site,
    )));
    true
}

pub fn stage_poweroff(state: &mut AppState) -> bool {
    state.current = Some(StagedPlan::One(plan_poweroff(state.tools.lando_path())));
    true
}

pub fn stage_pull(state: &mut AppState) -> bool {
    let Some((path, site, pantheon)) = require_local(state) else {
        return false;
    };
    if !pantheon {
        state.show_toast(
            ToastLevel::Warning,
            "recipe is not pantheon — pull/push hidden",
        );
        return false;
    }
    state.current = Some(StagedPlan::One(plan_pull(
        state.tools.lando_path(),
        path,
        site,
        "none",
        "live",
        "live",
    )));
    true
}

pub fn stage_push(state: &mut AppState, with_db: bool) -> bool {
    let Some((path, site, pantheon)) = require_local(state) else {
        return false;
    };
    if !pantheon {
        state.show_toast(
            ToastLevel::Warning,
            "recipe is not pantheon — pull/push hidden",
        );
        return false;
    }
    let db = if with_db {
        match &state.selected {
            TreeSel::Env { env, .. } => env.as_str(),
            _ => "live",
        }
    } else {
        "none"
    };
    state.current = Some(StagedPlan::One(plan_push(
        state.tools.lando_path(),
        path,
        site,
        "dev",
        db,
        "none",
    )));
    true
}

pub fn bind_root(state: &mut AppState, path: &Path) {
    let peek = lando::peek_lando_yml(path).ok();
    if let Some(p) = &peek {
        if p.recipe.is_some() && !p.is_pantheon() {
            state.show_toast(
                ToastLevel::Warning,
                format!(
                    "recipe is {} — pull/push hidden; global lando still allowed",
                    p.recipe.as_deref().unwrap_or("unknown")
                ),
            );
        }
    } else if !path.join(".lando.yml").exists() && !path.ends_with(".lando.yml") {
        state.show_toast(ToastLevel::Warning, "no .lando.yml — path bound anyway");
    }
    let local = local_from_peek(path, peek.as_ref());
    let site_name = local
        .terminus_site
        .clone()
        .or_else(|| local.lando_name.clone());
    if let Some(name) = site_name.clone() {
        state
            .config
            .config
            .locals
            .insert(name.clone(), path.display().to_string());
        state.config.mark_dirty();
        if attach_local(state, &name, local.clone()) {
            refresh_actions(state);
            return;
        }
    }
    if state.demo {
        if attach_local(state, "acme-wp", local.clone()) {
            refresh_actions(state);
            return;
        }
    }
    state.pending_local = Some(local);
    refresh_actions(state);
}

fn local_from_peek(path: &Path, peek: Option<&LandoPeek>) -> LocalApp {
    LocalApp {
        path: path.to_path_buf(),
        lando_name: peek.and_then(|p| p.name.clone()),
        recipe: peek.and_then(|p| p.recipe.clone()),
        framework: peek.and_then(|p| p.framework),
        terminus_site: peek.and_then(|p| p.site.clone()),
        running: None,
        url: None,
    }
}

fn attach_local(state: &mut AppState, site: &str, local: LocalApp) -> bool {
    let Some(rec) = state.sites.iter_mut().find(|s| s.name == site) else {
        return false;
    };
    if rec.overlay.as_ref().is_none_or(|o| o.cms.is_none()) {
        if let Some(cms) = local.framework {
            let overlay = rec.overlay.get_or_insert(crate::models::SiteOverlay {
                cms: None,
                multidev_ok: true,
                composer_managed: false,
                git_branch: None,
                local_path: None,
            });
            if overlay.cms.is_none() {
                overlay.cms = Some(cms);
            }
            if overlay.local_path.is_none() {
                overlay.local_path = Some(local.path.clone());
            }
        }
    }
    rec.local = Some(local);
    true
}

pub fn reattach(state: &mut AppState, old: HashMap<String, LocalApp>) {
    for (name, local) in old {
        let _ = attach_local(state, &name, local);
    }
    let locals = state.config.config.locals.clone();
    for (name, path) in locals {
        if state
            .sites
            .iter()
            .any(|s| s.name == name && s.local.is_some())
        {
            continue;
        }
        let path = PathBuf::from(path);
        let peek = lando::peek_lando_yml(&path).ok();
        let _ = attach_local(state, &name, local_from_peek(&path, peek.as_ref()));
    }
    if let Some(pending) = state.pending_local.clone() {
        let key = pending
            .terminus_site
            .clone()
            .or_else(|| pending.lando_name.clone());
        if let Some(name) = key {
            if attach_local(state, &name, pending) {
                state.pending_local = None;
            }
        }
    }
    refresh_actions(state);
}

pub fn refresh_actions(state: &mut AppState) {
    let has_local = state
        .selected_site()
        .and_then(|s| s.local.as_ref())
        .is_some();
    let pantheon = state.selected_site().is_some_and(|s| {
        s.local
            .as_ref()
            .and_then(|l| l.recipe.as_deref())
            .is_some_and(|r| r.eq_ignore_ascii_case("pantheon"))
    });
    let mut actions = crate::models::default_actions();
    if has_local {
        actions.extend([
            ActionItem {
                id: "lando-stop",
                label: "lando stop",
            },
            ActionItem {
                id: "lando-restart",
                label: "lando restart",
            },
            ActionItem {
                id: "lando-logs",
                label: "lando logs",
            },
            ActionItem {
                id: "lando-rebuild",
                label: "lando rebuild",
            },
            ActionItem {
                id: "lando-destroy",
                label: "lando destroy",
            },
        ]);
        if pantheon {
            actions.extend([
                ActionItem {
                    id: "lando-pull",
                    label: "lando pull",
                },
                ActionItem {
                    id: "lando-push",
                    label: "lando push",
                },
                ActionItem {
                    id: "lando-push-db",
                    label: "lando push db",
                },
            ]);
        }
    }
    actions.extend(crate::workflows::multidev::actions(state));
    actions.extend(crate::workflows::content::actions(state));
    actions.extend(crate::workflows::domains::actions(state));
    actions.push(ActionItem {
        id: "lando-poweroff",
        label: "lando poweroff",
    });
    state.actions = actions;
    if state
        .action_state
        .selected()
        .is_none_or(|i| i >= state.actions.len())
    {
        state.action_state.select(Some(0));
    }
}

pub fn on_selection_changed(state: &mut AppState) {
    refresh_actions(state);
    if state.demo {
        return;
    }
    if local_target(state).is_some() {
        state.pending_lando = Some(Instant::now());
    } else {
        state.pending_lando = None;
    }
}

pub fn flush_debounce(state: &mut AppState) {
    if state.demo {
        return;
    }
    if let Some(at) = state.pending_lando {
        if at.elapsed() >= DEBOUNCE {
            state.pending_lando = None;
            request_status(state, false);
        }
    }
}

pub fn refresh_selected(state: &mut AppState) {
    request_status(state, true);
}

fn request_status(state: &mut AppState, force: bool) {
    if state.demo || state.tools.lando.is_none() {
        return;
    }
    let list_key = "lando:list".to_string();
    if force || !state.inflight_readonly.contains_key(&list_key) {
        let plan = plan_list(state.tools.lando_path());
        if let Some(id) = start_job(state, plan, JobKind::LandoList) {
            state.inflight_readonly.insert(list_key, id);
        }
    }
    let Some((path, site, _)) = local_target(state) else {
        return;
    };
    let site_name = site.clone().unwrap_or_default();
    let info_key = format!("lando:info:{site_name}");
    if force || !state.inflight_readonly.contains_key(&info_key) {
        let plan = plan_info(state.tools.lando_path(), path, site);
        if let Some(id) = start_job(
            state,
            plan,
            JobKind::LandoInfo {
                site: site_name.clone(),
            },
        ) {
            state.inflight_readonly.insert(info_key, id);
        }
    }
}

pub fn apply_list(state: &mut AppState, json: &str) {
    match lando::parse_list(json) {
        Ok(rows) => {
            for site in &mut state.sites {
                let Some(local) = site.local.as_mut() else {
                    continue;
                };
                let hit = rows.iter().find(|r| {
                    local.lando_name.as_deref().is_some_and(|n| n == r.name)
                        || r.src.as_ref().is_some_and(|src| src == &local.path)
                        || r.name == site.name
                });
                if let Some(row) = hit {
                    local.running = Some(row.running);
                    if local.lando_name.is_none() {
                        local.lando_name = Some(row.name.clone());
                    }
                }
            }
        }
        Err(err) => state.show_toast(
            ToastLevel::Error,
            format!("lando list parse failed: {err:#}"),
        ),
    }
}

pub fn apply_info(state: &mut AppState, site: &str, json: &str) {
    match lando::parse_info_url(json) {
        Ok(url) => {
            if let Some(local) = state
                .sites
                .iter_mut()
                .find(|s| s.name == site)
                .and_then(|s| s.local.as_mut())
            {
                if let Some(url) = url {
                    local.url = Some(url);
                }
            }
        }
        Err(err) => state.show_toast(
            ToastLevel::Error,
            format!("lando info parse failed: {err:#}"),
        ),
    }
}

pub fn clear_inflight(state: &mut AppState, kind: &JobKind) {
    match kind {
        JobKind::LandoList => {
            state.inflight_readonly.remove("lando:list");
        }
        JobKind::LandoInfo { site } => {
            state
                .inflight_readonly
                .remove(&format!("lando:info:{site}"));
        }
        _ => {}
    }
}

pub fn snapshot_locals(state: &AppState) -> HashMap<String, LocalApp> {
    state
        .sites
        .iter()
        .filter_map(|s| s.local.clone().map(|l| (s.name.clone(), l)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_code_only_is_mutating() {
        let plan = plan_push(
            PathBuf::from("lando"),
            PathBuf::from("/tmp/acme-wp"),
            Some("acme-wp".into()),
            "dev",
            "none",
            "none",
        );
        assert_eq!(plan.safety, SafetyTier::Mutating);
        assert!(plan.argv.iter().any(|a| a == "--database=none"));
        assert!(plan.argv.iter().any(|a| a == "--code=dev"));
        assert_eq!(plan.cwd.as_deref(), Some(Path::new("/tmp/acme-wp")));
        assert!(plan.timeout.is_none());
    }

    #[test]
    fn push_with_db_is_livegate() {
        let plan = plan_push(
            PathBuf::from("lando"),
            PathBuf::from("/tmp/acme-wp"),
            Some("acme-wp".into()),
            "dev",
            "live",
            "none",
        );
        assert_eq!(plan.safety, SafetyTier::LiveGate);
        assert_eq!(crate::safety::live_gate_word(&plan), Some("database"));
        assert!(plan.effective_argv().contains(&"--yes".to_string()));
    }

    #[test]
    fn pull_and_rebuild_are_destructive() {
        let pull = plan_pull(
            PathBuf::from("lando"),
            PathBuf::from("/tmp/acme-wp"),
            None,
            "none",
            "live",
            "live",
        );
        assert_eq!(pull.safety, SafetyTier::Destructive);
        assert!(pull.argv.iter().any(|a| a == "--database=live"));
        let rebuild = plan_rebuild(PathBuf::from("lando"), PathBuf::from("/tmp/x"), None);
        assert_eq!(rebuild.safety, SafetyTier::Destructive);
        let destroy = plan_destroy(PathBuf::from("lando"), PathBuf::from("/tmp/x"), None);
        assert_eq!(destroy.safety, SafetyTier::Destructive);
    }

    #[test]
    fn poweroff_uses_global_slot() {
        let plan = plan_poweroff(PathBuf::from("lando"));
        assert_eq!(plan.target.mutating_slot(), "__global__");
        assert_eq!(plan.safety, SafetyTier::Mutating);
    }
}
