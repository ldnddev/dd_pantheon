use crate::plan::{CommandPlan, PlanTarget, SafetyTier, ToolKind};
use std::path::PathBuf;

pub fn plan(
    binary: PathBuf,
    argv: Vec<String>,
    why: impl Into<String>,
    safety_tier: SafetyTier,
    target: PlanTarget,
) -> CommandPlan {
    let cwd = match &target {
        PlanTarget::Local { path, .. } => Some(path.clone()),
        _ => None,
    };
    let confirm = safety_tier >= SafetyTier::Mutating;
    CommandPlan {
        tool: ToolKind::Lando,
        binary,
        argv,
        cwd,
        why: why.into(),
        safety: safety_tier,
        target,
        dry_run: false,
        timeout: None,
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: confirm,
    }
}

pub const PANTHEON_RECIPE_EXTRAS: &[&str] = &[
    "pull",
    "push",
    "terminus",
    "drush",
    "wp",
    "composer",
    "mysql",
    "db-import",
    "db-export",
];
