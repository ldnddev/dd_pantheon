use crate::doctor::AuthState;
use crate::plan::{CommandPlan, PlanTarget, Redact, SafetyTier, ToolKind};
use std::path::PathBuf;
use std::time::Duration;

pub fn plan_login(terminus: PathBuf, token: &str) -> CommandPlan {
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec!["auth:login".into(), format!("--machine-token={token}")],
        cwd: None,
        why: "login with machine token (visible in ps until login exits)".into(),
        safety: SafetyTier::Mutating,
        target: PlanTarget::None,
        dry_run: false,
        timeout: Some(Duration::from_secs(30)),
        expects_json: false,
        extra_env: vec![],
        redact: vec![
            Redact::FlagValue {
                flag: "--machine-token".into(),
            },
            Redact::Exact(token.to_string()),
        ],
        confirm_with_yes: true,
    }
}

pub fn plan_whoami(terminus: PathBuf) -> CommandPlan {
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec!["auth:whoami".into(), "--format=json".into()],
        cwd: None,
        why: "doctor: whoami".into(),
        safety: SafetyTier::ReadOnly,
        target: PlanTarget::None,
        dry_run: false,
        timeout: Some(Duration::from_secs(15)),
        expects_json: false, // empty stdout when logged out is not a JSON bug
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: false,
    }
}

pub fn plan_terminus_list(terminus: PathBuf) -> CommandPlan {
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec!["list".into(), "--format=json".into()],
        cwd: None,
        why: "doctor: terminus list".into(),
        safety: SafetyTier::ReadOnly,
        target: PlanTarget::None,
        dry_run: false,
        timeout: Some(Duration::from_secs(15)),
        expects_json: true,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: false,
    }
}

pub fn plan_version(tool: ToolKind, binary: PathBuf) -> CommandPlan {
    let argv = match tool {
        ToolKind::Terminus => vec!["--version".into()],
        ToolKind::Lando => vec!["version".into()],
        ToolKind::Git => vec!["--version".into()],
    };
    CommandPlan {
        tool,
        binary,
        argv,
        cwd: None,
        why: format!("doctor: {tool:?} version"),
        safety: SafetyTier::ReadOnly,
        target: PlanTarget::None,
        dry_run: false,
        timeout: Some(Duration::from_secs(15)),
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: false,
    }
}

pub fn auth_from_output(exit: i32, stdout: &str, stderr: &str) -> AuthState {
    let stdout = stdout.trim();
    let stderr_l = stderr.to_ascii_lowercase();
    if stdout.is_empty() && (exit == 0 || stderr_l.contains("not logged in")) {
        return AuthState::LoggedOut;
    }
    if stdout.is_empty() {
        return AuthState::LoggedOut;
    }
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(stdout) {
        let email = val
            .get("email")
            .and_then(|v| v.as_str())
            .or_else(|| val.as_str())
            .unwrap_or(stdout)
            .to_string();
        let id = val
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        return AuthState::LoggedIn { email, id };
    }
    AuthState::LoggedIn {
        email: stdout.trim_matches('"').to_string(),
        id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whoami_logged_out_empty_stdout() {
        let auth = auth_from_output(0, "", "You are not logged in.\n");
        assert_eq!(auth, AuthState::LoggedOut);
    }

    #[test]
    fn whoami_logged_in_json() {
        let auth = auth_from_output(0, r#"{"email":"a@b.com","id":"1"}"#, "");
        assert_eq!(
            auth,
            AuthState::LoggedIn {
                email: "a@b.com".into(),
                id: Some("1".into())
            }
        );
    }
}
