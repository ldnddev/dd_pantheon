use crate::jobs::JobKind;
use crate::models::{ActionItem, Domain, HttpsRow, LockStatus};
use crate::plan::{CommandPlan, PlanTarget, Redact, SafetyTier, StagedPlan, ToolKind};
use crate::state::{AppState, AuthState, Modal, TreeSel};
use crate::toast::ToastLevel;
use crate::workflows::start_job;
use serde_json::Value;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub const DEBOUNCE: Duration = Duration::from_millis(150);
const MUTATE_TIMEOUT: Duration = Duration::from_secs(600);
const WAKE_TIMEOUT: Duration = Duration::from_secs(60);
const LIST_TIMEOUT: Duration = Duration::from_secs(60);

pub fn plan_clear_cache(terminus: PathBuf, site: &str, env: &str) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec!["env:clear-cache".into(), site_env.clone()],
        cwd: None,
        why: format!("clear caches on {site_env}"),
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

pub fn plan_wake(terminus: PathBuf, site: &str, env: &str) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec!["env:wake".into(), site_env.clone()],
        cwd: None,
        why: format!("ping {site_env} so a sleeping env comes up (ReadOnly)"),
        safety: SafetyTier::ReadOnly,
        target: PlanTarget::Env {
            site: site.to_string(),
            env: env.to_string(),
        },
        dry_run: false,
        timeout: Some(WAKE_TIMEOUT),
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: false,
    }
}

pub fn plan_domain_list(terminus: PathBuf, site: &str, env: &str) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec![
            "domain:list".into(),
            site_env.clone(),
            "--format=json".into(),
            "--fields=id,type,primary,deletable,status".into(),
        ],
        cwd: None,
        why: format!("list domains on {site_env}"),
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

pub fn plan_domain_add(terminus: PathBuf, site: &str, env: &str, domain: &str) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec!["domain:add".into(), site_env.clone(), domain.into()],
        cwd: None,
        why: format!("add {domain} to {site_env}"),
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

pub fn plan_domain_remove(terminus: PathBuf, site: &str, env: &str, domain: &str) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec!["domain:remove".into(), site_env.clone(), domain.into()],
        cwd: None,
        why: format!("remove {domain} from {site_env}"),
        safety: SafetyTier::Destructive,
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

pub fn plan_https_info(terminus: PathBuf, site: &str, env: &str) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec![
            "https:info".into(),
            site_env.clone(),
            "--format=json".into(),
            "--fields=id,type,status,status_message,deletable".into(),
        ],
        cwd: None,
        why: format!("HTTPS status for {site_env}"),
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

pub fn plan_https_set(
    terminus: PathBuf,
    site: &str,
    env: &str,
    cert: &str,
    key: &str,
    intermediate: Option<&str>,
) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    let mut argv = vec![
        "https:set".into(),
        site_env.clone(),
        cert.into(),
        key.into(),
    ];
    if let Some(int) = intermediate.filter(|s| !s.is_empty()) {
        argv.push(format!("--intermediate-certificate={int}"));
    }
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv,
        cwd: None,
        why: format!("set HTTPS cert/key paths on {site_env} (paths, not file contents)"),
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

pub fn plan_lock_info(terminus: PathBuf, site: &str, env: &str) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec![
            "lock:info".into(),
            site_env.clone(),
            "--format=json".into(),
            "--fields=locked,username".into(),
        ],
        cwd: None,
        why: format!("lock status for {site_env} (password field omitted)"),
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

pub fn plan_lock_enable(
    terminus: PathBuf,
    site: &str,
    env: &str,
    username: &str,
    password: &str,
) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec![
            "lock:enable".into(),
            site_env.clone(),
            username.into(),
            password.into(),
        ],
        cwd: None,
        why: format!("enable HTTP basic auth on {site_env}"),
        safety: SafetyTier::Mutating,
        target: PlanTarget::Env {
            site: site.to_string(),
            env: env.to_string(),
        },
        dry_run: false,
        timeout: Some(MUTATE_TIMEOUT),
        expects_json: false,
        extra_env: vec![],
        redact: vec![Redact::Exact(password.to_string())],
        confirm_with_yes: true,
    }
}

