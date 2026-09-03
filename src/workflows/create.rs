use crate::jobs::JobKind;
use crate::models::{OrgRef, UpstreamRef};
use crate::plan::{CommandPlan, PlanTarget, SafetyTier, StagedPlan, ToolKind};
use crate::state::{AppState, AuthState, CreateField, Modal, SiteCreateForm};
use crate::toast::ToastLevel;
use crate::workflows::start_job;
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;

const LIST_TIMEOUT: Duration = Duration::from_secs(60);
const CREATE_TIMEOUT: Duration = Duration::from_secs(600);

pub fn plan_org_catalog(terminus: PathBuf) -> CommandPlan {
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec![
            "org:list".into(),
            "--format=json".into(),
            "--fields=id,name,label".into(),
        ],
        cwd: None,
        why: "list organizations for site:create".into(),
        safety: SafetyTier::ReadOnly,
        target: PlanTarget::None,
        dry_run: false,
        timeout: Some(LIST_TIMEOUT),
        expects_json: true,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: false,
    }
}

pub fn plan_upstream_list(terminus: PathBuf) -> CommandPlan {
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec![
            "upstream:list".into(),
            "--format=json".into(),
            "--fields=id,label,machine_name,framework".into(),
        ],
        cwd: None,
        why: "list upstreams for site:create".into(),
        safety: SafetyTier::ReadOnly,
        target: PlanTarget::None,
        dry_run: false,
        timeout: Some(LIST_TIMEOUT),
        expects_json: true,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: false,
    }
}

pub fn plan_create(
    terminus: PathBuf,
    name: &str,
    label: &str,
    upstream: &str,
    org: &str,
) -> CommandPlan {
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec![
            "site:create".into(),
            name.into(),
            label.into(),
            upstream.into(),
            format!("--org={org}"),
        ],
        cwd: None,
        why: format!("create site {name} in org {org}"),
        safety: SafetyTier::Mutating,
        target: PlanTarget::Site {
            site: name.to_string(),
        },
        dry_run: false,
        timeout: Some(CREATE_TIMEOUT),
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: true,
    }
}

pub fn parse_upstream_list(json: &str) -> anyhow::Result<Vec<UpstreamRef>> {
    if json.trim().is_empty() {
        return Ok(vec![]);
    }
    let value: Value = serde_json::from_str(json)?;
    let mut rows = Vec::new();
    for obj in iter_objects(&value) {
        if let Some(u) = upstream_from_value(obj) {
            rows.push(u);
        }
    }
    rows.sort_by(|a, b| a.label.cmp(&b.label));
    Ok(rows)
}

fn iter_objects(value: &Value) -> Vec<&Value> {
    match value {
        Value::Array(arr) => arr.iter().collect(),
        Value::Object(map) => {
            if map.get("id").is_some() || map.get("machine_name").is_some() {
                vec![value]
            } else {
                map.values().collect()
            }
        }
        _ => vec![],
    }
}

fn upstream_from_value(v: &Value) -> Option<UpstreamRef> {
    let id = json_str(v, "id").or_else(|| json_str(v, "machine_name"))?;
    let machine_name = json_str(v, "machine_name").unwrap_or_default();
    let label = json_str(v, "label")
        .or_else(|| json_str(v, "name"))
        .unwrap_or_else(|| {
            if machine_name.is_empty() {
                id.clone()
            } else {
                machine_name.clone()
            }
        });
    Some(UpstreamRef {
        id,
        label,
        machine_name,
        framework: json_str(v, "framework"),
    })
}

