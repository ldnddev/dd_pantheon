use crate::models::{ActionItem, Framework};
use crate::plan::{CommandPlan, PlanTarget, SafetyTier, StagedPlan, ToolKind};
use crate::state::{AppState, Modal, TreeSel};
use crate::toast::ToastLevel;
use std::path::PathBuf;
use std::time::Duration;

const MUTATE_TIMEOUT: Duration = Duration::from_secs(600);

pub fn is_multidev(env: &str) -> bool {
    !matches!(env, "dev" | "test" | "live")
}

pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    name.len() <= 11
        && first.is_ascii_lowercase()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && is_multidev(name)
}

pub fn multidev_ok(state: &AppState, site: &str) -> bool {
    state
        .site(site)
        .and_then(|s| s.overlay.as_ref())
        .map(|o| o.multidev_ok)
        .unwrap_or(true)
}

pub fn plan_create(terminus: PathBuf, site: &str, source: &str, name: &str) -> CommandPlan {
    let site_env = format!("{site}.{source}");
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec!["multidev:create".into(), site_env, name.into()],
        cwd: None,
        why: format!("create multidev {name} from {site}.{source}"),
        safety: SafetyTier::Mutating,
        target: PlanTarget::Env {
            site: site.to_string(),
            env: name.to_string(),
        },
        dry_run: false,
        timeout: Some(MUTATE_TIMEOUT),
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: true,
    }
}

pub fn plan_merge_to_dev(
    terminus: PathBuf,
    site: &str,
    branch: &str,
    updatedb: bool,
) -> CommandPlan {
    let site_env = format!("{site}.{branch}");
    let mut argv = vec!["multidev:merge-to-dev".into(), site_env];
    if updatedb {
        argv.push("--updatedb".into());
    }
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv,
        cwd: None,
        why: format!("merge {site}.{branch} into dev"),
        safety: SafetyTier::Mutating,
        target: PlanTarget::Env {
            site: site.to_string(),
            env: "dev".into(),
        },
        dry_run: false,
        timeout: Some(MUTATE_TIMEOUT),
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: true,
    }
}

pub fn plan_delete(
    terminus: PathBuf,
    site: &str,
    branch: &str,
    delete_branch: bool,
) -> CommandPlan {
    let site_env = format!("{site}.{branch}");
    let mut argv = vec!["multidev:delete".into(), site_env];
    if delete_branch {
        argv.push("--delete-branch".into());
    }
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv,
        cwd: None,
        why: format!("delete multidev {site}.{branch}"),
        safety: SafetyTier::Destructive,
        target: PlanTarget::Env {
            site: site.to_string(),
            env: branch.to_string(),
        },
        dry_run: false,
        timeout: Some(MUTATE_TIMEOUT),
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: true,
    }
}

pub fn actions(state: &AppState) -> Vec<ActionItem> {
    let Some(site) = state.selected_site() else {
        return vec![];
    };
    let mut out = Vec::new();
    if multidev_ok(state, &site.name) {
        out.push(ActionItem {
            id: "multidev-create",
            label: "multidev create",
        });
    }
    if let TreeSel::Env { env, .. } = &state.selected {
        if is_multidev(env) {
            out.push(ActionItem {
                id: "multidev-merge",
                label: "merge to dev",
            });
            out.push(ActionItem {
                id: "multidev-delete",
                label: "delete multidev",
            });
        }
    }
    out
}

pub fn open_create(state: &mut AppState) -> bool {
    let Some(site) = state.selected_site() else {
        state.show_toast(ToastLevel::Warning, "select a site");
        return false;
    };
    if site.frozen {
        state.show_toast(ToastLevel::Warning, "site frozen — multidev unavailable");
        return false;
    }
    if !multidev_ok(state, &site.name) {
        state.show_toast(
            ToastLevel::Warning,
            "multidev disabled for this site (palette only in PR 13)",
        );
        return false;
    }
    let sources: Vec<String> = state
        .envs
        .get(&site.name)
        .map(|envs| envs.iter().map(|e| e.id.clone()).collect())
        .unwrap_or_else(|| vec!["live".into(), "dev".into(), "test".into()]);
    let source_idx = sources.iter().position(|e| e == "live").unwrap_or(0);
    state.modal = Some(Modal::MultidevCreate {
        site: site.name.clone(),
        name: String::new(),
        sources,
        source_idx,
    });
    false
}

pub fn submit_create(state: &mut AppState, site: String, name: String, source: String) {
    let name = name.trim().to_string();
    if !valid_name(&name) {
        state.show_toast(
            ToastLevel::Warning,
            "multidev name: ≤11 chars, lowercase alnum/dashes, not dev/test/live",
        );
        return;
    }
    let plan = plan_create(state.tools.terminus_path(), &site, &source, &name);
    state.current = Some(StagedPlan::One(plan));
    crate::workflows::request_run(state);
}

pub fn stage_merge(state: &mut AppState) -> bool {
    let Some((site, env)) = selected_multidev(state) else {
        state.show_toast(ToastLevel::Warning, "select a multidev environment");
        return false;
    };
    let updatedb = state
        .site(&site)
        .is_some_and(|s| s.framework == Framework::Drupal);
    let plan = plan_merge_to_dev(state.tools.terminus_path(), &site, &env, updatedb);
    state.current = Some(StagedPlan::One(plan));
    true
}

pub fn stage_delete(state: &mut AppState) -> bool {
    let Some((site, env)) = selected_multidev(state) else {
        state.show_toast(ToastLevel::Warning, "select a multidev environment");
        return false;
    };
    let plan = plan_delete(state.tools.terminus_path(), &site, &env, true);
    state.current = Some(StagedPlan::One(plan));
    true
}

fn selected_multidev(state: &AppState) -> Option<(String, String)> {
    match &state.selected {
        TreeSel::Env { site, env } if is_multidev(env) => Some((site.clone(), env.clone())),
        _ => None,
    }
}

pub fn on_env_mutate(state: &mut AppState, site: &str) {
    crate::workflows::inventory::request_env_list(state, site, true);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_rules() {
        assert!(valid_name("feat-x"));
        assert!(valid_name("a"));
        assert!(valid_name("feat-123456")); // 11
        assert!(!valid_name("feat-1234567"));
        assert!(!valid_name("Feat"));
        assert!(!valid_name("dev"));
        assert!(!valid_name("test"));
        assert!(!valid_name("live"));
        assert!(!valid_name(""));
    }

    #[test]
    fn create_argv_and_delete_is_destructive() {
        let create = plan_create(PathBuf::from("terminus"), "acme-wp", "live", "feat-y");
        assert_eq!(create.argv[0], "multidev:create");
        assert_eq!(create.argv[1], "acme-wp.live");
        assert_eq!(create.argv[2], "feat-y");
        assert_eq!(create.safety, SafetyTier::Mutating);
        let del = plan_delete(PathBuf::from("terminus"), "acme-wp", "feat-x", true);
        assert_eq!(del.safety, SafetyTier::Destructive);
        assert!(del.argv.iter().any(|a| a == "--delete-branch"));
    }
}
