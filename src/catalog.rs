use crate::plan::{SafetyTier, ToolKind};
use crate::safety::{hint_from_name, is_hidden_name};
use crate::tools::lando::PANTHEON_RECIPE_EXTRAS;
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatalogKind {
    Workflow,
    Palette,
    Hidden,
}

#[derive(Clone, Debug)]
pub struct CatalogArg {
    pub name: String,
    pub required: bool,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct CatalogOpt {
    pub name: String,
    pub shortcut: Option<String>,
    pub accept_value: bool,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct CatalogEntry {
    pub tool: ToolKind,
    pub name: String,
    pub description: String,
    pub kind: CatalogKind,
    pub safety_hint: SafetyTier,
    pub arguments: Vec<CatalogArg>,
    pub options: Vec<CatalogOpt>,
}

const SKIP_OPTS: &[&str] = &[
    "help",
    "--help",
    "quiet",
    "--quiet",
    "verbose",
    "--verbose",
    "version",
    "--version",
    "ansi",
    "--ansi",
    "no-ansi",
    "--no-ansi",
    "no-interaction",
    "--no-interaction",
    "define",
    "--define",
    "yes",
    "--yes",
    "-y",
    "-n",
    "-h",
    "-q",
    "-v",
    "-V",
];

const WORKFLOW_NAMES: &[&str] = &[
    "auth:login",
    "auth:logout",
    "auth:whoami",
    "backup:create",
    "backup:list",
    "backup:get",
    "backup:restore",
    "connection:info",
    "connection:set",
    "env:clear-cache",
    "env:clone-content",
    "env:commit",
    "env:deploy",
    "env:diffstat",
    "env:info",
    "env:list",
    "env:metrics",
    "env:wake",
    "env:wipe",
    "lock:enable",
    "lock:disable",
    "lock:info",
    "multidev:create",
    "multidev:delete",
    "multidev:list",
    "multidev:merge-to-dev",
    "multidev:merge-from-dev",
    "site:create",
    "site:info",
    "site:list",
    "site:org:list",
    "tag:add",
    "tag:list",
    "tag:remove",
    "workflow:wait",
    "workflow:list",
    "domain:add",
    "domain:list",
    "domain:remove",
    "https:info",
    "https:set",
    "remote:wp",
    "remote:drush",
];

#[derive(Debug, Deserialize)]
struct TerminusList {
    #[serde(default)]
    commands: Vec<TerminusCommand>,
}

#[derive(Debug, Deserialize)]
struct TerminusCommand {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    definition: Option<TerminusDefinition>,
}

#[derive(Debug, Deserialize, Default)]
struct TerminusDefinition {
    #[serde(default)]
    arguments: Value,
    #[serde(default)]
    options: Value,
}

pub fn parse_terminus_list(json: &str) -> anyhow::Result<Vec<CatalogEntry>> {
    let parsed: TerminusList = serde_json::from_str(json)?;
    let mut out = Vec::with_capacity(parsed.commands.len());
    for cmd in parsed.commands {
        let def = cmd.definition.unwrap_or_default();
        let arguments = parse_arguments(&def.arguments);
        let options = parse_options(&def.options);
        let kind = kind_for(&cmd.name, cmd.hidden);
        out.push(CatalogEntry {
            tool: ToolKind::Terminus,
            safety_hint: hint_from_name(&cmd.name),
            name: cmd.name,
            description: cmd.description,
            kind,
            arguments,
            options,
        });
    }
    Ok(out)
}

fn kind_for(name: &str, hidden_flag: bool) -> CatalogKind {
    if hidden_flag || is_hidden_name(name) {
        CatalogKind::Hidden
    } else if WORKFLOW_NAMES.contains(&name) {
        CatalogKind::Workflow
    } else {
        CatalogKind::Palette
    }
}

fn parse_arguments(value: &Value) -> Vec<CatalogArg> {
    match value {
        Value::Object(map) => map
            .iter()
            .map(|(key, v)| CatalogArg {
                name: v
                    .get("name")
                    .and_then(|x| x.as_str())
                    .unwrap_or(key)
                    .to_string(),
                required: v
                    .get("is_required")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false),
                description: v
                    .get("description")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
            })
            .collect(),
        Value::Array(arr) => arr
            .iter()
            .filter_map(|v| {
                v.get("name").and_then(|n| n.as_str()).map(|n| CatalogArg {
                    name: n.to_string(),
                    required: v
                        .get("is_required")
                        .and_then(|x| x.as_bool())
                        .unwrap_or(false),
                    description: v
                        .get("description")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
            })
            .collect(),
        _ => vec![],
    }
}

fn parse_options(value: &Value) -> Vec<CatalogOpt> {
    let mut out = Vec::new();
    let entries: Vec<(String, &Value)> = match value {
        Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), v)).collect(),
        Value::Array(arr) => arr
            .iter()
            .filter_map(|v| {
                v.get("name")
                    .and_then(|n| n.as_str())
                    .map(|n| (n.to_string(), v))
            })
            .collect(),
        _ => vec![],
    };
    for (key, v) in entries {
        let name = v
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or(&key)
            .to_string();
        let bare = name.trim_start_matches('-');
        if SKIP_OPTS
            .iter()
            .any(|s| *s == name || *s == bare || *s == key)
        {
            continue;
        }
        let shortcut = v
            .get("shortcut")
            .and_then(|x| x.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        out.push(CatalogOpt {
            name,
            shortcut,
            accept_value: v
                .get("accept_value")
                .and_then(|x| x.as_bool())
                .unwrap_or(false),
            description: v
                .get("description")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
        });
    }
    out
}

pub fn parse_lando_help(help: &str) -> Vec<CatalogEntry> {
    let mut entries = Vec::new();
    for line in help.lines() {
        let trimmed = line.trim();
        let rest = if let Some(r) = trimmed.strip_prefix("lando ") {
            r
        } else {
            continue;
        };
        let name = rest.split_whitespace().next().unwrap_or("");
        if name.is_empty() || name.starts_with('-') || name == "<command>" {
            continue;
        }
        let desc = rest[name.len()..].trim().to_string();
        entries.push(lando_entry(name, desc));
    }
    entries
}

fn lando_entry(name: &str, description: String) -> CatalogEntry {
    let full = format!("lando {name}");
    CatalogEntry {
        tool: ToolKind::Lando,
        safety_hint: hint_from_name(&full),
        kind: if matches!(
            name,
            "start"
                | "stop"
                | "restart"
                | "info"
                | "logs"
                | "rebuild"
                | "destroy"
                | "pull"
                | "push"
        ) {
            CatalogKind::Workflow
        } else {
            CatalogKind::Palette
        },
        name: full,
        description,
        arguments: vec![],
        options: vec![],
    }
}

pub fn merge_pantheon_recipe_extras(catalog: &mut Vec<CatalogEntry>) {
    for extra in PANTHEON_RECIPE_EXTRAS {
        let name = format!("lando {extra}");
        if catalog.iter().any(|e| e.name == name) {
            continue;
        }
        catalog.push(lando_entry(extra, format!("{extra} (pantheon recipe)")));
    }
}

pub fn visible<'a>(catalog: &'a [CatalogEntry]) -> impl Iterator<Item = &'a CatalogEntry> {
    catalog.iter().filter(|e| e.kind != CatalogKind::Hidden)
}

