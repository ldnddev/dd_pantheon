use crate::plan::{CommandPlan, PlanTarget, SafetyTier, ToolKind};
use std::path::PathBuf;
use std::time::Duration;

pub fn plan_push(binary: PathBuf, cwd: PathBuf, remote: &str, branch: &str) -> CommandPlan {
    CommandPlan {
        tool: ToolKind::Git,
        binary,
        argv: vec!["push".into(), remote.into(), branch.into()],
        cwd: Some(cwd.clone()),
        why: format!("push {branch} to {remote} (git-mode deploy)"),
        safety: SafetyTier::Mutating,
        target: PlanTarget::Local {
            path: cwd,
            site: None,
        },
        dry_run: true,
        timeout: Some(Duration::from_secs(600)),
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: false,
    }
}
