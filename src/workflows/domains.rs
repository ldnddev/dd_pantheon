use crate::plan::{CommandPlan, PlanTarget, SafetyTier, StagedPlan, ToolKind};
use crate::state::{AppState, TreeSel};
use crate::toast::ToastLevel;
use std::path::PathBuf;
use std::time::Duration;

const MUTATE_TIMEOUT: Duration = Duration::from_secs(600);
const WAKE_TIMEOUT: Duration = Duration::from_secs(60);

pub fn plan_clear_cache(terminus: PathBuf, site: &str, env: &str) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec!["env:clear-cache".into(), site_env.clone()],
        cwd: None,
        why: format!("clear caches on {site_env}"),
        safety: SafetyTier::Mutating,
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

pub fn plan_wake(terminus: PathBuf, site: &str, env: &str) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec!["env:wake".into(), site_env.clone()],
        cwd: None,
        why: format!("ping {site_env} so a sleeping env comes up (ReadOnly)"),
        safety: SafetyTier::ReadOnly,
        target: PlanTarget::Env {
            site: site.to_string(),
            env: env.to_string(),
        },
        dry_run: false,
        timeout: Some(WAKE_TIMEOUT),
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: false,
    }
}

fn selected_env(state: &AppState) -> Option<(String, String)> {
    match &state.selected {
        TreeSel::Env { site, env } => Some((site.clone(), env.clone())),
        _ => None,
    }
}

pub fn stage_clear_cache(state: &mut AppState) -> bool {
    let Some((site, env)) = selected_env(state) else {
        state.show_toast(ToastLevel::Warning, "select an environment");
        return false;
    };
    let plan = plan_clear_cache(state.tools.terminus_path(), &site, &env);
    state.current = Some(StagedPlan::One(plan));
    true
}

pub fn stage_wake(state: &mut AppState) -> bool {
    let Some((site, env)) = selected_env(state) else {
        state.show_toast(ToastLevel::Warning, "select an environment");
        return false;
    };
    let plan = plan_wake(state.tools.terminus_path(), &site, &env);
    state.current = Some(StagedPlan::One(plan));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_is_mutating_wake_is_readonly() {
        let cache = plan_clear_cache(PathBuf::from("terminus"), "acme-wp", "test");
        assert_eq!(cache.argv, vec!["env:clear-cache", "acme-wp.test"]);
        assert_eq!(cache.safety, SafetyTier::Mutating);
        assert!(cache.confirm_with_yes);

        let wake = plan_wake(PathBuf::from("terminus"), "acme-wp", "dev");
        assert_eq!(wake.argv, vec!["env:wake", "acme-wp.dev"]);
        assert_eq!(wake.safety, SafetyTier::ReadOnly);
        assert!(!wake.confirm_with_yes);
    }
}
