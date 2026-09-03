use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    Terminus,
    Lando,
    Git,
}

impl ToolKind {
    pub fn binary_name(self) -> &'static str {
        match self {
            Self::Terminus => "terminus",
            Self::Lando => "lando",
            Self::Git => "git",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SafetyTier {
    ReadOnly,
    Mutating,
    Destructive,
    LiveGate,
}

impl SafetyTier {
    pub fn badge(self) -> &'static str {
        match self {
            Self::ReadOnly => "READ-ONLY",
            Self::Mutating => "MUTATING",
            Self::Destructive => "DESTRUCTIVE",
            Self::LiveGate => "LIVEGATE",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PlanTarget {
    None,
    Site { site: String },
    Env { site: String, env: String },
    Local { path: PathBuf, site: Option<String> },
}

impl PlanTarget {
    pub fn env_key(&self) -> Option<String> {
        match self {
            Self::Env { site, env } => Some(format!("{site}.{env}")),
            Self::Local { path, .. } => Some(path.display().to_string()),
            Self::Site { site } => Some(site.clone()),
            Self::None => None,
        }
    }

    pub fn is_live(&self) -> bool {
        matches!(self, Self::Env { env, .. } if env == "live")
    }

    pub fn mutating_slot(&self) -> String {
        match self {
            Self::Env { site, env } => format!("{site}.{env}"),
            Self::Local { path, .. } => path.display().to_string(),
            Self::Site { site } => site.clone(),
            Self::None => "__global__".to_string(),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::None => "—".to_string(),
            Self::Site { site } => site.clone(),
            Self::Env { site, env } => format!("{site}.{env}"),
            Self::Local { path, .. } => path.display().to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Redact {
    FlagValue { flag: String },
    Exact(String),
}

#[derive(Clone, Debug)]
pub struct CommandPlan {
    pub tool: ToolKind,
    pub binary: PathBuf,
    pub argv: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub why: String,
    pub safety: SafetyTier,
    pub target: PlanTarget,
    pub dry_run: bool,
    pub timeout: Option<Duration>,
    pub expects_json: bool,
    pub extra_env: Vec<(String, String)>,
    pub redact: Vec<Redact>,
    pub confirm_with_yes: bool,
}

impl CommandPlan {
    pub fn effective_argv(&self) -> Vec<String> {
        self.argv_with_injection(true)
    }

    pub fn argv_with_injection(&self, gate_passed: bool) -> Vec<String> {
        let mut args = self.argv.clone();
        match self.tool {
            ToolKind::Terminus => {
                if !has_flag(&args, "--no-interaction") && !has_flag(&args, "-n") {
                    args.push("--no-interaction".to_string());
                }
                if self.confirm_with_yes
                    && gate_passed
                    && !has_flag(&args, "--yes")
                    && !has_flag(&args, "-y")
                {
                    args.push("--yes".to_string());
                }
            }
            ToolKind::Lando => {
                if self.confirm_with_yes
                    && gate_passed
                    && !has_flag(&args, "--yes")
                    && !has_flag(&args, "-y")
                {
                    args.push("--yes".to_string());
                }
            }
            ToolKind::Git => {}
        }
        args
    }

    pub fn shell_line(&self) -> String {
        render_shell_line(&self.binary, &self.effective_argv(), self.cwd.as_ref())
    }

    pub fn redacted_shell_line(&self) -> String {
        let argv = redact_argv(&self.effective_argv(), &self.redact);
        render_shell_line(&self.binary, &argv, self.cwd.as_ref())
    }

    pub fn redact_text(&self, text: &str) -> String {
        redact_text(text, &self.redact)
    }
}

#[derive(Clone, Debug)]
pub struct WorkflowPlan {
    pub title: String,
    pub why: String,
    pub safety: SafetyTier,
    pub steps: Vec<CommandPlan>,
    pub stop_on_failure: bool,
}

#[derive(Clone, Debug)]
pub enum StagedPlan {
    One(CommandPlan),
    Workflow { plan: WorkflowPlan, step: usize },
}

impl StagedPlan {
    pub fn safety(&self) -> SafetyTier {
        match self {
            Self::One(p) => p.safety,
            Self::Workflow { plan, .. } => plan.safety,
        }
    }

    pub fn target_label(&self) -> String {
        match self {
            Self::One(p) => p.target.label(),
            Self::Workflow { plan, step } => plan
                .steps
                .get(*step)
                .map(|p| p.target.label())
                .unwrap_or_else(|| "—".to_string()),
        }
    }

    pub fn current(&self) -> Option<&CommandPlan> {
        match self {
            Self::One(p) => Some(p),
            Self::Workflow { plan, step } => plan.steps.get(*step),
        }
    }

    pub fn why(&self) -> &str {
        match self {
            Self::One(p) => &p.why,
            Self::Workflow { plan, .. } => &plan.why,
        }
    }
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn render_shell_line(binary: &Path, argv: &[String], cwd: Option<&PathBuf>) -> String {
    let mut parts = Vec::with_capacity(argv.len() + 1);
    parts.push(binary.display().to_string());
    parts.extend(argv.iter().cloned());
    let joined =
        shlex::try_join(parts.iter().map(|s| s.as_str())).unwrap_or_else(|_| parts.join(" "));
    match cwd {
        Some(cwd) => format!("cd {} && {joined}", cwd.display()),
        None => joined,
    }
}

pub fn redact_argv(argv: &[String], rules: &[Redact]) -> Vec<String> {
    argv.iter()
        .enumerate()
        .map(|(i, arg)| {
            for rule in rules {
                match rule {
                    Redact::Exact(secret) if arg == secret => return "***".to_string(),
                    Redact::FlagValue { flag } => {
                        let prefix = format!("{flag}=");
                        if arg.starts_with(&prefix) {
                            return format!("{flag}=***");
                        }
                        if arg == flag {
                            return flag.clone();
                        }
                        if i > 0 && argv[i - 1] == *flag {
                            return "***".to_string();
                        }
                    }
                    _ => {}
                }
            }
            arg.clone()
        })
        .collect()
}

pub fn redact_text(text: &str, rules: &[Redact]) -> String {
    let mut out = text.to_string();
    for rule in rules {
        match rule {
            Redact::Exact(secret) if !secret.is_empty() => {
                out = out.replace(secret, "***");
            }
            Redact::FlagValue { flag } => {
                let prefix = format!("{flag}=");
                if let Some(idx) = out.find(&prefix) {
                    let rest = &out[idx + prefix.len()..];
                    let end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
                    let value = &rest[..end];
                    if !value.is_empty() {
                        out = out.replace(&format!("{prefix}{value}"), &format!("{flag}=***"));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terminus_plan(argv: &[&str], confirm: bool) -> CommandPlan {
        CommandPlan {
            tool: ToolKind::Terminus,
            binary: PathBuf::from("terminus"),
            argv: argv.iter().map(|s| s.to_string()).collect(),
            cwd: None,
            why: "test".into(),
            safety: SafetyTier::Mutating,
            target: PlanTarget::None,
            dry_run: true,
            timeout: None,
            expects_json: false,
            extra_env: vec![],
            redact: vec![],
            confirm_with_yes: confirm,
        }
    }

    #[test]
    fn terminus_injects_no_interaction_and_yes() {
        let plan = terminus_plan(&["backup:create", "acme.test"], true);
        let argv = plan.effective_argv();
        assert!(argv.contains(&"--no-interaction".to_string()));
        assert!(argv.contains(&"--yes".to_string()));
        assert_eq!(argv.iter().filter(|a| *a == "--yes").count(), 1);
    }

    #[test]
    fn git_never_gets_yes() {
        let plan = CommandPlan {
            tool: ToolKind::Git,
            binary: PathBuf::from("git"),
            argv: vec!["push".into(), "origin".into(), "master".into()],
            cwd: Some(PathBuf::from("/tmp")),
            why: "deploy".into(),
            safety: SafetyTier::Mutating,
            target: PlanTarget::None,
            dry_run: true,
            timeout: None,
            expects_json: false,
            extra_env: vec![],
            redact: vec![],
            confirm_with_yes: true,
        };
        let argv = plan.effective_argv();
        assert!(!argv.iter().any(|a| a == "--yes" || a == "-y" || a == "-n"));
    }

    #[test]
    fn mutating_slot_none_is_global() {
        assert_eq!(PlanTarget::None.mutating_slot(), "__global__");
        assert_eq!(
            PlanTarget::Env {
                site: "acme-wp".into(),
                env: "test".into(),
            }
            .mutating_slot(),
            "acme-wp.test"
        );
    }

    #[test]
    fn redact_machine_token_flag() {
        let plan = CommandPlan {
            tool: ToolKind::Terminus,
            binary: PathBuf::from("terminus"),
            argv: vec!["auth:login".into(), "--machine-token=super-secret".into()],
            cwd: None,
            why: "login".into(),
            safety: SafetyTier::Mutating,
            target: PlanTarget::None,
            dry_run: true,
            timeout: None,
            expects_json: false,
            extra_env: vec![],
            redact: vec![Redact::FlagValue {
                flag: "--machine-token".into(),
            }],
            confirm_with_yes: false,
        };
        let line = plan.redacted_shell_line();
        assert!(line.contains("--machine-token=***"));
        assert!(!line.contains("super-secret"));
    }
}