fn json_str(v: &Value, key: &str) -> Option<String> {
    match v.get(key)? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

pub fn valid_site_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    name.len() <= 32
        && first.is_ascii_lowercase()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub fn demo_orgs() -> Vec<OrgRef> {
    vec![OrgRef {
        org_id: "11111111-2222-3333-4444-555555555555".into(),
        org_name: "ldnddev".into(),
    }]
}

pub fn demo_upstreams() -> Vec<UpstreamRef> {
    vec![
        UpstreamRef {
            id: "wordpress".into(),
            label: "WordPress".into(),
            machine_name: "wordpress".into(),
            framework: Some("wordpress".into()),
        },
        UpstreamRef {
            id: "drupal-composer-managed".into(),
            label: "Drupal Composer Managed".into(),
            machine_name: "drupal-composer-managed".into(),
            framework: Some("drupal".into()),
        },
    ]
}

pub fn open(state: &mut AppState) {
    if !state.demo {
        match state.auth {
            AuthState::LoggedOut => {
                state.show_toast(ToastLevel::Warning, "Not logged in — open login from F3");
                return;
            }
            AuthState::Unknown | AuthState::LoggedIn { .. } => {}
        }
        request_lists(state);
    }
    let orgs = if state.demo && state.create_orgs.is_empty() {
        demo_orgs()
    } else {
        state.create_orgs.clone()
    };
    let upstreams = if state.demo && state.create_upstreams.is_empty() {
        demo_upstreams()
    } else {
        state.create_upstreams.clone()
    };
    if state.demo {
        state.create_orgs = orgs.clone();
        state.create_upstreams = upstreams.clone();
    }
    state.modal = Some(Modal::SiteCreate {
        form: SiteCreateForm {
            name: String::new(),
            label: String::new(),
            orgs,
            org_idx: 0,
            upstreams,
            upstream_idx: 0,
            bind_local: false,
            local_path: String::new(),
            focus: CreateField::Name,
        },
    });
}

fn request_lists(state: &mut AppState) {
    if !state.tools.terminus_ok() {
        return;
    }
    if !state.inflight_readonly.contains_key("org:list") {
        let plan = plan_org_catalog(state.tools.terminus_path());
        if let Some(id) = start_job(state, plan, JobKind::CreateOrgList) {
            state.inflight_readonly.insert("org:list".into(), id);
        }
    }
    if !state.inflight_readonly.contains_key("upstream:list") {
        let plan = plan_upstream_list(state.tools.terminus_path());
        if let Some(id) = start_job(state, plan, JobKind::CreateUpstreamList) {
            state.inflight_readonly.insert("upstream:list".into(), id);
        }
    }
}

pub fn apply_org_catalog(state: &mut AppState, json: &str) {
    match crate::workflows::tags::parse_org_list(json) {
        Ok(orgs) => {
            state.create_orgs = orgs.clone();
            if let Some(Modal::SiteCreate { form }) = &mut state.modal {
                form.orgs = orgs;
                if form.org_idx >= form.orgs.len() {
                    form.org_idx = form.orgs.len().saturating_sub(1);
                }
            }
        }
        Err(err) => state.show_toast(ToastLevel::Error, format!("org:list parse failed: {err:#}")),
    }
}

pub fn apply_upstream_list(state: &mut AppState, json: &str) {
    match parse_upstream_list(json) {
        Ok(rows) => {
            state.create_upstreams = rows.clone();
            if let Some(Modal::SiteCreate { form }) = &mut state.modal {
                form.upstreams = rows;
                if form.upstream_idx >= form.upstreams.len() {
                    form.upstream_idx = form.upstreams.len().saturating_sub(1);
                }
            }
        }
        Err(err) => state.show_toast(
            ToastLevel::Error,
            format!("upstream:list parse failed: {err:#}"),
        ),
    }
}

pub fn clear_inflight(state: &mut AppState, kind: &JobKind) {
    match kind {
        JobKind::CreateOrgList => {
            state.inflight_readonly.remove("org:list");
        }
        JobKind::CreateUpstreamList => {
            state.inflight_readonly.remove("upstream:list");
        }
        _ => {}
    }
}

pub fn submit(state: &mut AppState, form: SiteCreateForm) {
    let name = form.name.trim().to_string();
    if !valid_site_name(&name) {
        state.show_toast(
            ToastLevel::Warning,
            "site name: start with a letter, lowercase alnum/dashes, ≤32",
        );
        state.modal = Some(Modal::SiteCreate { form });
        return;
    }
    let label = {
        let t = form.label.trim();
        if t.is_empty() {
            name.clone()
        } else {
            t.to_string()
        }
    };
    let Some(org) = form.orgs.get(form.org_idx) else {
        state.show_toast(
            ToastLevel::Warning,
            "no organizations — cannot create a site",
        );
        state.modal = Some(Modal::SiteCreate { form });
        return;
    };
    let Some(up) = form.upstreams.get(form.upstream_idx) else {
        state.show_toast(ToastLevel::Warning, "no upstreams — cannot create a site");
        state.modal = Some(Modal::SiteCreate { form });
        return;
    };
    if form.bind_local && form.local_path.trim().is_empty() {
        state.show_toast(ToastLevel::Warning, "local path required when bind is on");
        state.modal = Some(Modal::SiteCreate { form });
        return;
    }
    let plan = plan_create(
        state.tools.terminus_path(),
        &name,
        &label,
        up.create_id(),
        &org.org_id,
    );
    if form.bind_local {
        state.pending_create_bind = Some((name.clone(), PathBuf::from(form.local_path.trim())));
    } else {
        state.pending_create_bind = None;
    }
    state.modal = None;
    state.current = Some(StagedPlan::One(plan));
    crate::workflows::request_run(state);
}

pub fn on_created(state: &mut AppState) {
    crate::workflows::inventory::request_site_list(state, true);
    if let Some((name, path)) = state.pending_create_bind.take() {
        state
            .config
            .config
            .locals
            .insert(name.clone(), path.display().to_string());
        state.config.mark_dirty();
        crate::workflows::local::bind_root(state, &path);
        if !state
            .sites
            .iter()
            .any(|s| s.name == name && s.local.is_some())
        {
            state.pending_local = state.pending_local.clone().or_else(|| {
                Some(crate::models::LocalApp {
                    path,
                    lando_name: None,
                    recipe: None,
                    framework: None,
                    terminus_site: Some(name),
                    running: None,
                    url: None,
                })
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_includes_org_flag() {
        let plan = plan_create(
            PathBuf::from("terminus"),
            "acme-new",
            "Acme New",
            "wordpress",
            "org-uuid",
        );
        assert_eq!(plan.argv[0], "site:create");
        assert_eq!(plan.argv[1], "acme-new");
        assert_eq!(plan.argv[2], "Acme New");
        assert_eq!(plan.argv[3], "wordpress");
        assert!(plan.argv.iter().any(|a| a == "--org=org-uuid"));
        assert_eq!(plan.safety, SafetyTier::Mutating);
        assert_eq!(plan.timeout, Some(CREATE_TIMEOUT));
    }

    #[test]
    fn parses_upstream_object_map() {
        let json = r#"{
          "aaaa": {"id":"aaaa","label":"WordPress","machine_name":"wordpress","framework":"wordpress"},
          "bbbb": {"id":"bbbb","name":"Drupal","machine_name":"drupal-composer-managed"}
        }"#;
        let rows = parse_upstream_list(json).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].create_id(), "drupal-composer-managed");
        assert_eq!(rows[1].create_id(), "wordpress");
    }

    #[test]
    fn site_name_rules() {
        assert!(valid_site_name("acme-wp"));
        assert!(valid_site_name("a"));
        assert!(!valid_site_name(""));
        assert!(!valid_site_name("Acme"));
        assert!(!valid_site_name("1acme"));
        assert!(!valid_site_name("acme_wp"));
    }
}
