use crate::jobs::JobKind;
use crate::models::{ActionItem, Backup};
use crate::plan::{CommandPlan, PlanTarget, SafetyTier, StagedPlan, ToolKind, WorkflowPlan};
use crate::state::{AppState, AuthState, BackupPickKind, Modal, TreeSel};
use crate::toast::ToastLevel;
use crate::workflows::start_job;
use serde_json::Value;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub const DEBOUNCE: Duration = Duration::from_millis(150);
const LIST_TIMEOUT: Duration = Duration::from_secs(60);
const MUTATE_TIMEOUT: Duration = Duration::from_secs(600);
const LIST_FIELDS: &str = "file,size,date,expiry,initiator";

pub fn plan_create(terminus: PathBuf, site: &str, env: &str, element: &str) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec![
            "backup:create".into(),
            site_env.clone(),
            format!("--element={element}"),
        ],
        cwd: None,
        why: format!("backup {site_env}"),
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

pub fn plan_list(terminus: PathBuf, site: &str, env: &str) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec![
            "backup:list".into(),
            site_env.clone(),
            "--format=json".into(),
            format!("--fields={LIST_FIELDS}"),
        ],
        cwd: None,
        why: format!("list backups for {site_env}"),
        safety: SafetyTier::ReadOnly,
        target: PlanTarget::Env {
            site: site.to_string(),
            env: env.to_string(),
        },
        dry_run: false,
        timeout: Some(LIST_TIMEOUT),
        expects_json: true,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: false,
    }
}

pub fn plan_get(terminus: PathBuf, site: &str, env: &str, file: Option<&str>) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    let mut argv = vec!["backup:get".into(), site_env.clone()];
    if let Some(file) = file {
        argv.push(format!("--file={file}"));
    }
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv,
        cwd: None,
        why: format!("show download URL for {site_env} (not saved to disk)"),
        safety: SafetyTier::ReadOnly,
        target: PlanTarget::Env {
            site: site.to_string(),
            env: env.to_string(),
        },
        dry_run: false,
        timeout: Some(LIST_TIMEOUT),
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: false,
    }
}

pub fn plan_restore(terminus: PathBuf, site: &str, env: &str, file: Option<&str>) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    let mut argv = vec!["backup:restore".into(), site_env.clone()];
    if let Some(file) = file {
        argv.push(format!("--file={file}"));
    }
    let safety = if env == "live" {
        SafetyTier::LiveGate
    } else {
        SafetyTier::Destructive
    };
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv,
        cwd: None,
        why: format!("restore backup onto {site_env} (overwrites the env)"),
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

pub fn restore_workflow(
    terminus: PathBuf,
    site: &str,
    env: &str,
    file: Option<&str>,
) -> WorkflowPlan {
    let target = PlanTarget::Env {
        site: site.to_string(),
        env: env.to_string(),
    };
    let mut steps = crate::safety::backup_first(terminus.clone(), &target);
    steps.push(plan_restore(terminus, site, env, file));
    let safety = steps
        .iter()
        .map(|s| s.safety)
        .max()
        .unwrap_or(SafetyTier::Destructive);
    WorkflowPlan {
        title: format!("restore {site}.{env}"),
        why: format!("backup-first restore onto {site}.{env}"),
        safety,
        steps,
        stop_on_failure: true,
    }
}

pub fn parse_backup_list(json: &str) -> anyhow::Result<Vec<Backup>> {
    if json.trim().is_empty() {
        return Ok(vec![]);
    }
    let value: Value = serde_json::from_str(json)?;
    let mut rows = backups_from_value(&value);
    rows.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| b.file.cmp(&a.file)));
    Ok(rows)
}

fn backups_from_value(value: &Value) -> Vec<Backup> {
    match value {
        Value::Array(arr) => arr.iter().filter_map(backup_from_value).collect(),
        Value::Object(map) => {
            if map.get("file").is_some() || map.get("filename").is_some() {
                return backup_from_value(value).into_iter().collect();
            }
            let mut rows = Vec::new();
            for (k, v) in map {
                if let Some(mut b) = backup_from_value(v) {
                    if b.file.is_empty() {
                        b.file = k.clone();
                    }
                    rows.push(b);
                }
            }
            rows
        }
        _ => vec![],
    }
}

