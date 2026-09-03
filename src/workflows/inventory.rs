use crate::jobs::JobKind;
use crate::models::{ConnectionMode, Env, Framework, Site};
use crate::plan::{CommandPlan, PlanTarget, SafetyTier, StagedPlan, ToolKind};
use crate::state::{AppState, AuthState, TreeSel};
use crate::workflows::start_job;
use serde_json::Value;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub const DEBOUNCE: Duration = Duration::from_millis(150);

const SITE_FIELDS: &str = "name,id,label,plan_name,framework,region,owner,created,memberships,frozen,upstream,upstream_label";
const ENV_LIST_FIELDS: &str =
    "id,created,domain,connection_mode,locked,initialized,php_version,php_runtime_generation";
const ENV_INFO_FIELDS: &str = "id,created,domain,locked,initialized,connection_mode,php_version,drush_version,php_runtime_generation";

pub fn plan_site_list(terminus: PathBuf) -> CommandPlan {
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec![
            "site:list".into(),
            "--format=json".into(),
            format!("--fields={SITE_FIELDS}"),
        ],
        cwd: None,
        why: "inventory refresh".into(),
        safety: SafetyTier::ReadOnly,
        target: PlanTarget::None,
        dry_run: false,
        timeout: Some(Duration::from_secs(60)),
        expects_json: true,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: false,
    }
}

pub fn plan_env_list(terminus: PathBuf, site: &str) -> CommandPlan {
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec![
            "env:list".into(),
            site.into(),
            "--format=json".into(),
            format!("--fields={ENV_LIST_FIELDS}"),
        ],
        cwd: None,
        why: format!("list environments for {site}"),
        safety: SafetyTier::ReadOnly,
        target: PlanTarget::Site {
            site: site.to_string(),
        },
        dry_run: false,
        timeout: Some(Duration::from_secs(60)),
        expects_json: true,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: false,
    }
}

pub fn plan_env_info(terminus: PathBuf, site: &str, env: &str) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec![
            "env:info".into(),
            site_env.clone(),
            "--format=json".into(),
            format!("--fields={ENV_INFO_FIELDS}"),
        ],
        cwd: None,
        why: format!("env info for {site_env}"),
        safety: SafetyTier::ReadOnly,
        target: PlanTarget::Env {
            site: site.to_string(),
            env: env.to_string(),
        },
        dry_run: false,
        timeout: Some(Duration::from_secs(60)),
        expects_json: true,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: false,
    }
}

pub fn parse_site_list(json: &str) -> anyhow::Result<Vec<Site>> {
    if json.trim().is_empty() {
        return Ok(vec![]);
    }
    let value: Value = serde_json::from_str(json)?;
    let mut sites = Vec::new();
    for obj in iter_objects(&value) {
        if let Some(site) = site_from_value(obj) {
            sites.push(site);
        }
    }
    sites.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(sites)
}

pub fn parse_env_list(json: &str, site: &str) -> anyhow::Result<Vec<Env>> {
    if json.trim().is_empty() {
        return Ok(vec![]);
    }
    let value: Value = serde_json::from_str(json)?;
    let mut envs = Vec::new();
    for obj in iter_objects(&value) {
        if let Some(env) = env_from_value(obj, site) {
            envs.push(env);
        }
    }
    envs.sort_by(|a, b| env_order(&a.id).cmp(&env_order(&b.id)));
    Ok(envs)
}

pub fn parse_env_info(json: &str, site: &str) -> anyhow::Result<Option<Env>> {
    if json.trim().is_empty() {
        return Ok(None);
    }
    let value: Value = serde_json::from_str(json)?;
    let obj = match &value {
        Value::Object(_) => &value,
        _ => return Ok(None),
    };
    Ok(env_from_value(obj, site))
}

fn env_order(id: &str) -> (u8, String) {
    let rank = match id {
        "dev" => 0,
        "test" => 1,
        "live" => 2,
        _ => 3,
    };
    (rank, id.to_string())
}

