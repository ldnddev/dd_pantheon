use crate::plan::{CommandPlan, PlanTarget, SafetyTier, ToolKind};
use std::path::PathBuf;
use std::time::Duration;

/// Branch names that must not become git options or refspecs (`--force`, `+main:main`).
pub fn safe_branch_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphanumeric() || name.len() > 128 {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.'))
        && !name.contains("..")
        && !name.contains("//")
        && !name.ends_with('/')
        && !name.ends_with('.')
        && !name.ends_with(".lock")
        && !name
            .split('/')
            .any(|seg| seg.is_empty() || seg.ends_with(".lock"))
}

pub fn plan_push(
    binary: PathBuf,
    cwd: PathBuf,
    remote: &str,
    branch: &str,
    site: Option<String>,
) -> Result<CommandPlan, String> {
    if !safe_branch_name(remote) || !safe_branch_name(branch) {
        return Err(format!("refusing git ref `{remote}` / `{branch}`"));
    }
    Ok(CommandPlan {
        tool: ToolKind::Git,
        binary,
        // `--` keeps a ref that looks like a flag from being parsed as an option.
        argv: vec!["push".into(), "--".into(), remote.into(), branch.into()],
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
    })
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
        )
        .expect("branch");
        assert_eq!(plan.argv, vec!["push", "--", "origin", "master"]);
        assert!(safe_branch_name("feature/foo"));
        assert!(!safe_branch_name("--force"));
        assert!(!safe_branch_name("+refs/heads/main"));
        assert!(!safe_branch_name("main:refs/heads/main"));
        assert!(
            plan_push(
                PathBuf::from("git"),
                PathBuf::from("/tmp/acme-wp"),
                "origin",
                "--force",
                None,
            )
            .is_err()
        );
        assert_eq!(
            plan.cwd.as_deref(),
            Some(PathBuf::from("/tmp/acme-wp").as_path())
        );
        assert!(!plan.dry_run);
        assert!(!plan.effective_argv().iter().any(|a| a == "--yes"));
        assert_eq!(plan.timeout, Some(Duration::from_secs(600)));
    }
}
