use crate::jobs::JobKind;
use crate::models::{ConnectionMode, Framework};
use crate::plan::{CommandPlan, PlanTarget, SafetyTier, StagedPlan, ToolKind, WorkflowPlan};
use crate::safety::SafetyBlock;
use crate::state::{AppState, Modal, TreeSel};
use crate::toast::ToastLevel;
use crate::workflows::start_job;
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;

pub const DEFAULT_NOTE: &str = "Deploy from dd_pantheon";
const WAIT_MAX: u64 = 600;
const WAIT_TIMEOUT: Duration = Duration::from_secs(660);
const MUTATE_TIMEOUT: Duration = Duration::from_secs(600);
const READ_TIMEOUT: Duration = Duration::from_secs(60);

pub fn plan_diffstat(terminus: PathBuf, site: &str, env: &str) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec![
            "env:diffstat".into(),
            site_env.clone(),
            "--format=json".into(),
            "--fields=file,status,deletions,additions".into(),
        ],
        cwd: None,
        why: format!("uncommitted files on {site_env} (blocks connection:set git if dirty)"),
        safety: SafetyTier::ReadOnly,
        target: PlanTarget::Env {
            site: site.to_string(),
            env: env.to_string(),
        },
        dry_run: false,
        timeout: Some(READ_TIMEOUT),
        expects_json: true,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: false,
    }
}

pub fn plan_connection_set(
    terminus: PathBuf,
    site: &str,
    env: &str,
    mode: &str,
    dirty: bool,
) -> Result<CommandPlan, SafetyBlock> {
    if dirty && mode == "git" {
        return Err(SafetyBlock {
            message: "dirty env:diffstat — commit via env:commit or abort".into(),
        });
    }
    let site_env = format!("{site}.{env}");
    Ok(CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec!["connection:set".into(), site_env.clone(), mode.into()],
        cwd: None,
        why: format!("set {site_env} connection mode to {mode}"),
        safety: SafetyTier::Mutating,
        target: PlanTarget::Env {
            site: site.to_string(),
            env: env.to_string(),
        },
        dry_run: false,
        timeout: Some(READ_TIMEOUT),
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: true,
    })
}

pub fn plan_commit(terminus: PathBuf, site: &str, env: &str) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec![
            "env:commit".into(),
            site_env.clone(),
            "--message=Commit from dd_pantheon".into(),
        ],
        cwd: None,
        why: format!("commit SFTP changes on {site_env} so connection:set git can proceed"),
        safety: SafetyTier::Mutating,
        target: PlanTarget::Env {
            site: site.to_string(),
            env: env.to_string(),
        },
        dry_run: false,
        timeout: Some(MUTATE_TIMEOUT),
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: true,
    }
}

pub fn plan_wait(terminus: PathBuf, site: &str, env: &str) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec![
            "workflow:wait".into(),
            site_env.clone(),
            format!("--max={WAIT_MAX}"),
        ],
        cwd: None,
        why: format!("wait up to {WAIT_MAX}s for code sync on {site_env}"),
        safety: SafetyTier::ReadOnly,
        target: PlanTarget::Env {
            site: site.to_string(),
            env: env.to_string(),
        },
        dry_run: false,
        timeout: Some(WAIT_TIMEOUT),
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: false,
    }
}

pub fn plan_deploy(
    terminus: PathBuf,
    site: &str,
    env: &str,
    note: &str,
    sync_content: bool,
    updatedb: bool,
) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    let sync = sync_content && env == "test";
    let mut argv = vec![
        "env:deploy".into(),
        site_env.clone(),
        "--cc".into(),
        format!("--note={note}"),
    ];
    if sync {
        argv.push("--sync-content".into());
    }
    if updatedb {
        argv.push("--updatedb".into());
    }
    let safety = if env == "live" {
        SafetyTier::LiveGate
    } else if sync {
        SafetyTier::Destructive
    } else {
        SafetyTier::Mutating
    };
    let why = if sync {
        format!("deploy code to {site_env} (sync-content from live, backup-first)")
    } else {
        format!("deploy code to {site_env}")
    };
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv,
        cwd: None,
        why,
        safety,
        target: PlanTarget::Env {
            site: site.to_string(),
            env: env.to_string(),
        },
        dry_run: false,
        timeout: Some(MUTATE_TIMEOUT),
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: true,
    }
}

pub fn parse_diffstat(json: &str) -> anyhow::Result<Vec<String>> {
    if json.trim().is_empty() {
        return Ok(vec![]);
    }
    let value: Value = serde_json::from_str(json)?;
    Ok(files_from_value(&value))
}

fn files_from_value(value: &Value) -> Vec<String> {
    match value {
        Value::Array(arr) => arr.iter().filter_map(file_from_value).collect(),
        Value::Object(map) => {
            if let Some(inner) = map.get("diffstat").or_else(|| map.get("data")) {
                return files_from_value(inner);
            }
            if map.get("file").is_some() || map.get("status").is_some() {
                return file_from_value(value).into_iter().collect();
            }
            let mut files = Vec::new();
            for (k, v) in map {
                if let Some(name) = file_from_value(v) {
                    files.push(name);
                } else if v.is_object() || v.is_string() {
                    files.push(k.clone());
                }
            }
            files
        }
        _ => vec![],
    }
}

