use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// PR 2 spawn exception: blocking `Command::output` for detect / versions / list.
/// Not called from the draw tick. PR 3 migrates this onto the job runner.

#[derive(Clone, Debug, Default)]
pub struct Toolset {
    pub terminus: Option<ToolBinary>,
    pub lando: Option<ToolBinary>,
    pub git: Option<ToolBinary>,
}

impl Toolset {
    pub fn terminus_path(&self) -> PathBuf {
        self.terminus
            .as_ref()
            .map(|t| t.path.clone())
            .unwrap_or_else(|| PathBuf::from("terminus"))
    }

    pub fn lando_path(&self) -> PathBuf {
        self.lando
            .as_ref()
            .map(|t| t.path.clone())
            .unwrap_or_else(|| PathBuf::from("lando"))
    }

    pub fn git_path(&self) -> PathBuf {
        self.git
            .as_ref()
            .map(|t| t.path.clone())
            .unwrap_or_else(|| PathBuf::from("git"))
    }

    pub fn terminus_ok(&self) -> bool {
        self.terminus.is_some()
    }
}

#[derive(Clone, Debug)]
pub struct ToolBinary {
    pub path: PathBuf,
    pub version: Option<String>,
}

/// PATH lookup only. Version / list / whoami go through the job runner (PR 3).
pub fn detect_which() -> Toolset {
    Toolset {
        terminus: which_only("terminus"),
        lando: which_only("lando"),
        git: which_only("git"),
    }
}

fn which_only(name: &str) -> Option<ToolBinary> {
    Some(ToolBinary {
        path: which::which(name).ok()?,
        version: None,
    })
}

pub fn detect_tools() -> Toolset {
    Toolset {
        terminus: probe("terminus", &["--version"]),
        lando: probe("lando", &["version"]),
        git: probe("git", &["--version"]),
    }
}

fn probe(name: &str, version_args: &[&str]) -> Option<ToolBinary> {
    let path = which::which(name).ok()?;
    let version = run_output(&path, version_args, None)
        .ok()
        .map(|out| parse_version_line(&String::from_utf8_lossy(&out.stdout)));
    Some(ToolBinary { path, version })
}

pub fn run_output(bin: &Path, args: &[&str], cwd: Option<&Path>) -> Result<Output> {
    let mut cmd = Command::new(bin);
    cmd.args(args).stdin(std::process::Stdio::null());
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    cmd.output()
        .with_context(|| format!("{} {}", bin.display(), args.join(" ")))
}

pub fn parse_version_line(stdout: &str) -> String {
    stdout
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_takes_first_nonempty_line() {
        assert_eq!(
            parse_version_line("Terminus 4.3.2\nmore\n"),
            "Terminus 4.3.2"
        );
        assert_eq!(parse_version_line("\nv3.26.8\n"), "v3.26.8");
    }

    #[test]
    fn git_is_usually_on_path() {
        let tools = detect_tools();
        assert!(
            tools.git.is_some(),
            "git should be on PATH in this environment"
        );
    }
}