fn iter_objects(value: &Value) -> Vec<&Value> {
    match value {
        Value::Object(map) => {
            let is_record = matches!(map.get("id"), Some(Value::String(_)))
                || matches!(map.get("name"), Some(Value::String(_)));
            if is_record {
                vec![value]
            } else {
                map.values().collect()
            }
        }
        Value::Array(arr) => arr.iter().collect(),
        _ => vec![],
    }
}

fn site_from_value(v: &Value) -> Option<Site> {
    let name = json_str(v, "name")?;
    if name.is_empty() {
        return None;
    }
    Some(Site {
        name: name.clone(),
        id: json_str(v, "id").unwrap_or(name),
        label: json_str(v, "label"),
        framework: json_str(v, "framework")
            .map(|s| Framework::from_terminus(&s))
            .unwrap_or(Framework::Other),
        region: json_str(v, "region"),
        frozen: json_bool(v, "frozen"),
        plan_name: json_str(v, "plan_name"),
        owner: json_str(v, "owner"),
        upstream: json_str(v, "upstream"),
        upstream_label: json_str(v, "upstream_label"),
        memberships: json_str(v, "memberships"),
        tags: vec![],
        orgs: vec![],
        local: None,
        overlay: None,
    })
}

fn env_from_value(v: &Value, site: &str) -> Option<Env> {
    let id = json_str(v, "id")?;
    Some(Env {
        id,
        site: site.to_string(),
        domain: json_str(v, "domain"),
        connection_mode: json_str(v, "connection_mode")
            .map(|s| ConnectionMode::from_terminus(&s))
            .unwrap_or(ConnectionMode::Unknown),
        locked: json_bool(v, "locked"),
        initialized: json_bool(v, "initialized"),
        php_version: json_str(v, "php_version"),
        php_runtime_generation: json_str(v, "php_runtime_generation"),
        drush_version: json_str(v, "drush_version"),
        created: json_str(v, "created"),
    })
}

fn json_str(v: &Value, key: &str) -> Option<String> {
    match v.get(key)? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn json_bool(v: &Value, key: &str) -> bool {
    match v.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0) != 0,
        Some(Value::String(s)) => matches!(s.to_ascii_lowercase().as_str(), "true" | "1" | "yes"),
        _ => false,
    }
}

fn inflight_key(kind: &JobKind) -> Option<String> {
    match kind {
        JobKind::SiteList => Some("site:list".into()),
        JobKind::EnvList { site } => Some(format!("env:list:{site}")),
        JobKind::EnvInfo { site, env } => Some(format!("env:info:{site}.{env}")),
        JobKind::OrgList { site } => Some(format!("org:list:{site}")),
        JobKind::TagList { site, org } => Some(format!("tag:list:{site}:{org}")),
        _ => None,
    }
}

pub fn request_site_list(state: &mut AppState, force: bool) {
    if state.demo || !state.tools.terminus_ok() {
        return;
    }
    if !matches!(state.auth, AuthState::LoggedIn { .. }) {
        return;
    }
    let key = "site:list".to_string();
    if !force && state.inflight_readonly.contains_key(&key) {
        return;
    }
    let plan = plan_site_list(state.tools.terminus_path());
    state.current = Some(StagedPlan::One(plan.clone()));
    if let Some(id) = start_job(state, plan, JobKind::SiteList) {
        state.inflight_readonly.insert(key, id);
    }
}

pub fn request_env_list(state: &mut AppState, site: &str, force: bool) {
    if state.demo || !state.tools.terminus_ok() {
        return;
    }
    if !force && state.envs.get(site).is_some_and(|e| !e.is_empty()) {
        return;
    }
    let key = format!("env:list:{site}");
    if !force && state.inflight_readonly.contains_key(&key) {
        return;
    }
    let plan = plan_env_list(state.tools.terminus_path(), site);
    state.current = Some(StagedPlan::One(plan.clone()));
    if let Some(id) = start_job(state, plan, JobKind::EnvList { site: site.into() }) {
        state.inflight_readonly.insert(key, id);
    }
}

