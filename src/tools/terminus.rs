use crate::plan::{CommandPlan, PlanTarget, SafetyTier, ToolKind};
use crate::safety;
use std::path::PathBuf;
use std::time::Duration;

pub fn plan(
    binary: PathBuf,
    argv: Vec<String>,
    why: impl Into<String>,
    safety_tier: SafetyTier,
    target: PlanTarget,
) -> CommandPlan {
    let confirm = safety_tier >= SafetyTier::Mutating;
    let expects_json = argv
        .iter()
        .any(|a| a == "--format=json" || a.starts_with("--format="));
    CommandPlan {
        tool: ToolKind::Terminus,
        binary,
        argv,
        cwd: None,
        why: why.into(),
        safety: safety::apply_live_gate(safety_tier, &target),
        target,
        dry_run: false,
        timeout: Some(Duration::from_secs(60)),
        expects_json,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: confirm,
    }
}
