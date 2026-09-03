use crate::models::{ActionItem, Framework};
use crate::plan::{CommandPlan, PlanTarget, SafetyTier, StagedPlan, ToolKind, WorkflowPlan};
use crate::state::{AppState, Modal, TreeSel};
use crate::toast::ToastLevel;
use std::path::PathBuf;
use std::time::Duration;

const MUTATE_TIMEOUT: Duration = Duration::from_secs(600);

pub fn plan_clone_content(
    terminus: PathBuf,
    site: &str,
    origin: &str,
    target: &str,
    cc: bool,
    db_only: bool,
    files_only: bool,
    updatedb: bool,
) -> CommandPlan {
    let origin_se = format!("{site}.{origin}");
    let mut argv = vec!["env:clone-content".into(), origin_se, target.into()];
    if cc {
        argv.push("--cc".into());
    }
    if db_only {
        argv.push("--db-only".into());
    }
    if files_only {
        argv.push("--files-only".into());
    }
    if updatedb {
        argv.push("--updatedb".into());
    }
    let safety = if target == "live" {
        SafetyTier::LiveGate
    } else {
        SafetyTier::Destructive
    };
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv,
        cwd: None,
        why: format!("clone {site}.{origin} → {site}.{target} (overwrites target)"),
        safety,
        target: PlanTarget::Env {
            site: site.to_string(),
            env: target.to_string(),
        },
        dry_run: false,
        timeout: Some(MUTATE_TIMEOUT),
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: true,
    }
}

pub fn plan_wipe_step(terminus: PathBuf, site: &str, env: &str) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    let safety = if env == "live" {
        SafetyTier::LiveGate
    } else {
        SafetyTier::Destructive
    };
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec!["env:wipe".into(), site_env.clone()],
        cwd: None,
        why: format!("wipe database and files on {site_env}"),
        safety,
        target: PlanTarget::Env {
            site: site.to_string(),
            env: env.to_string(),
        },
        dry_run: false,
        timeout: Some(MUTATE_TIMEOUT),
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: true,
    }
}

fn backup_first_workflow(
    terminus: PathBuf,
    target: PlanTarget,
    last: CommandPlan,
    title: String,
    why: String,
) -> WorkflowPlan {
    let mut steps = crate::safety::backup_first(terminus, &target);
    steps.push(last);
    let safety = steps
        .iter()
        .map(|s| s.safety)
        .max()
        .unwrap_or(SafetyTier::Destructive);
    WorkflowPlan {
        title,
        why,
        safety,
        steps,
        stop_on_failure: true,
    }
}

pub fn plan_wipe(terminus: PathBuf, site: &str, env: &str) -> WorkflowPlan {
    let target = PlanTarget::Env {
        site: site.to_string(),
        env: env.to_string(),
    };
    let wipe = plan_wipe_step(terminus.clone(), site, env);
    backup_first_workflow(
        terminus,
        target,
        wipe,
        format!("wipe {site}.{env}"),
        format!("backup-first wipe of {site}.{env}"),
    )
}

pub fn plan_clone_workflow(
    terminus: PathBuf,
    site: &str,
    origin: &str,
    target: &str,
    cc: bool,
    db_only: bool,
    files_only: bool,
    updatedb: bool,
) -> WorkflowPlan {
    let tgt = PlanTarget::Env {
        site: site.to_string(),
        env: target.to_string(),
    };
    let clone = plan_clone_content(
        terminus.clone(),
        site,
        origin,
        target,
        cc,
        db_only,
        files_only,
        updatedb,
    );
    backup_first_workflow(
        terminus,
        tgt,
        clone,
        format!("clone {origin} → {site}.{target}"),
        format!("backup-first clone onto {site}.{target}"),
    )
}

pub fn actions(state: &AppState) -> Vec<ActionItem> {
    if !matches!(state.selected, TreeSel::Env { .. }) {
        return vec![];
    }
    vec![
        ActionItem {
            id: "clone-content",
            label: "clone content",
        },
        ActionItem {
            id: "wipe",
            label: "Wipe environment…",
        },
    ]
}