pub fn plan_lock_disable(terminus: PathBuf, site: &str, env: &str) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec!["lock:disable".into(), site_env.clone()],
        cwd: None,
        why: format!("disable HTTP basic auth on {site_env}"),
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

pub fn parse_domain_list(json: &str) -> anyhow::Result<Vec<Domain>> {
    if json.trim().is_empty() {
        return Ok(vec![]);
    }
    let value: Value = serde_json::from_str(json)?;
    let mut rows = Vec::new();
    for obj in iter_objects(&value) {
        if let Some(d) = domain_from_value(obj) {
            rows.push(d);
        }
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(rows)
}

pub fn parse_https_info(json: &str) -> anyhow::Result<Vec<HttpsRow>> {
    if json.trim().is_empty() {
        return Ok(vec![]);
    }
    let value: Value = serde_json::from_str(json)?;
    let mut rows = Vec::new();
    for obj in iter_objects(&value) {
        if let Some(h) = https_from_value(obj) {
            rows.push(h);
        }
    }
    Ok(rows)
}

pub fn parse_lock_info(json: &str) -> anyhow::Result<LockStatus> {
    if json.trim().is_empty() {
        return Ok(LockStatus {
            locked: false,
            username: None,
        });
    }
    let value: Value = serde_json::from_str(json)?;
    let obj = match &value {
        Value::Object(map) => {
            if map.get("locked").is_some() || map.get("username").is_some() {
                &value
            } else {
                map.values().next().unwrap_or(&value)
            }
        }
        _ => &value,
    };
    Ok(LockStatus {
        locked: json_bool(obj, "locked"),
        username: json_str(obj, "username"),
    })
}

fn iter_objects(value: &Value) -> Vec<&Value> {
    match value {
        Value::Array(arr) => arr.iter().collect(),
        Value::Object(map) => {
            if map.get("id").is_some() || map.get("domain").is_some() {
                vec![value]
            } else {
                map.values().collect()
            }
        }
        _ => vec![],
    }
}

fn domain_from_value(v: &Value) -> Option<Domain> {
    let id = json_str(v, "id")
        .or_else(|| json_str(v, "domain"))
        .unwrap_or_default();
    if id.is_empty() {
        return None;
    }
    Some(Domain {
        id,
        kind: json_str(v, "type").unwrap_or_default(),
        primary: json_bool(v, "primary"),
        deletable: json_bool(v, "deletable"),
        status: json_str(v, "status").unwrap_or_default(),
    })
}

fn https_from_value(v: &Value) -> Option<HttpsRow> {
    let id = json_str(v, "id").unwrap_or_default();
    if id.is_empty() && json_str(v, "status").is_none() {
        return None;
    }
    Some(HttpsRow {
        id,
        kind: json_str(v, "type").unwrap_or_default(),
        status: json_str(v, "status").unwrap_or_default(),
        status_message: json_str(v, "status_message").unwrap_or_default(),
    })
}

fn json_str(v: &Value, key: &str) -> Option<String> {
    match v.get(key)? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn json_bool(v: &Value, key: &str) -> bool {
    match v.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => matches!(s.to_ascii_lowercase().as_str(), "true" | "1" | "yes"),
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0) != 0,
        _ => false,
    }
}

fn cache_key(site: &str, env: &str) -> String {
    format!("{site}.{env}")
}

fn selected_env(state: &AppState) -> Option<(String, String)> {
    match &state.selected {
        TreeSel::Env { site, env } => Some((site.clone(), env.clone())),
        _ => None,
    }
}

pub fn stage_clear_cache(state: &mut AppState) -> bool {
    let Some((site, env)) = selected_env(state) else {
        state.show_toast(ToastLevel::Warning, "select an environment");
        return false;
    };
    let plan = plan_clear_cache(state.tools.terminus_path(), &site, &env);
    state.current = Some(StagedPlan::One(plan));
    true
}