/// Small catalog so `--demo` palette can run without Terminus.
pub fn demo_catalog() -> Vec<CatalogEntry> {
    vec![
        terminus(
            "env:deploy",
            "Deploy the current path onto a target environment",
            vec![req("site_env")],
            &["--cc", "--updatedb", "--sync-content"],
        ),
        terminus(
            "env:wipe",
            "Wipe an environment database and files",
            vec![req("site_env")],
            &[],
        ),
        terminus(
            "env:clone-content",
            "Clone database and files from one env onto another",
            vec![req("site_env"), req("to_environment")],
            &["--cc", "--updatedb", "--db-only", "--files-only"],
        ),
        terminus(
            "env:clear-cache",
            "Clear caches on an environment",
            vec![req("site_env")],
            &[],
        ),
        terminus(
            "backup:restore",
            "Restore a backup onto an environment",
            vec![req("site_env")],
            &["--element"],
        ),
        terminus(
            "connection:set",
            "Set git or sftp connection mode",
            vec![req("site_env"), req("mode")],
            &[],
        ),
        terminus(
            "multidev:delete",
            "Delete a Multidev environment",
            vec![req("site_env")],
            &["--delete-branch"],
        ),
        terminus(
            "domain:remove",
            "Remove a domain from an environment",
            vec![req("site_env"), req("domain")],
            &[],
        ),
        terminus(
            "remote:wp",
            "Run WP-CLI on a remote WordPress environment",
            vec![req("site_env")],
            &[],
        ),
        terminus(
            "remote:drush",
            "Run Drush on a remote Drupal environment",
            vec![req("site_env")],
            &[],
        ),
        lando_entry("pull", "Pull code/db/files from Pantheon".into()),
        lando_entry("rebuild", "Rebuild the local app".into()),
        lando_entry("destroy", "Destroy the local app".into()),
        lando_entry("push", "Push code/db/files to Pantheon".into()),
        lando_entry("start", "Start the local app".into()),
    ]
}

