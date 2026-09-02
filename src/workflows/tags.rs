use crate::jobs::JobKind;
use crate::models::{OrgRef, Tag};
use crate::plan::{CommandPlan, PlanTarget, SafetyTier, StagedPlan, ToolKind};
use crate::state::{AppState, Modal, TreeSel};
use crate::toast::ToastLevel;
use crate::workflows::start_job;
use serde_json::Value;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub const DEBOUNCE: Duration = Duration::from_millis(150);

pub fn plan_org_list(terminus: PathBuf, site: &str) -> CommandPlan {
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec![
            "site:org:list".into(),
            site.into(),
            "--format=json".into(),
            "--fields=org_name,org_id".into(),
        ],
        cwd: None,
        why: format!("list organizations for {site}"),
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

pub fn plan_tag_list(terminus: PathBuf, site: &str, org: &str) -> CommandPlan {
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec![
            "tag:list".into(),
            site.into(),
            org.into(),
            "--format=json".into(),
        ],
        cwd: None,
        why: format!("list tags for {site} in org {org}"),
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

pub fn plan_tag_add(terminus: PathBuf, site: &str, org: &str, tag: &str) -> CommandPlan {
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec!["tag:add".into(), site.into(), org.into(), tag.into()],
        cwd: None,
        why: format!("add tag `{tag}` to {site}"),
        safety: SafetyTier::Mutating,
        target: PlanTarget::Site {
            site: site.to_string(),
        },
        dry_run: false,
        timeout: Some(Duration::from_secs(60)),
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: true,
    }
}

pub fn plan_tag_remove(terminus: PathBuf, site: &str, org: &str, tag: &str) -> CommandPlan {
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec!["tag:remove".into(), site.into(), org.into(), tag.into()],
        cwd: None,
        why: format!("remove tag `{tag}` from {site}"),
        safety: SafetyTier::Mutating,
        target: PlanTarget::Site {
            site: site.to_string(),
        },
        dry_run: false,
        timeout: Some(Duration::from_secs(60)),
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: true,
    }
}

pub fn parse_org_list(json: &str) -> anyhow::Result<Vec<OrgRef>> {
    if json.trim().is_empty() {
        return Ok(vec![]);
    }
    let value: Value = serde_json::from_str(json)?;
    let mut orgs = Vec::new();
    for obj in iter_objects(&value) {
        if let Some(org) = org_from_value(obj) {
            orgs.push(org);
        }
    }
    orgs.sort_by(|a, b| a.org_name.cmp(&b.org_name));
    Ok(orgs)
}

pub fn parse_tag_list(json: &str, org: &str) -> anyhow::Result<Vec<Tag>> {
    if json.trim().is_empty() {
        return Ok(vec![]);
    }
    let value: Value = serde_json::from_str(json)?;
    let names = tag_names(&value);
    Ok(names
        .into_iter()
        .map(|name| Tag {
            name,
            org: org.to_string(),
        })
        .collect())
}

