use crate::models::Framework;
use crate::plan::{CommandPlan, PlanTarget, SafetyTier, ToolKind};
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const PANTHEON_RECIPE_EXTRAS: &[&str] = &[
    "pull",
    "push",
    "terminus",
    "drush",
    "wp",
    "composer",
    "mysql",
    "db-import",
    "db-export",
];

pub fn plan(
    binary: PathBuf,
    argv: Vec<String>,
    why: impl Into<String>,
    safety_tier: SafetyTier,
    target: PlanTarget,
) -> CommandPlan {
    let cwd = match &target {
        PlanTarget::Local { path, .. } => Some(path.clone()),
        _ => None,
    };
    let confirm = safety_tier >= SafetyTier::Mutating;
    CommandPlan {
        tool: ToolKind::Lando,
        binary,
        argv,
        cwd,
        why: why.into(),
        safety: safety_tier,
        target,
        dry_run: false,
        timeout: None,
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: confirm,
    }
}

#[derive(Clone, Debug, Default)]
pub struct LandoPeek {
    pub name: Option<String>,
    pub recipe: Option<String>,
    pub framework: Option<Framework>,
    pub site: Option<String>,
}

impl LandoPeek {
    pub fn is_pantheon(&self) -> bool {
        self.recipe
            .as_deref()
            .is_some_and(|r| r.eq_ignore_ascii_case("pantheon"))
    }
}

#[derive(Debug, Deserialize)]
struct LandoYml {
    name: Option<String>,
    recipe: Option<String>,
    config: Option<LandoYmlConfig>,
}

#[derive(Debug, Deserialize)]
struct LandoYmlConfig {
    framework: Option<String>,
    site: Option<String>,
}

pub fn peek_lando_yml(dir: &Path) -> anyhow::Result<LandoPeek> {
    let file = if dir.ends_with(".lando.yml") {
        dir.to_path_buf()
    } else {
        dir.join(".lando.yml")
    };
    let raw =
        fs::read_to_string(&file).map_err(|err| anyhow::anyhow!("{}: {err}", file.display()))?;
    let parsed: LandoYml = serde_yaml::from_str(&raw)?;
    let recipe = parsed.recipe;
    let (framework, site) = match parsed.config {
        Some(c) => (c.framework.as_deref().map(Framework::from_terminus), c.site),
        None => (None, None),
    };
    Ok(LandoPeek {
        name: parsed.name,
        recipe,
        framework,
        site,
    })
}

#[derive(Clone, Debug)]
pub struct LandoAppStatus {
    pub name: String,
    pub running: bool,
    pub src: Option<PathBuf>,
}

pub fn parse_list(json: &str) -> anyhow::Result<Vec<LandoAppStatus>> {
    if json.trim().is_empty() {
        return Ok(vec![]);
    }
    let value: Value = serde_json::from_str(json)?;
    Ok(list_from_value(&value))
}

fn list_from_value(value: &Value) -> Vec<LandoAppStatus> {
    match value {
        Value::Array(arr) => merge_status(arr.iter().filter_map(status_from_value)),
        Value::Object(map) => {
            let mut out = Vec::new();
            for (k, v) in map {
                match v {
                    Value::Array(arr) => {
                        let mut group = merge_status(arr.iter().filter_map(status_from_value));
                        for g in &mut group {
                            if g.name.is_empty() {
                                g.name = k.clone();
                            }
                        }
                        if group.is_empty() {
                            out.push(LandoAppStatus {
                                name: k.clone(),
                                running: arr.iter().any(value_running),
                                src: arr.iter().find_map(value_src),
                            });
                        } else {
                            out.extend(group);
                        }
                    }
                    Value::Object(_) => {
                        if let Some(mut s) = status_from_value(v) {
                            if s.name.is_empty() {
                                s.name = k.clone();
                            }
                            out.push(s);
                        }
                    }
                    Value::Bool(b) => out.push(LandoAppStatus {
                        name: k.clone(),
                        running: *b,
                        src: None,
                    }),
                    _ => {}
                }
            }
            merge_status(out.into_iter())
        }
        _ => vec![],
    }
}

fn merge_status(iter: impl IntoIterator<Item = LandoAppStatus>) -> Vec<LandoAppStatus> {
    let mut by_name: Vec<LandoAppStatus> = Vec::new();
    for row in iter {
        if let Some(existing) = by_name.iter_mut().find(|e| e.name == row.name) {
            existing.running |= row.running;
            if existing.src.is_none() {
                existing.src = row.src;
            }
        } else {
            by_name.push(row);
        }
    }
    by_name
}