pub fn request_env_info(state: &mut AppState, site: &str, env: &str, force: bool) {
    if state.demo || !state.tools.terminus_ok() {
        return;
    }
    let key = format!("env:info:{site}.{env}");
    if !force && state.inflight_readonly.contains_key(&key) {
        return;
    }
    let plan = plan_env_info(state.tools.terminus_path(), site, env);
    state.current = Some(StagedPlan::One(plan.clone()));
    if let Some(id) = start_job(
        state,
        plan,
        JobKind::EnvInfo {
            site: site.into(),
            env: env.into(),
        },
    ) {
        state.inflight_readonly.insert(key, id);
    }
}

pub fn on_selection_changed(state: &mut AppState) {
    if state.demo {
        return;
    }
    match state.selected.clone() {
        TreeSel::Site(site) => {
            state.pending_env_list = Some((site, Instant::now()));
            state.pending_env_info = None;
        }
        TreeSel::Env { site, env } => {
            if state.envs.get(&site).map(|e| e.is_empty()).unwrap_or(true) {
                state.pending_env_list = Some((site.clone(), Instant::now()));
            }
            state.pending_env_info = Some((site, env, Instant::now()));
        }
        TreeSel::None => {
            state.pending_env_list = None;
            state.pending_env_info = None;
        }
    }
}

pub fn on_expand(state: &mut AppState, site: &str) {
    if state.demo {
        return;
    }
    state.pending_env_list = Some((site.to_string(), Instant::now()));
}

pub fn flush_debounce(state: &mut AppState) {
    if state.demo {
        return;
    }
    if let Some((site, at)) = state.pending_env_list.clone() {
        if at.elapsed() >= DEBOUNCE {
            state.pending_env_list = None;
            request_env_list(state, &site, false);
        }
    }
    if let Some((site, env, at)) = state.pending_env_info.clone() {
        if at.elapsed() >= DEBOUNCE {
            state.pending_env_info = None;
            request_env_info(state, &site, &env, false);
        }
    }
}

pub fn refresh(state: &mut AppState) {
    if state.demo {
        return;
    }
    request_site_list(state, true);
    match state.selected.clone() {
        TreeSel::Site(site) => request_env_list(state, &site, true),
        TreeSel::Env { site, env } => {
            request_env_list(state, &site, true);
            request_env_info(state, &site, &env, true);
        }
        TreeSel::None => {}
    }
    crate::workflows::tags::refresh_selected(state);
    crate::workflows::metrics::refresh_selected(state);
    crate::workflows::backup::refresh_selected(state);
    crate::workflows::local::refresh_selected(state);
    crate::workflows::domains::refresh_selected(state);
}

pub fn apply_site_list(state: &mut AppState, json: &str) {
    match parse_site_list(json) {
        Ok(sites) => {
            let old = crate::workflows::local::snapshot_locals(state);
            state.sites = sites;
            crate::workflows::local::reattach(state, old);
            restore_selection(state);
            state.rebuild_tree();
            state.select_matching_row();
        }
        Err(err) => state.show_toast(
            crate::toast::ToastLevel::Error,
            format!("site:list parse failed: {err:#}"),
        ),
    }
}

pub fn apply_env_list(state: &mut AppState, site: &str, json: &str) {
    match parse_env_list(json, site) {
        Ok(envs) => {
            state.envs.insert(site.to_string(), envs);
            state.rebuild_tree();
            state.select_matching_row();
        }
        Err(err) => state.show_toast(
            crate::toast::ToastLevel::Error,
            format!("env:list parse failed: {err:#}"),
        ),
    }
}

pub fn apply_env_info(state: &mut AppState, site: &str, env_id: &str, json: &str) {
    match parse_env_info(json, site) {
        Ok(Some(info)) => {
            let list = state.envs.entry(site.to_string()).or_default();
            if let Some(existing) = list.iter_mut().find(|e| e.id == env_id) {
                *existing = info;
            } else {
                list.push(info);
                list.sort_by(|a, b| env_order(&a.id).cmp(&env_order(&b.id)));
            }
        }
        Ok(None) => {}
        Err(err) => state.show_toast(
            crate::toast::ToastLevel::Error,
            format!("env:info parse failed: {err:#}"),
        ),
    }
}