fn req(name: &str) -> CatalogArg {
    CatalogArg {
        name: name.into(),
        required: true,
        description: String::new(),
    }
}

fn terminus(
    name: &str,
    description: &str,
    arguments: Vec<CatalogArg>,
    option_names: &[&str],
) -> CatalogEntry {
    CatalogEntry {
        tool: ToolKind::Terminus,
        safety_hint: hint_from_name(name),
        kind: kind_for(name, false),
        name: name.into(),
        description: description.into(),
        arguments,
        options: option_names
            .iter()
            .map(|n| CatalogOpt {
                name: (*n).into(),
                shortcut: None,
                accept_value: *n == "--element",
                description: String::new(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "application": {"name": "Terminus", "version": "4.3.2"},
      "commands": [
        {
          "name": "env:metrics",
          "description": "Displays pages served and unique visits",
          "hidden": false,
          "definition": {
            "arguments": {
              "site_env": {
                "name": "site_env",
                "is_required": true,
                "description": "Site & environment"
              }
            },
            "options": {
              "period": {
                "name": "--period",
                "shortcut": "",
                "accept_value": true,
                "description": "month|week|day"
              },
              "help": {
                "name": "--help",
                "accept_value": false,
                "description": "Display help"
              },
              "yes": {
                "name": "--yes",
                "shortcut": "-y",
                "accept_value": false,
                "description": "Answer yes"
              }
            }
          }
        },
        {
          "name": "art",
          "description": "ASCII art",
          "hidden": false,
          "definition": { "arguments": {}, "options": {} }
        },
        {
          "name": "_complete",
          "description": "internal",
          "hidden": true,
          "definition": { "arguments": {}, "options": {} }
        }
      ]
    }"#;

    #[test]
    fn terminus_list_keeps_args_and_drops_yes_help() {
        let entries = parse_terminus_list(SAMPLE).expect("parse");
        assert_eq!(entries.len(), 3);
        let metrics = entries.iter().find(|e| e.name == "env:metrics").unwrap();
        assert_eq!(metrics.kind, CatalogKind::Workflow);
        assert_eq!(metrics.arguments.len(), 1);
        assert_eq!(metrics.arguments[0].name, "site_env");
        assert!(metrics.arguments[0].required);
        assert!(metrics.options.iter().any(|o| o.name == "--period"));
        assert!(
            !metrics
                .options
                .iter()
                .any(|o| o.name == "--yes" || o.name == "--help")
        );
        assert_eq!(metrics.safety_hint, SafetyTier::ReadOnly);

        let art = entries.iter().find(|e| e.name == "art").unwrap();
        assert_eq!(art.kind, CatalogKind::Hidden);
        let complete = entries.iter().find(|e| e.name == "_complete").unwrap();
        assert_eq!(complete.kind, CatalogKind::Hidden);
    }

    #[test]
    fn lando_help_parses_command_names() {
        let help = "\
Commands:
  lando config    Displays the lando configuration
  lando destroy   Destroys your app
  lando start     Starts your app
";
        let entries = parse_lando_help(help);
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"lando config"));
        assert!(names.contains(&"lando destroy"));
        assert!(names.contains(&"lando start"));
        let destroy = entries.iter().find(|e| e.name == "lando destroy").unwrap();
        assert_eq!(destroy.safety_hint, SafetyTier::Destructive);
        assert_eq!(destroy.kind, CatalogKind::Workflow);
    }

    #[test]
    fn live_terminus_list_parses_when_present() {
        let tools = crate::tools::detect_tools();
        let Some(term) = tools.terminus else {
            return;
        };
        let out = crate::tools::run_output(&term.path, &["list", "--format=json"], None)
            .expect("terminus list");
        let stdout = String::from_utf8_lossy(&out.stdout);
        let entries = parse_terminus_list(&stdout).expect("parse live list");
        assert!(
            entries.len() > 50,
            "expected a full terminus catalog, got {}",
            entries.len()
        );
        let metrics = entries.iter().find(|e| e.name == "env:metrics").unwrap();
        assert!(metrics.arguments.iter().any(|a| a.name == "site_env"));
        assert!(metrics.options.iter().any(|o| o.name == "--period"));
        assert!(!metrics.options.iter().any(|o| o.name == "--yes"));
    }
}
