use crate::plan::{CommandPlan, PlanTarget, SafetyTier, ToolKind};
use std::path::PathBuf;
use std::time::Duration;

pub fn plan_push(
    binary: PathBuf,
    cwd: PathBuf,
    remote: &str,
    branch: &str,
    site: Option<String>,
) -> CommandPlan {
    CommandPlan {
        tool: ToolKind::Git,
        binary,
        argv: vec!["push".into(), remote.into(), branch.into()],
        cwd: Some(cwd.clone()),
        why: format!("push {branch} to {remote} (git-mode deploy)"),
        safety: SafetyTier::Mutating,
        target: PlanTarget::Local { path: cwd, site },
        dry_run: false,
        timeout: Some(Duration::from_secs(600)),
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_has_cwd_and_no_yes() {
        let plan = plan_push(
            PathBuf::from("git"),
            PathBuf::from("/tmp/acme-wp"),
            "origin",
            "master",
            Some("acme-wp".into()),
        );
        assert_eq!(plan.argv, vec!["push", "origin", "master"]);
        assert_eq!(
            plan.cwd.as_deref(),
            Some(PathBuf::from("/tmp/acme-wp").as_path())
        );
        assert!(!plan.dry_run);
        assert!(!plan.effective_argv().iter().any(|a| a == "--yes"));
        assert_eq!(plan.timeout, Some(Duration::from_secs(600)));
    }
}