pub fn clear_inflight(state: &mut AppState, kind: &JobKind) {
    if let Some(key) = inflight_key(kind) {
        state.inflight_readonly.remove(&key);
    }
}

fn restore_selection(state: &mut AppState) {
    let last_site = state.config.config.last_site.clone();
    let last_env = state.config.config.last_env.clone();
    if let Some(site) = last_site {
        if state.sites.iter().any(|s| s.name == site) {
            if let Some(env) = last_env {
                state.expanded.insert(site.clone());
                state.selected = TreeSel::Env { site, env };
            } else {
                state.selected = TreeSel::Site(site);
            }
            return;
        }
    }
    if let Some(first) = state.sites.first() {
        state.selected = TreeSel::Site(first.name.clone());
    } else {
        state.selected = TreeSel::None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SITES: &str = r#"{
      "acme-wp": {
        "name": "acme-wp",
        "id": "aaa",
        "label": "Acme WP",
        "plan_name": "Gold",
        "framework": "wordpress",
        "region": "us-central1",
        "owner": "user",
        "created": "2020-01-01",
        "memberships": "team",
        "frozen": false,
        "upstream": "up-id",
        "upstream_label": "WordPress"
      },
      "frozen-lab": {
        "name": "frozen-lab",
        "id": "bbb",
        "framework": "drupal8",
        "frozen": true
      }
    }"#;

    const ENVS: &str = r#"{
      "live": {"id":"live","domain":"live-acme-wp.pantheonsite.io","connection_mode":"git","locked":true,"initialized":true,"php_version":"8.3"},
      "dev": {"id":"dev","domain":"dev-acme-wp.pantheonsite.io","connection_mode":"sftp","locked":false,"initialized":true,"php_version":"8.2"}
    }"#;

    const INFO: &str = r#"{
      "id": "dev",
      "created": "2020-01-01",
      "domain": "dev-acme-wp.pantheonsite.io",
      "locked": false,
      "initialized": true,
      "connection_mode": "git",
      "php_version": "8.3",
      "drush_version": "12",
      "php_runtime_generation": "8.3"
    }"#;

    #[test]
    fn parses_site_object_map() {
        let sites = parse_site_list(SITES).unwrap();
        assert_eq!(sites.len(), 2);
        assert_eq!(sites[0].name, "acme-wp");
        assert_eq!(sites[0].framework, Framework::WordPress);
        assert_eq!(sites[0].upstream_label.as_deref(), Some("WordPress"));
        assert!(sites[1].frozen);
        assert_eq!(sites[1].framework, Framework::Drupal);
    }

    #[test]
    fn parses_site_array() {
        let json = r#"[{"name":"z-site","id":"1","framework":"wordpress"}]"#;
        let sites = parse_site_list(json).unwrap();
        assert_eq!(sites[0].name, "z-site");
    }

    #[test]
    fn parses_env_list_sorted() {
        let envs = parse_env_list(ENVS, "acme-wp").unwrap();
        assert_eq!(envs[0].id, "dev");
        assert_eq!(envs[1].id, "live");
        assert_eq!(envs[0].connection_mode, ConnectionMode::Sftp);
        assert!(envs[1].locked);
    }

    #[test]
    fn parses_env_info() {
        let env = parse_env_info(INFO, "acme-wp").unwrap().unwrap();
        assert_eq!(env.php_version.as_deref(), Some("8.3"));
        assert_eq!(env.drush_version.as_deref(), Some("12"));
        assert_eq!(env.connection_mode, ConnectionMode::Git);
    }

    #[test]
    fn empty_json_is_empty_list() {
        assert!(parse_site_list("").unwrap().is_empty());
        assert!(parse_env_list("  ", "x").unwrap().is_empty());
    }
}