fn backup_from_value(v: &Value) -> Option<Backup> {
    let obj = v.as_object()?;
    let file = json_str(obj.get("file"))
        .or_else(|| json_str(obj.get("filename")))
        .unwrap_or_default();
    let date = json_str(obj.get("date")).unwrap_or_default();
    if file.is_empty() && date.is_empty() {
        return None;
    }
    Some(Backup {
        file,
        size: json_size(obj.get("size")),
        date,
        expiry: json_str(obj.get("expiry")).unwrap_or_else(|| "—".into()),
        initiator: json_str(obj.get("initiator")).unwrap_or_default(),
    })
}

fn json_str(v: Option<&Value>) -> Option<String> {
    match v {
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

fn json_size(v: Option<&Value>) -> String {
    match v {
        Some(Value::Number(n)) => human_size(n.as_u64().unwrap_or(0)),
        Some(Value::String(s)) if !s.is_empty() => s.clone(),
        _ => "—".into(),
    }
}

fn human_size(bytes: u64) -> String {
    const K: f64 = 1024.0;
    let b = bytes as f64;
    if bytes < 1024 {
        format!("{bytes}B")
    } else if b < K * K {
        format!("{:.0}K", b / K)
    } else if b < K * K * K {
        format!("{:.1}M", b / (K * K))
    } else {
        format!("{:.1}G", b / (K * K * K))
    }
}

fn cache_key(site: &str, env: &str) -> String {
    format!("{site}.{env}")
}

fn inflight_key(site: &str, env: &str) -> String {
    format!("backup:list:{site}.{env}")
}

pub fn on_selection_changed(state: &mut AppState) {
    if state.demo {
        return;
    }
    match &state.selected {
        TreeSel::Env { site, env } => {
            state.pending_backup_list = Some((site.clone(), env.clone(), Instant::now()));
        }
        _ => {
            state.pending_backup_list = None;
        }
    }
}

pub fn flush_debounce(state: &mut AppState) {
    if state.demo {
        return;
    }
    if let Some((site, env, at)) = state.pending_backup_list.clone() {
        if at.elapsed() >= DEBOUNCE {
            state.pending_backup_list = None;
            request(state, &site, &env, false);
        }
    }
}

pub fn request(state: &mut AppState, site: &str, env: &str, force: bool) {
    if state.demo || !state.tools.terminus_ok() {
        return;
    }
    if !matches!(state.auth, AuthState::LoggedIn { .. }) {
        return;
    }
    let ikey = inflight_key(site, env);
    if !force && state.inflight_readonly.contains_key(&ikey) {
        return;
    }
    if !force && state.backups.contains_key(&cache_key(site, env)) {
        return;
    }
    let plan = plan_list(state.tools.terminus_path(), site, env);
    state.current = Some(StagedPlan::One(plan.clone()));
    if let Some(id) = start_job(
        state,
        plan,
        JobKind::BackupList {
            site: site.into(),
            env: env.into(),
        },
    ) {
        state.inflight_readonly.insert(ikey, id);
    }
}

pub fn apply_list(state: &mut AppState, site: &str, env: &str, json: &str) {
    match parse_backup_list(json) {
        Ok(rows) => {
            state.backups.insert(cache_key(site, env), rows);
        }
        Err(err) => state.show_toast(
            ToastLevel::Error,
            format!("backup:list parse failed: {err:#}"),
        ),
    }
}

pub fn clear_inflight(state: &mut AppState, kind: &JobKind) {
    if let JobKind::BackupList { site, env } = kind {
        state.inflight_readonly.remove(&inflight_key(site, env));
    }
}

pub fn refresh_selected(state: &mut AppState) {
    if let TreeSel::Env { site, env } = state.selected.clone() {
        request(state, &site, &env, true);
    }
}

pub fn actions(state: &AppState) -> Vec<ActionItem> {
    if !matches!(state.selected, TreeSel::Env { .. }) {
        return vec![];
    }
    if state.selected_backups().is_empty() {
        return vec![];
    }
    vec![
        ActionItem {
            id: "backup-restore",
            label: "Restore backup…",
        },
        ActionItem {
            id: "backup-get",
            label: "backup URL",
        },
    ]
}

pub fn open_restore(state: &mut AppState) -> bool {
    open_pick(state, BackupPickKind::Restore)
}

pub fn open_get(state: &mut AppState) -> bool {
    open_pick(state, BackupPickKind::Get)
}

fn open_pick(state: &mut AppState, kind: BackupPickKind) -> bool {
    let Some((site, env)) = selected_env(state) else {
        state.show_toast(ToastLevel::Warning, "select an environment");
        return false;
    };
    let files: Vec<String> = state
        .selected_backups()
        .iter()
        .map(|b| b.file.clone())
        .collect();
    if files.is_empty() {
        state.show_toast(ToastLevel::Warning, "no backups loaded");
        return false;
    }
    state.modal = Some(Modal::BackupPick {
        site,
        env,
        files,
        selected: 0,
        kind,
    });
    false
}

pub fn submit_pick(
    state: &mut AppState,
    site: String,
    env: String,
    file: String,
    kind: BackupPickKind,
) {
    state.modal = None;
    match kind {
        BackupPickKind::Restore => {
            let wf = restore_workflow(
                state.tools.terminus_path(),
                &site,
                &env,
                Some(file.as_str()),
            );
            state.current = Some(StagedPlan::Workflow { plan: wf, step: 0 });
            crate::workflows::request_run(state);
        }
        BackupPickKind::Get => {
            let plan = plan_get(
                state.tools.terminus_path(),
                &site,
                &env,
                Some(file.as_str()),
            );
            state.current = Some(StagedPlan::One(plan));
            crate::workflows::request_run(state);
        }
    }
}

pub fn stage_create(state: &mut AppState) -> bool {
    let Some((site, env)) = selected_env(state) else {
        state.show_toast(ToastLevel::Warning, "select an environment");
        return false;
    };
    let plan = plan_create(state.tools.terminus_path(), &site, &env, "all");
    state.current = Some(StagedPlan::One(plan));
    true
}

fn selected_env(state: &AppState) -> Option<(String, String)> {
    match &state.selected {
        TreeSel::Env { site, env } => Some((site.clone(), env.clone())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_object_map() {
        let json = r#"{
          "backup_20260831_acme-wp_test_all": {
            "file": "backup_20260831_acme-wp_test_all.tgz",
            "size": 50593792,
            "date": "2026-08-31 12:01:03",
            "expiry": "2027-08-31",
            "initiator": "manual"
          },
          "older": {
            "file": "backup_20260801.tgz",
            "size": "12M",
            "date": "2026-08-01 09:00:00",
            "expiry": "2027-08-01"
          }
        }"#;
        let rows = parse_backup_list(json).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].file, "backup_20260831_acme-wp_test_all.tgz");
        assert_eq!(rows[0].size, "48.2M");
        assert_eq!(rows[1].file, "backup_20260801.tgz");
        assert_eq!(rows[1].size, "12M");
    }

    #[test]
    fn parses_array_and_empty() {
        let json = r#"[{"filename":"a.tgz","size":512,"date":"2026-08-02"}]"#;
        let rows = parse_backup_list(json).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].file, "a.tgz");
        assert_eq!(rows[0].size, "512B");
        assert!(parse_backup_list("").unwrap().is_empty());
    }

    #[test]
    fn create_is_mutating_with_element_all() {
        let plan = plan_create(PathBuf::from("terminus"), "acme-wp", "live", "all");
        assert_eq!(plan.argv[0], "backup:create");
        assert_eq!(plan.argv[1], "acme-wp.live");
        assert!(plan.argv.iter().any(|a| a == "--element=all"));
        assert_eq!(plan.safety, SafetyTier::Mutating);
        assert_eq!(plan.timeout, Some(MUTATE_TIMEOUT));
        assert!(plan.confirm_with_yes);
    }

    #[test]
    fn get_does_not_write_to_cwd() {
        let plan = plan_get(
            PathBuf::from("terminus"),
            "acme-wp",
            "test",
            Some("backup.tgz"),
        );
        assert_eq!(plan.argv[0], "backup:get");
        assert!(!plan.argv.iter().any(|a| a.starts_with("--to")));
        assert_eq!(plan.safety, SafetyTier::ReadOnly);
    }

    #[test]
    fn restore_is_backup_first_and_livegate_on_live() {
        let wf = restore_workflow(PathBuf::from("terminus"), "acme-wp", "live", None);
        assert_eq!(wf.steps[0].argv[0], "backup:create");
        assert_eq!(wf.steps[1].argv[0], "backup:list");
        assert_eq!(wf.steps[2].argv[0], "backup:restore");
        assert_eq!(wf.safety, SafetyTier::LiveGate);
        assert_eq!(wf.steps[2].safety, SafetyTier::LiveGate);
        let test = plan_restore(PathBuf::from("terminus"), "acme-wp", "test", Some("a.tgz"));
        assert_eq!(test.safety, SafetyTier::Destructive);
        assert!(test.argv.iter().any(|a| a == "--file=a.tgz"));
    }
}