fn file_from_value(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Object(obj) => obj
            .get("file")
            .or_else(|| obj.get("filename"))
            .and_then(|f| f.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        _ => None,
    }
}

pub fn stage_deploy(state: &mut AppState) -> bool {
    if state.selected_site().is_some_and(|s| s.frozen) {
        state.show_toast(ToastLevel::Warning, "site frozen — deploy unavailable");
        return false;
    }
    let Some((site, env)) = selected_env(state) else {
        state.show_toast(ToastLevel::Warning, "select an environment");
        return false;
    };
    match env.as_str() {
        "test" => stage_test(state, &site, &env),
        "live" => stage_live(state, &site, &env),
        _ => stage_code(state, &site, &env),
    }
}

fn stage_test(state: &mut AppState, site: &str, env: &str) -> bool {
    let updatedb = wants_updatedb(state, site);
    let target = PlanTarget::Env {
        site: site.to_string(),
        env: env.to_string(),
    };
    let mut steps = crate::safety::backup_first(state.tools.terminus_path(), &target);
    let deploy = plan_deploy(
        state.tools.terminus_path(),
        site,
        env,
        DEFAULT_NOTE,
        true,
        updatedb,
    );
    steps.push(deploy);
    let safety = steps
        .iter()
        .map(|s| s.safety)
        .max()
        .unwrap_or(SafetyTier::Destructive);
    state.current = Some(StagedPlan::Workflow {
        plan: WorkflowPlan {
            title: format!("deploy {site}.{env}"),
            why: format!("backup-first deploy to {site}.{env}"),
            safety,
            steps,
            stop_on_failure: true,
        },
        step: 0,
    });
    true
}

fn stage_live(state: &mut AppState, site: &str, env: &str) -> bool {
    let updatedb = wants_updatedb(state, site);
    let plan = plan_deploy(
        state.tools.terminus_path(),
        site,
        env,
        DEFAULT_NOTE,
        false,
        updatedb,
    );
    state.current = Some(StagedPlan::One(plan));
    true
}

fn stage_code(state: &mut AppState, site: &str, env: &str) -> bool {
    if state.demo {
        return stage_after_diffstat(state, site, env, vec![]);
    }
    request_diffstat(state, site, env);
    false
}

pub fn request_diffstat(state: &mut AppState, site: &str, env: &str) {
    if state.demo || !state.tools.terminus_ok() {
        if !state.demo && !state.tools.terminus_ok() {
            state.show_toast(ToastLevel::Error, "terminus not on PATH");
        }
        return;
    }
    let plan = plan_diffstat(state.tools.terminus_path(), site, env);
    state.current = Some(StagedPlan::One(plan.clone()));
    start_job(
        state,
        plan,
        JobKind::Diffstat {
            site: site.into(),
            env: env.into(),
        },
    );
}

pub fn apply_diffstat(state: &mut AppState, site: &str, env: &str, json: &str) {
    match parse_diffstat(json) {
        Ok(files) => {
            if files.is_empty() {
                stage_after_diffstat(state, site, env, files);
            } else {
                state.modal = Some(Modal::DiffstatDirty {
                    site: site.to_string(),
                    env: env.to_string(),
                    files,
                });
            }
        }
        Err(err) => state.show_toast(
            ToastLevel::Error,
            format!("env:diffstat parse failed: {err:#}"),
        ),
    }
}

pub fn stage_after_diffstat(
    state: &mut AppState,
    site: &str,
    env: &str,
    files: Vec<String>,
) -> bool {
    if !files.is_empty() {
        match plan_connection_set(state.tools.terminus_path(), site, env, "git", true) {
            Err(block) => {
                state.show_toast(ToastLevel::Warning, block.message);
                state.modal = Some(Modal::DiffstatDirty {
                    site: site.to_string(),
                    env: env.to_string(),
                    files,
                });
                return false;
            }
            Ok(_) => {}
        }
    }
    let sftp = state
        .envs
        .get(site)
        .and_then(|envs| envs.iter().find(|e| e.id == env))
        .is_some_and(|e| e.connection_mode == ConnectionMode::Sftp);
    let mut steps: Vec<CommandPlan> = Vec::new();
    if sftp {
        match plan_connection_set(
            state.tools.terminus_path(),
            site,
            env,
            "git",
            !files.is_empty(),
        ) {
            Ok(plan) => steps.push(plan),
            Err(block) => {
                state.show_toast(ToastLevel::Warning, block.message);
                return false;
            }
        }
    }
    let branch = git_branch(state, site);
    match local_path(state, site) {
        Some(path) if state.tools.git.is_some() || state.demo => {
            steps.push(crate::tools::git::plan_push(
                state.tools.git_path(),
                path,
                "origin",
                &branch,
                Some(site.to_string()),
            ));
            steps.push(plan_wait(state.tools.terminus_path(), site, env));
        }
        Some(_) => {
            state.show_toast(ToastLevel::Warning, "git not on PATH — skip git push");
        }
        None => {
            state.show_toast(
                ToastLevel::Warning,
                "bind a local path or push from another terminal",
            );
        }
    }
    if steps.is_empty() {
        return false;
    }
    let safety = steps
        .iter()
        .map(|s| s.safety)
        .max()
        .unwrap_or(SafetyTier::Mutating);
    if steps.len() == 1 {
        state.current = Some(StagedPlan::One(steps.remove(0)));
    } else {
        state.current = Some(StagedPlan::Workflow {
            plan: WorkflowPlan {
                title: format!("git-mode deploy {site}.{env}"),
                why: format!("push code to {site}.{env} then workflow:wait"),
                safety,
                steps,
                stop_on_failure: true,
            },
            step: 0,
        });
    }
    true
}