pub fn stage_wake(state: &mut AppState) -> bool {
    let Some((site, env)) = selected_env(state) else {
        state.show_toast(ToastLevel::Warning, "select an environment");
        return false;
    };
    let plan = plan_wake(state.tools.terminus_path(), &site, &env);
    state.current = Some(StagedPlan::One(plan));
    true
}

pub fn actions(state: &AppState) -> Vec<ActionItem> {
    if !matches!(state.selected, TreeSel::Env { .. }) {
        return vec![];
    }
    vec![
        ActionItem {
            id: "domain-add",
            label: "add domain",
        },
        ActionItem {
            id: "domain-remove",
            label: "remove domain",
        },
        ActionItem {
            id: "https-set",
            label: "set HTTPS",
        },
        ActionItem {
            id: "lock-enable",
            label: "lock enable",
        },
        ActionItem {
            id: "lock-disable",
            label: "lock disable",
        },
    ]
}

pub fn open_domain_add(state: &mut AppState) -> bool {
    let Some((site, env)) = selected_env(state) else {
        state.show_toast(ToastLevel::Warning, "select an environment");
        return false;
    };
    state.modal = Some(Modal::DomainAdd {
        site,
        env,
        value: String::new(),
    });
    false
}

pub fn submit_domain_add(state: &mut AppState, site: String, env: String, value: String) {
    let domain = value.trim().to_string();
    if domain.is_empty() || !domain.contains('.') {
        state.show_toast(ToastLevel::Warning, "enter a domain like example.com");
        state.modal = Some(Modal::DomainAdd { site, env, value });
        return;
    }
    state.modal = None;
    let plan = plan_domain_add(state.tools.terminus_path(), &site, &env, &domain);
    state.current = Some(StagedPlan::One(plan));
    crate::workflows::request_run(state);
}

pub fn open_domain_remove(state: &mut AppState) -> bool {
    let Some((site, env)) = selected_env(state) else {
        state.show_toast(ToastLevel::Warning, "select an environment");
        return false;
    };
    let key = cache_key(&site, &env);
    let domains: Vec<String> = state
        .domains
        .get(&key)
        .map(|rows| {
            rows.iter()
                .filter(|d| d.deletable || d.kind == "custom")
                .map(|d| d.id.clone())
                .collect()
        })
        .unwrap_or_default();
    if domains.is_empty() {
        state.show_toast(ToastLevel::Warning, "no removable domains loaded");
        return false;
    }
    state.modal = Some(Modal::DomainRemove {
        site,
        env,
        domains,
        selected: 0,
    });
    false
}

pub fn submit_domain_remove(state: &mut AppState, site: String, env: String, domain: String) {
    state.modal = None;
    let plan = plan_domain_remove(state.tools.terminus_path(), &site, &env, &domain);
    state.current = Some(StagedPlan::One(plan));
    crate::workflows::request_run(state);
}

pub fn open_https_set(state: &mut AppState) -> bool {
    let Some((site, env)) = selected_env(state) else {
        state.show_toast(ToastLevel::Warning, "select an environment");
        return false;
    };
    state.modal = Some(Modal::HttpsSet {
        site,
        env,
        cert: String::new(),
        key: String::new(),
        intermediate: String::new(),
        focus: 0,
    });
    false
}

pub fn submit_https_set(
    state: &mut AppState,
    site: String,
    env: String,
    cert: String,
    key: String,
    intermediate: String,
) {
    let cert = cert.trim().to_string();
    let key = key.trim().to_string();
    if cert.is_empty() || key.is_empty() {
        state.show_toast(ToastLevel::Warning, "cert and key paths are required");
        state.modal = Some(Modal::HttpsSet {
            site,
            env,
            cert,
            key,
            intermediate,
            focus: 0,
        });
        return;
    }
    state.modal = None;
    let int = intermediate.trim();
    let plan = plan_https_set(
        state.tools.terminus_path(),
        &site,
        &env,
        &cert,
        &key,
        if int.is_empty() { None } else { Some(int) },
    );
    state.current = Some(StagedPlan::One(plan));
    crate::workflows::request_run(state);
}