pub fn open_clone(state: &mut AppState) -> bool {
    let Some((site, target)) = selected_env(state) else {
        state.show_toast(ToastLevel::Warning, "select an environment");
        return false;
    };
    if state.selected_site().is_some_and(|s| s.frozen) {
        state.show_toast(ToastLevel::Warning, "site frozen — content ops unavailable");
        return false;
    }
    let origins: Vec<String> = state
        .envs
        .get(&site)
        .map(|envs| envs.iter().map(|e| e.id.clone()).collect())
        .unwrap_or_else(|| vec!["live".into(), "dev".into(), "test".into()]);
    let origin_idx = origins
        .iter()
        .position(|e| e == "live" && e != &target)
        .or_else(|| origins.iter().position(|e| e != &target))
        .unwrap_or(0);
    let updatedb = state
        .site(&site)
        .is_some_and(|s| s.framework == Framework::Drupal);
    state.modal = Some(Modal::CloneContent {
        site,
        target,
        origins,
        origin_idx,
        cc: true,
        db_only: false,
        files_only: false,
        updatedb,
    });
    false
}

pub fn submit_clone(
    state: &mut AppState,
    site: String,
    target: String,
    origin: String,
    cc: bool,
    db_only: bool,
    files_only: bool,
    updatedb: bool,
) {
    if origin == target {
        state.show_toast(ToastLevel::Warning, "origin and target must differ");
        return;
    }
    if db_only && files_only {
        state.show_toast(ToastLevel::Warning, "pick db-only or files-only, not both");
        return;
    }
    state.modal = None;
    let wf = plan_clone_workflow(
        state.tools.terminus_path(),
        &site,
        &origin,
        &target,
        cc,
        db_only,
        files_only,
        updatedb,
    );
    state.current = Some(StagedPlan::Workflow { plan: wf, step: 0 });
    crate::workflows::request_run(state);
}

pub fn stage_wipe(state: &mut AppState) -> bool {
    let Some((site, env)) = selected_env(state) else {
        state.show_toast(ToastLevel::Warning, "select an environment");
        return false;
    };
    if state.selected_site().is_some_and(|s| s.frozen) {
        state.show_toast(ToastLevel::Warning, "site frozen — content ops unavailable");
        return false;
    }
    let wf = plan_wipe(state.tools.terminus_path(), &site, &env);
    state.current = Some(StagedPlan::Workflow { plan: wf, step: 0 });
    true
}

fn selected_env(state: &AppState) -> Option<(String, String)> {
    match &state.selected {
        TreeSel::Env { site, env } => Some((site.clone(), env.clone())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wipe_is_backup_first_and_livegate_on_live() {
        let wf = plan_wipe(PathBuf::from("terminus"), "acme-wp", "live");
        assert_eq!(wf.steps[0].argv[0], "backup:create");
        assert_eq!(wf.steps[1].argv[0], "backup:list");
        assert_eq!(wf.steps[2].argv[0], "env:wipe");
        assert_eq!(wf.steps[2].argv[1], "acme-wp.live");
        assert_eq!(wf.safety, SafetyTier::LiveGate);
        assert_eq!(wf.steps[2].safety, SafetyTier::LiveGate);
        let test = plan_wipe(PathBuf::from("terminus"), "acme-wp", "test");
        assert_eq!(test.safety, SafetyTier::Destructive);
    }

    #[test]
    fn clone_origin_then_target_and_backup_first() {
        let wf = plan_clone_workflow(
            PathBuf::from("terminus"),
            "acme-wp",
            "live",
            "test",
            true,
            false,
            false,
            false,
        );
        assert_eq!(wf.steps[0].argv[0], "backup:create");
        assert_eq!(wf.steps[0].argv[1], "acme-wp.test");
        let clone = wf.steps.last().unwrap();
        assert_eq!(clone.argv[0], "env:clone-content");
        assert_eq!(clone.argv[1], "acme-wp.live");
        assert_eq!(clone.argv[2], "test");
        assert!(clone.argv.iter().any(|a| a == "--cc"));
        assert_eq!(clone.safety, SafetyTier::Destructive);
        let live = plan_clone_content(
            PathBuf::from("terminus"),
            "acme-wp",
            "test",
            "live",
            true,
            false,
            false,
            false,
        );
        assert_eq!(live.safety, SafetyTier::LiveGate);
    }
}
