use crate::plan::{CommandPlan, PlanTarget, SafetyTier, ToolKind};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SafetyBlock {
    pub message: String,
}

impl std::fmt::Display for SafetyBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SafetyBlock {}

/// Preview and (later) the job runner share this: Mutating needs Enter;
/// Destructive needs the confirm modal; LiveGate needs the typed word.
pub fn gate_passed(tier: SafetyTier, destructive_confirmed: bool, livegate_ok: bool) -> bool {
    match tier {
        SafetyTier::ReadOnly | SafetyTier::Mutating => true,
        SafetyTier::Destructive => destructive_confirmed,
        SafetyTier::LiveGate => livegate_ok,
    }
}

pub fn apply_live_gate(tier: SafetyTier, target: &PlanTarget) -> SafetyTier {
    if target.is_live() && tier >= SafetyTier::Mutating && tier != SafetyTier::ReadOnly {
        // Backup on live stays Mutating (skill: backup is allowed).
        // Deploy/wipe/clone onto live become LiveGate at the workflow layer.
        tier
    } else {
        tier
    }
}

/// One gate word per plan. Lando DB push is `database`, even if dest is live.
pub fn live_gate_word(plan: &CommandPlan) -> Option<&'static str> {
    if plan.safety != SafetyTier::LiveGate {
        return None;
    }
    if plan.tool == ToolKind::Lando && plan.argv.iter().any(|a| a == "push") {
        let db = plan.argv.iter().any(|a| {
            a == "--database"
                || (a.starts_with("--database=") && !a.ends_with("=none") && a != "--database=none")
        });
        if db {
            return Some("database");
        }
    }
    Some("live")
}

pub fn backup_first(terminus: PathBuf, target: &PlanTarget) -> Vec<CommandPlan> {
    let site_env = match target {
        PlanTarget::Env { site, env } => format!("{site}.{env}"),
        PlanTarget::Site { site } => format!("{site}.dev"),
        _ => return vec![],
    };
    let mut create = crate::tools::terminus::plan(
        terminus.clone(),
        vec![
            "backup:create".into(),
            site_env.clone(),
            "--element=all".into(),
        ],
        format!("backup {site_env} before a destructive content op"),
        SafetyTier::Mutating,
        target.clone(),
    );
    create.timeout = Some(Duration::from_secs(600));
    let mut list = crate::tools::terminus::plan(
        terminus,
        vec![
            "backup:list".into(),
            site_env.clone(),
            "--format=json".into(),
        ],
        format!("verify backup exists for {site_env}"),
        SafetyTier::ReadOnly,
        target.clone(),
    );
    list.confirm_with_yes = false;
    vec![create, list]
}

pub fn hint_from_name(name: &str) -> SafetyTier {
    let n = name.to_ascii_lowercase();
    if is_hidden_name(&n) {
        return SafetyTier::ReadOnly;
    }
    const DESTRUCTIVE: &[&str] = &[
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
    if DESTRUCTIVE.iter().any(|needle| n.contains(needle)) {
        return SafetyTier::Destructive;
    }
    const READONLY: &[&str] = &[
        "list", "info", "whoami", "metrics", "logs", "status", "lookup", "version", "help",
        "describe", "diffstat", "code-log", "view",
    ];
    if READONLY.iter().any(|needle| {
        n == *needle
            || n.ends_with(&format!(":{needle}"))
            || n.starts_with(&format!("{needle}:"))
            || n.contains(&format!(":{needle}"))
    }) {
        return SafetyTier::ReadOnly;
    }
    SafetyTier::Mutating
}

pub fn is_hidden_name(name: &str) -> bool {
    matches!(
        name,
        "_complete" | "completion" | "art" | "art:list" | "self:console"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pull_and_rebuild_are_destructive() {
        assert_eq!(hint_from_name("lando pull"), SafetyTier::Destructive);
        assert_eq!(hint_from_name("rebuild"), SafetyTier::Destructive);
        assert_eq!(hint_from_name("env:wipe"), SafetyTier::Destructive);
        assert_eq!(hint_from_name("env:deploy"), SafetyTier::Mutating);
        assert_eq!(hint_from_name("env:metrics"), SafetyTier::ReadOnly);
        assert_eq!(hint_from_name("backup:list"), SafetyTier::ReadOnly);
    }

    #[test]
    fn backup_first_has_create_then_list() {
        let target = PlanTarget::Env {
            site: "acme-wp".into(),
            env: "test".into(),
        };
        let steps = backup_first(PathBuf::from("terminus"), &target);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].argv[0], "backup:create");
        assert_eq!(steps[1].argv[0], "backup:list");
        assert_eq!(steps[0].safety, SafetyTier::Mutating);
        assert_eq!(steps[1].safety, SafetyTier::ReadOnly);
    }

    #[test]
    fn live_gate_word_for_terminus_live() {
        let plan = crate::tools::terminus::plan(
            PathBuf::from("terminus"),
            vec!["env:wipe".into(), "acme-wp.live".into()],
            "wipe",
            SafetyTier::LiveGate,
            PlanTarget::Env {
                site: "acme-wp".into(),
                env: "live".into(),
            },
        );
        assert_eq!(live_gate_word(&plan), Some("live"));
    }

    #[test]
    fn live_gate_word_for_lando_db_push_is_database() {
        let plan = crate::tools::lando::plan(
            PathBuf::from("lando"),
            vec!["push".into(), "--database=live".into()],
            "push db",
            SafetyTier::LiveGate,
            PlanTarget::Local {
                path: PathBuf::from("/tmp/site"),
                site: Some("acme-wp".into()),
            },
        );
        assert_eq!(live_gate_word(&plan), Some("database"));
    }
}