pub fn open_lock_enable(state: &mut AppState) -> bool {
    let Some((site, env)) = selected_env(state) else {
        state.show_toast(ToastLevel::Warning, "select an environment");
        return false;
    };
    state.modal = Some(Modal::LockEnable {
        site,
        env,
        username: String::new(),
        password: String::new(),
        focus: 0,
    });
    false
}

pub fn submit_lock_enable(
    state: &mut AppState,
    site: String,
    env: String,
    username: String,
    password: String,
) {
    if username.trim().is_empty() || password.is_empty() {
        state.show_toast(ToastLevel::Warning, "username and password required");
        state.modal = Some(Modal::LockEnable {
            site,
            env,
            username,
            password,
            focus: 0,
        });
        return;
    }
    state.modal = None;
    let plan = plan_lock_enable(
        state.tools.terminus_path(),
        &site,
        &env,
        username.trim(),
        &password,
    );
    state.current = Some(StagedPlan::One(plan));
    crate::workflows::request_run(state);
}

pub fn stage_lock_disable(state: &mut AppState) -> bool {
    let Some((site, env)) = selected_env(state) else {
        state.show_toast(ToastLevel::Warning, "select an environment");
        return false;
    };
    let plan = plan_lock_disable(state.tools.terminus_path(), &site, &env);
    state.current = Some(StagedPlan::One(plan));
    true
}

pub fn on_selection_changed(state: &mut AppState) {
    if state.demo {
        return;
    }
    match &state.selected {
        TreeSel::Env { site, env } => {
            state.pending_edge = Some((site.clone(), env.clone(), Instant::now()));
        }
        _ => state.pending_edge = None,
    }
}

pub fn flush_debounce(state: &mut AppState) {
    if state.demo {
        return;
    }
    if let Some((site, env, at)) = state.pending_edge.clone() {
        if at.elapsed() >= DEBOUNCE {
            state.pending_edge = None;
            request(state, &site, &env, false);
        }
    }
}

pub fn refresh_selected(state: &mut AppState) {
    if let TreeSel::Env { site, env } = state.selected.clone() {
        request(state, &site, &env, true);
    }
}

fn request(state: &mut AppState, site: &str, env: &str, force: bool) {
    if state.demo || !state.tools.terminus_ok() {
        return;
    }
    if !matches!(state.auth, AuthState::LoggedIn { .. }) {
        return;
    }
    start_quiet(
        state,
        plan_domain_list(state.tools.terminus_path(), site, env),
        JobKind::DomainList {
            site: site.into(),
            env: env.into(),
        },
        format!("domain:list:{site}.{env}"),
        force,
    );
    start_quiet(
        state,
        plan_https_info(state.tools.terminus_path(), site, env),
        JobKind::HttpsInfo {
            site: site.into(),
            env: env.into(),
        },
        format!("https:info:{site}.{env}"),
        force,
    );
    start_quiet(
        state,
        plan_lock_info(state.tools.terminus_path(), site, env),
        JobKind::LockInfo {
            site: site.into(),
            env: env.into(),
        },
        format!("lock:info:{site}.{env}"),
        force,
    );
}

fn start_quiet(state: &mut AppState, plan: CommandPlan, kind: JobKind, key: String, force: bool) {
    if !force && state.inflight_readonly.contains_key(&key) {
        return;
    }
    if let Some(id) = start_job(state, plan, kind) {
        state.inflight_readonly.insert(key, id);
    }
}

pub fn apply_domains(state: &mut AppState, site: &str, env: &str, json: &str) {
    match parse_domain_list(json) {
        Ok(rows) => {
            state.domains.insert(cache_key(site, env), rows);
        }
        Err(err) => state.show_toast(
            ToastLevel::Error,
            format!("domain:list parse failed: {err:#}"),
        ),
    }
}