fn status_from_value(v: &Value) -> Option<LandoAppStatus> {
    let obj = v.as_object()?;
    let name = json_str(obj.get("app"))
        .or_else(|| json_str(obj.get("name")))
        .unwrap_or_default();
    Some(LandoAppStatus {
        name,
        running: value_running(v),
        src: value_src(v),
    })
}

fn value_running(v: &Value) -> bool {
    match v {
        Value::Object(obj) => match obj.get("running") {
            Some(Value::Bool(b)) => *b,
            _ => obj
                .get("status")
                .and_then(|s| s.as_str())
                .is_some_and(|s| s.to_ascii_lowercase().contains("up")),
        },
        _ => false,
    }
}

fn value_src(v: &Value) -> Option<PathBuf> {
    let obj = v.as_object()?;
    json_str(obj.get("src"))
        .or_else(|| json_str(obj.get("dir")))
        .or_else(|| json_str(obj.get("path")))
        .or_else(|| json_str(obj.get("destination")))
        .map(PathBuf::from)
}

pub fn parse_info_url(json: &str) -> anyhow::Result<Option<String>> {
    if json.trim().is_empty() {
        return Ok(None);
    }
    let value: Value = serde_json::from_str(json)?;
    Ok(first_url(&value))
}

fn first_url(value: &Value) -> Option<String> {
    let mut http = None;
    collect_urls(value, &mut http);
    http
}

fn collect_urls(value: &Value, found: &mut Option<String>) {
    if found.as_ref().is_some_and(|u| u.starts_with("https://")) {
        return;
    }
    match value {
        Value::String(s) if s.starts_with("http://") || s.starts_with("https://") => {
            prefer_url(found, s.clone());
        }
        Value::Array(arr) => {
            for v in arr {
                collect_urls(v, found);
            }
        }
        Value::Object(map) => {
            for key in ["urls", "url", "uri"] {
                if let Some(v) = map.get(key) {
                    collect_urls(v, found);
                }
            }
            for (k, v) in map {
                if k == "urls" || k == "url" || k == "uri" {
                    continue;
                }
                collect_urls(v, found);
            }
        }
        _ => {}
    }
}

fn prefer_url(slot: &mut Option<String>, url: String) {
    match slot {
        Some(existing) if existing.starts_with("https://") => {}
        _ if url.starts_with("https://") => *slot = Some(url),
        None => *slot = Some(url),
        _ => {}
    }
}

fn json_str(v: Option<&Value>) -> Option<String> {
    match v {
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

pub fn with_timeout(mut plan: CommandPlan, timeout: Option<Duration>) -> CommandPlan {
    plan.timeout = timeout;
    plan
}

pub fn with_json(mut plan: CommandPlan) -> CommandPlan {
    plan.expects_json = true;
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peeks_pantheon_recipe() {
        let dir =
            std::env::temp_dir().join(format!("dd_pantheon_lando_yml_{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        fs::write(
            dir.join(".lando.yml"),
            "name: acmewp\nrecipe: pantheon\nconfig:\n  framework: wordpress\n  site: acme-wp\n",
        )
        .unwrap();
        let peek = peek_lando_yml(&dir).unwrap();
        assert_eq!(peek.name.as_deref(), Some("acmewp"));
        assert!(peek.is_pantheon());
        assert_eq!(peek.framework, Some(Framework::WordPress));
        assert_eq!(peek.site.as_deref(), Some("acme-wp"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn parses_list_object_and_array() {
        let json = r#"{
          "acmewp": [
            {"name":"appserver","app":"acmewp","running":true,"src":"/tmp/acme-wp"},
            {"name":"database","app":"acmewp","running":true}
          ]
        }"#;
        let rows = parse_list(json).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "acmewp");
        assert!(rows[0].running);
        assert_eq!(rows[0].src, Some(PathBuf::from("/tmp/acme-wp")));

        let arr = r#"[{"app":"other","running":false,"dir":"/tmp/other"}]"#;
        let rows = parse_list(arr).unwrap();
        assert_eq!(rows[0].name, "other");
        assert!(!rows[0].running);
    }

    #[test]
    fn parses_info_prefers_https() {
        let json = r#"[{"service":"appserver","urls":["http://localhost:8080","https://acme-wp.lndo.site"]}]"#;
        assert_eq!(
            parse_info_url(json).unwrap().as_deref(),
            Some("https://acme-wp.lndo.site")
        );
        assert!(parse_info_url("").unwrap().is_none());
    }
}