fn iter_objects(value: &Value) -> Vec<&Value> {
    match value {
        Value::Object(map) => {
            let is_record = map.get("org_id").is_some() || map.get("org_name").is_some();
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

fn org_from_value(v: &Value) -> Option<OrgRef> {
    let org_id = json_str(v, "org_id").or_else(|| json_str(v, "id"))?;
    let org_name = json_str(v, "org_name")
        .or_else(|| json_str(v, "name"))
        .unwrap_or_else(|| org_id.clone());
    Some(OrgRef { org_id, org_name })
}

fn tag_names(value: &Value) -> Vec<String> {
    match value {
        Value::Array(arr) => arr
            .iter()
            .filter_map(|v| match v {
                Value::String(s) if !s.is_empty() => Some(s.clone()),
                Value::Object(map) => map
                    .get("name")
                    .and_then(|n| n.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .or_else(|| {
                        map.get("tag")
                            .and_then(|n| n.as_str())
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string())
                    }),
                _ => None,
            })
            .collect(),
        Value::Object(map) => {
            if let Some(name) = map.get("name").and_then(|n| n.as_str()) {
                vec![name.to_string()]
            } else {
                map.keys().cloned().collect()
            }
        }
        Value::String(s) if !s.is_empty() => vec![s.clone()],
        _ => vec![],
    }
}

fn json_str(v: &Value, key: &str) -> Option<String> {
    match v.get(key)? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

fn inflight_key(kind: &JobKind) -> Option<String> {
    match kind {
        JobKind::OrgList { site } => Some(format!("org:list:{site}")),
        JobKind::TagList { site, org } => Some(format!("tag:list:{site}:{org}")),
        _ => None,
    }
}

pub fn selected_site_name(state: &AppState) -> Option<String> {
    match &state.selected {
        TreeSel::Site(s) | TreeSel::Env { site: s, .. } => Some(s.clone()),
        TreeSel::None => None,
    }
}

pub fn resolved_org(state: &AppState, site: &str) -> Option<OrgRef> {
    let site_rec = state.site(site)?;
    if site_rec.orgs.is_empty() {
        return None;
    }
    if site_rec.orgs.len() == 1 {
        return site_rec.orgs.first().cloned();
    }
    if let Some(id) = state.config.config.orgs.get(site) {
        if let Some(org) = site_rec.orgs.iter().find(|o| &o.org_id == id) {
            return Some(org.clone());
        }
    }
    None
}

pub fn on_selection_changed(state: &mut AppState) {
    if state.demo {
        return;
    }
    if let Some(site) = selected_site_name(state) {
        state.pending_org_list = Some((site, Instant::now()));
    } else {
        state.pending_org_list = None;
        state.pending_tag_list = None;
    }
}

pub fn flush_debounce(state: &mut AppState) {
    if state.demo {
        return;
    }
    if let Some((site, at)) = state.pending_org_list.clone() {
        if at.elapsed() >= DEBOUNCE {
            state.pending_org_list = None;
            request_org_list(state, &site, false);
        }
    }
    if let Some((site, org, at)) = state.pending_tag_list.clone() {
        if at.elapsed() >= DEBOUNCE {
            state.pending_tag_list = None;
            request_tag_list(state, &site, &org, false);
        }
    }
}

pub fn request_org_list(state: &mut AppState, site: &str, force: bool) {
    if state.demo || !state.tools.terminus_ok() {
        return;
    }
    if !force {
        if let Some(org) = resolved_org(state, site) {
            request_tag_list(state, site, &org.org_id, false);
            return;
        }
        if state.org_prompted.contains(site) {
            return;
        }
    }
    let key = format!("org:list:{site}");
    if !force && state.inflight_readonly.contains_key(&key) {
        return;
    }
    let plan = plan_org_list(state.tools.terminus_path(), site);
    state.current = Some(StagedPlan::One(plan.clone()));
    if let Some(id) = start_job(state, plan, JobKind::OrgList { site: site.into() }) {
        state.inflight_readonly.insert(key, id);
    }
}

pub fn request_tag_list(state: &mut AppState, site: &str, org: &str, force: bool) {
    if state.demo || !state.tools.terminus_ok() {
        return;
    }
    let key = format!("tag:list:{site}:{org}");
    if !force && state.inflight_readonly.contains_key(&key) {
        return;
    }
    let plan = plan_tag_list(state.tools.terminus_path(), site, org);
    state.current = Some(StagedPlan::One(plan.clone()));
    if let Some(id) = start_job(
        state,
        plan,
        JobKind::TagList {
            site: site.into(),
            org: org.into(),
        },
    ) {
        state.inflight_readonly.insert(key, id);
    }
}

pub fn apply_org_list(state: &mut AppState, site: &str, json: &str) {
    let orgs = match parse_org_list(json) {
        Ok(o) => o,
        Err(err) => {
            state.show_toast(
                ToastLevel::Error,
                format!("site:org:list parse failed: {err:#}"),
            );
            return;
        }
    };
    if let Some(rec) = state.sites.iter_mut().find(|s| s.name == site) {
        rec.orgs = orgs.clone();
    }
    match orgs.len() {
        0 => {
            state.org_prompted.insert(site.to_string());
        }
        1 => {
            let org = &orgs[0];
            state
                .config
                .config
                .orgs
                .insert(site.to_string(), org.org_id.clone());
            state.config.mark_dirty();
            request_tag_list(state, site, &org.org_id, true);
        }
        _ => {
            if let Some(org) = resolved_org(state, site) {
                request_tag_list(state, site, &org.org_id, true);
            } else if !state.org_prompted.contains(site) {
                state.org_prompted.insert(site.to_string());
                state.modal = Some(Modal::OrgPicker {
                    site: site.to_string(),
                    orgs,
                    selected: 0,
                });
            }
        }
    }
}

pub fn apply_tag_list(state: &mut AppState, site: &str, org: &str, json: &str) {
    match parse_tag_list(json, org) {
        Ok(tags) => {
            if let Some(rec) = state.sites.iter_mut().find(|s| s.name == site) {
                rec.tags = tags;
            }
            state.rebuild_tree();
        }
        Err(err) => state.show_toast(ToastLevel::Error, format!("tag:list parse failed: {err:#}")),
    }
}

pub fn clear_inflight(state: &mut AppState, kind: &JobKind) {
    if let Some(key) = inflight_key(kind) {
        state.inflight_readonly.remove(&key);
    }
}

pub fn pick_org(state: &mut AppState, site: &str, org: OrgRef) {
    state
        .config
        .config
        .orgs
        .insert(site.to_string(), org.org_id.clone());
    state.config.mark_dirty();
    request_tag_list(state, site, &org.org_id, true);
}

pub fn open_add(state: &mut AppState) {
    let Some(site) = selected_site_name(state) else {
        state.show_toast(ToastLevel::Warning, "select a site");
        return;
    };
    if resolved_org(state, &site).is_none() {
        state.show_toast(ToastLevel::Warning, "tags require an organization");
        return;
    }
    state.modal = Some(Modal::TagAdd {
        value: String::new(),
    });
}

pub fn submit_add(state: &mut AppState, tag: String) {
    let tag = tag.trim().to_string();
    if tag.is_empty() {
        state.show_toast(ToastLevel::Warning, "tag is empty");
        return;
    }
    let Some(site) = selected_site_name(state) else {
        return;
    };
    let Some(org) = resolved_org(state, &site) else {
        state.show_toast(ToastLevel::Warning, "tags require an organization");
        return;
    };
    let plan = plan_tag_add(state.tools.terminus_path(), &site, &org.org_id, &tag);
    state.current = Some(StagedPlan::One(plan.clone()));
    crate::workflows::request_run(state);
}

pub fn stage_remove(state: &mut AppState, tag: &str) {
    let Some(site) = selected_site_name(state) else {
        return;
    };
    let Some(org) = resolved_org(state, &site) else {
        state.show_toast(ToastLevel::Warning, "tags require an organization");
        return;
    };
    let plan = plan_tag_remove(state.tools.terminus_path(), &site, &org.org_id, tag);
    state.current = Some(StagedPlan::One(plan.clone()));
    crate::workflows::request_run(state);
}

pub fn open_tag_picker(state: &mut AppState) {
    if state.tag_filter.is_some() {
        state.tag_filter = None;
        state.rebuild_tree();
        state.select_matching_row();
        return;
    }
    let mut tags: Vec<String> = state
        .sites
        .iter()
        .flat_map(|s| s.tags.iter().map(|t| t.name.clone()))
        .collect();
    tags.sort();
    tags.dedup();
    if tags.is_empty() {
        state.show_toast(ToastLevel::Info, "no tags loaded");
        return;
    }
    state.modal = Some(Modal::TagPicker { tags, selected: 0 });
}

pub fn pin_filter(state: &mut AppState, tag: String) {
    state.tag_filter = Some(tag);
    state.rebuild_tree();
    state.select_matching_row();
}

pub fn cycle_chip(state: &mut AppState, delta: isize) {
    let Some(site) = state.selected_site() else {
        return;
    };
    if site.tags.is_empty() {
        return;
    }
    let names: Vec<String> = site.tags.iter().map(|t| t.name.clone()).collect();
    let cur = state
        .selected_chip
        .as_ref()
        .and_then(|c| names.iter().position(|n| n == c))
        .unwrap_or(0);
    let len = names.len() as isize;
    let next = (cur as isize + delta).rem_euclid(len) as usize;
    state.selected_chip = Some(names[next].clone());
}

pub fn refresh_selected(state: &mut AppState) {
    if let Some(site) = selected_site_name(state) {
        request_org_list(state, &site, true);
    }
}

pub fn on_tag_mutate_done(state: &mut AppState) {
    if let Some(site) = selected_site_name(state) {
        if let Some(org) = resolved_org(state, &site) {
            request_tag_list(state, &site, &org.org_id, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_org_object_map() {
        let json = r#"{
          "aaa-uuid": {"org_name":"ldnddev","org_id":"aaa-uuid"},
          "bbb-uuid": {"org_name":"clients","org_id":"bbb-uuid"}
        }"#;
        let orgs = parse_org_list(json).unwrap();
        assert_eq!(orgs.len(), 2);
        assert_eq!(orgs[0].org_name, "clients");
        assert_eq!(orgs[1].org_id, "aaa-uuid");
    }

    #[test]
    fn parses_org_array() {
        let json = r#"[{"org_name":"Acme","org_id":"1"}]"#;
        let orgs = parse_org_list(json).unwrap();
        assert_eq!(orgs[0].org_name, "Acme");
    }

    #[test]
    fn parses_tag_string_array() {
        let tags = parse_tag_list(r#"["prod","client-acme"]"#, "org").unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "prod");
        assert_eq!(tags[0].org, "org");
    }

    #[test]
    fn parses_tag_object_array() {
        let tags = parse_tag_list(r#"[{"name":"prod"},{"tag":"beta"}]"#, "o").unwrap();
        assert_eq!(tags[0].name, "prod");
        assert_eq!(tags[1].name, "beta");
    }

    #[test]
    fn parses_tag_object_keys() {
        let tags = parse_tag_list(r#"{"prod":{},"qa":{}}"#, "o").unwrap();
        assert!(tags.iter().any(|t| t.name == "prod"));
        assert!(tags.iter().any(|t| t.name == "qa"));
    }

    #[test]
    fn empty_json_is_empty() {
        assert!(parse_org_list("").unwrap().is_empty());
        assert!(parse_tag_list("  ", "o").unwrap().is_empty());
    }
}