pub fn stage_commit_from_diffstat(state: &mut AppState, site: &str, env: &str) {
    let plan = plan_commit(state.tools.terminus_path(), site, env);
    state.current = Some(StagedPlan::One(plan));
}

pub fn on_commit_done(state: &mut AppState, site: &str, env: &str) {
    request_diffstat(state, site, env);
}

pub fn on_connection_set_done(state: &mut AppState, stdout_argv_site: &str, env: &str) {
    if let Some(envs) = state.envs.get_mut(stdout_argv_site) {
        if let Some(e) = envs.iter_mut().find(|e| e.id == env) {
            e.connection_mode = ConnectionMode::Git;
        }
    }
}

fn selected_env(state: &AppState) -> Option<(String, String)> {
    match &state.selected {
        TreeSel::Env { site, env } => Some((site.clone(), env.clone())),
        _ => None,
    }
}

fn wants_updatedb(state: &AppState, site: &str) -> bool {
    state.site(site).is_some_and(|s| {
        s.framework == Framework::Drupal
            || s.overlay
                .as_ref()
                .and_then(|o| o.cms)
                .is_some_and(|c| c == Framework::Drupal)
    })
}

fn git_branch(state: &AppState, site: &str) -> String {
    state
        .site(site)
        .and_then(|s| s.overlay.as_ref()?.git_branch.clone())
        .unwrap_or_else(|| "master".into())
}

fn local_path(state: &AppState, site: &str) -> Option<PathBuf> {
    let site = state.site(site)?;
    site.local
        .as_ref()
        .map(|l| l.path.clone())
        .or_else(|| site.overlay.as_ref().and_then(|o| o.local_path.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_diffstat_blocks_connection_set() {
        let err = plan_connection_set(PathBuf::from("terminus"), "acme-wp", "dev", "git", true)
            .unwrap_err();
        assert!(err.message.contains("diffstat"));
        assert!(
            plan_connection_set(PathBuf::from("terminus"), "acme-wp", "dev", "git", false).is_ok()
        );
    }

    #[test]
    fn parses_diffstat_array_and_empty() {
        let dirty = r#"[{"file":"index.php","status":"M","additions":2,"deletions":1}]"#;
        assert_eq!(parse_diffstat(dirty).unwrap(), vec!["index.php"]);
        assert!(parse_diffstat("").unwrap().is_empty());
        assert!(parse_diffstat("[]").unwrap().is_empty());
    }

    #[test]
    fn parses_diffstat_object_map() {
        let json = r#"{"index.php":{"status":"M"},"src/foo.rs":{"status":"A"}}"#;
        let mut files = parse_diffstat(json).unwrap();
        files.sort();
        assert_eq!(files, vec!["index.php", "src/foo.rs"]);
    }

    #[test]
    fn live_deploy_is_livegate_without_sync_content() {
        let plan = plan_deploy(
            PathBuf::from("terminus"),
            "acme-wp",
            "live",
            DEFAULT_NOTE,
            true,
            false,
        );
        assert_eq!(plan.safety, SafetyTier::LiveGate);
        assert!(!plan.argv.iter().any(|a| a == "--sync-content"));
        assert!(plan.argv.iter().any(|a| a == "--cc"));
        assert!(
            plan.argv
                .iter()
                .any(|a| a == "--note=Deploy from dd_pantheon")
        );
    }

    #[test]
    fn test_deploy_sync_content_is_destructive() {
        let plan = plan_deploy(
            PathBuf::from("terminus"),
            "acme-wp",
            "test",
            DEFAULT_NOTE,
            true,
            true,
        );
        assert_eq!(plan.safety, SafetyTier::Destructive);
        assert!(plan.argv.iter().any(|a| a == "--sync-content"));
        assert!(plan.argv.iter().any(|a| a == "--updatedb"));
    }

    #[test]
    fn wait_passes_max_600() {
        let plan = plan_wait(PathBuf::from("terminus"), "acme-wp", "dev");
        assert_eq!(plan.argv[0], "workflow:wait");
        assert!(plan.argv.iter().any(|a| a == "--max=600"));
        assert_eq!(plan.timeout, Some(WAIT_TIMEOUT));
        assert_eq!(plan.safety, SafetyTier::ReadOnly);
    }
}