pub fn apply_https(state: &mut AppState, site: &str, env: &str, json: &str) {
    match parse_https_info(json) {
        Ok(rows) => {
            state.https.insert(cache_key(site, env), rows);
        }
        Err(err) => state.show_toast(
            ToastLevel::Error,
            format!("https:info parse failed: {err:#}"),
        ),
    }
}

pub fn apply_lock(state: &mut AppState, site: &str, env: &str, json: &str) {
    match parse_lock_info(json) {
        Ok(info) => {
            if let Some(envs) = state.envs.get_mut(site) {
                if let Some(e) = envs.iter_mut().find(|e| e.id == env) {
                    e.locked = info.locked;
                }
            }
            state.locks.insert(cache_key(site, env), info);
        }
        Err(err) => state.show_toast(
            ToastLevel::Error,
            format!("lock:info parse failed: {err:#}"),
        ),
    }
}

pub fn clear_inflight(state: &mut AppState, kind: &JobKind) {
    match kind {
        JobKind::DomainList { site, env } => {
            state
                .inflight_readonly
                .remove(&format!("domain:list:{site}.{env}"));
        }
        JobKind::HttpsInfo { site, env } => {
            state
                .inflight_readonly
                .remove(&format!("https:info:{site}.{env}"));
        }
        JobKind::LockInfo { site, env } => {
            state
                .inflight_readonly
                .remove(&format!("lock:info:{site}.{env}"));
        }
        _ => {}
    }
}

pub fn on_edge_mutate(state: &mut AppState, site: &str, env: &str) {
    request(state, site, env, true);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_is_mutating_wake_is_readonly() {
        let cache = plan_clear_cache(PathBuf::from("terminus"), "acme-wp", "test");
        assert_eq!(cache.argv, vec!["env:clear-cache", "acme-wp.test"]);
        assert_eq!(cache.safety, SafetyTier::Mutating);
        let wake = plan_wake(PathBuf::from("terminus"), "acme-wp", "dev");
        assert_eq!(wake.safety, SafetyTier::ReadOnly);
    }

    #[test]
    fn domain_remove_is_destructive() {
        let plan = plan_domain_remove(PathBuf::from("terminus"), "acme-wp", "live", "example.com");
        assert_eq!(plan.argv[0], "domain:remove");
        assert_eq!(plan.safety, SafetyTier::Destructive);
    }

    #[test]
    fn lock_enable_redacts_password() {
        let plan = plan_lock_enable(
            PathBuf::from("terminus"),
            "acme-wp",
            "test",
            "ops",
            "s3cret-pass",
        );
        let line = plan.redacted_shell_line();
        assert!(!line.contains("s3cret-pass"), "{line}");
        assert!(line.contains("***"));
        assert!(plan.argv.iter().any(|a| a == "s3cret-pass"));
    }

    #[test]
    fn lock_info_drops_password_field() {
        let json = r#"{"locked":true,"username":"ops","password":"s3cret-pass"}"#;
        let info = parse_lock_info(json).unwrap();
        assert!(info.locked);
        assert_eq!(info.username.as_deref(), Some("ops"));
    }

    #[test]
    fn parses_domain_object_map() {
        let json = r#"{
          "example.com": {"id":"example.com","type":"custom","primary":false,"deletable":true,"status":"ok"},
          "www.example.com": {"id":"www.example.com","type":"custom","deletable":true}
        }"#;
        let rows = parse_domain_list(json).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|d| d.id == "example.com" && d.deletable));
    }

    #[test]
    fn https_set_keeps_paths_not_contents() {
        let plan = plan_https_set(
            PathBuf::from("terminus"),
            "acme-wp",
            "live",
            "/etc/ssl/cert.pem",
            "/etc/ssl/key.pem",
            Some("/etc/ssl/chain.pem"),
        );
        assert_eq!(plan.argv[2], "/etc/ssl/cert.pem");
        assert!(
            plan.argv
                .iter()
                .any(|a| a == "--intermediate-certificate=/etc/ssl/chain.pem")
        );
        assert_eq!(plan.safety, SafetyTier::Mutating);
    }
}
