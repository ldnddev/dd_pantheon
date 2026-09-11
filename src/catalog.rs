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
    "local:clone",
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

/// Core `lando` tasks plus Pantheon-recipe extras. Names always start with `lando `.
pub const LANDO_CORE_COMMANDS: &[(&str, &str)] = &[
    ("config", "Displays the lando configuration"),
    ("destroy", "Destroys your app"),
    ("exec", "Runs command(s) on a service"),
    ("info", "Prints info about your app"),
    ("init", "Initializes a Landofile"),
    ("list", "Lists running lando apps and containers"),
    ("logs", "Displays logs for your app"),
    ("poweroff", "Spins down all lando related containers"),
    ("rebuild", "Rebuilds your app from scratch, preserving data"),
    ("restart", "Restarts your app"),
    ("start", "Starts your app"),
    ("stop", "Stops your app"),
    ("update", "Updates lando"),
    ("version", "Displays lando version information"),
];

const LANDO_PANTHEON_COMMANDS: &[(&str, &str)] = &[
    ("pull", "Pull code/db/files from Pantheon"),
    ("push", "Push code/db/files to Pantheon"),
    ("terminus", "Run terminus inside the app"),
    ("drush", "Run drush inside the app"),
    ("wp", "Run wp-cli inside the app"),
    ("composer", "Run composer inside the app"),
    ("mysql", "Drop into a MySQL shell"),
    ("db-import", "Import a database dump"),
    ("db-export", "Export a database dump"),
];

pub fn lando_entry(name: &str, description: String) -> CatalogEntry {
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
    for (cmd, desc) in LANDO_PANTHEON_COMMANDS {
        let name = format!("lando {cmd}");
        if catalog.iter().any(|e| e.name == name) {
            continue;
        }
        catalog.push(lando_entry(cmd, (*desc).into()));
    }
    for extra in PANTHEON_RECIPE_EXTRAS {
        let name = format!("lando {extra}");
        if catalog.iter().any(|e| e.name == name) {
            continue;
        }
        catalog.push(lando_entry(extra, format!("{extra} (pantheon recipe)")));
    }
}

pub fn seed_lando_catalog() -> Vec<CatalogEntry> {
    let mut out: Vec<CatalogEntry> = LANDO_CORE_COMMANDS
        .iter()
        .map(|(cmd, desc)| lando_entry(cmd, (*desc).into()))
        .collect();
    merge_pantheon_recipe_extras(&mut out);
    out
}

/// Insert missing built-in Lando commands. Names stay `lando <cmd>`.
pub fn ensure_lando_catalog(catalog: &mut Vec<CatalogEntry>) {
    for entry in seed_lando_catalog() {
        if !catalog.iter().any(|e| e.name == entry.name) {
            catalog.push(entry);
        }
    }
}

pub fn is_lando_init(name: &str) -> bool {
    name.eq_ignore_ascii_case("lando init")
}

pub fn invocation_prefix(entry: &CatalogEntry) -> String {
    match entry.tool {
        ToolKind::Lando => {
            if entry.name.starts_with("lando ") {
                entry.name.clone()
            } else {
                format!("lando {}", entry.name)
            }
        }
        ToolKind::Terminus => format!("terminus {}", entry.name),
        ToolKind::Git => format!("git {}", entry.name),
    }
}

/// Placeholder syntax, e.g. `terminus tag:add <site_name> <organization> <tag>`.
pub fn command_usage(entry: &CatalogEntry) -> String {
    let mut parts = vec![invocation_prefix(entry)];
    for arg in &entry.arguments {
        if arg.required {
            parts.push(format!("<{}>", arg.name));
        } else {
            parts.push(format!("[{}]", arg.name));
        }
    }
    let mut shown = 0usize;
    for opt in &entry.options {
        if shown >= 4 {
            parts.push("[options]".into());
            break;
        }
        let flag = if opt.name.starts_with('-') {
            opt.name.clone()
        } else {
            format!("--{}", opt.name)
        };
        if opt.accept_value {
            parts.push(format!("[{flag}=<value>]"));
        } else {
            parts.push(format!("[{flag}]"));
        }
        shown += 1;
    }
    if entry.arguments.is_empty() && entry.options.is_empty() {
        parts.push("[options]".into());
    }
    parts.join(" ")
}

/// Live argv with empty fields kept as `<name>` so the operator sees remaining holes.
pub fn command_preview(
    entry: &CatalogEntry,
    filled: &[(String, String)],
    toggles: &[(String, bool)],
) -> String {
    let mut parts = vec![invocation_prefix(entry)];
    for arg in &entry.arguments {
        let val = filled
            .iter()
            .find(|(k, _)| k == &arg.name)
            .map(|(_, v)| v.trim())
            .unwrap_or("");
        if val.is_empty() {
            parts.push(format!("<{}>", arg.name));
        } else {
            parts.push(val.to_string());
        }
    }
    if let Some(el) = filled
        .iter()
        .find(|(k, v)| (k == "--element" || k == "element") && !v.trim().is_empty())
    {
        parts.push(format!("--element={}", el.1.trim()));
    }
    for (flag, on) in toggles {
        if *on {
            parts.push(flag.clone());
        }
    }
    if let Some((_, extra)) = filled
        .iter()
        .find(|(k, v)| k == "extra" && !v.trim().is_empty())
    {
        parts.push(extra.trim().to_string());
    }
    parts.join(" ")
}

/// Concrete example using filled values, then per-arg samples.
pub fn command_example(entry: &CatalogEntry, filled: &[(String, String)]) -> String {
    if let Some(fixed) = curated_example(entry) {
        if filled.iter().all(|(_, v)| v.trim().is_empty()) {
            return fixed.to_string();
        }
    }
    let mut parts = vec![invocation_prefix(entry)];
    for arg in &entry.arguments {
        let val = filled
            .iter()
            .find(|(k, v)| k == &arg.name && !v.trim().is_empty())
            .map(|(_, v)| v.as_str())
            .unwrap_or_else(|| sample_value(&arg.name));
        parts.push(val.to_string());
    }
    if let Some(extra) = filled
        .iter()
        .find(|(k, v)| k == "extra" && !v.trim().is_empty())
    {
        parts.push(extra.1.clone());
    } else if let Some(tail) = curated_extra(entry) {
        parts.push(tail.to_string());
    }
    parts.join(" ")
}

pub fn arg_placeholder(name: &str) -> String {
    match name {
        "extra" => "type optional flags…  e.g. --note=\"deploy msg\"".into(),
        "--element" | "element" => "type element…  all | code | database | files".into(),
        "site_env" | "site_env_id" => "type site.env…  e.g. acme-wp.test".into(),
        "site_name" => "type site machine name…  e.g. acme-wp".into(),
        "organization" => "type org name or id…".into(),
        "tag" => "type tag name…  e.g. production".into(),
        "domain" => "type domain…  e.g. www.example.com".into(),
        "mode" => "type mode…  git or sftp".into(),
        _ => format!("type {name}…"),
    }
}

pub fn arg_help(entry: &CatalogEntry, name: &str) -> String {
    if name == "extra" {
        return "Additional argv appended as typed. --yes is injected on confirm, not here.".into();
    }
    if let Some(arg) = entry.arguments.iter().find(|a| a.name == name) {
        if !arg.description.trim().is_empty() {
            return arg.description.clone();
        }
    }
    if let Some(opt) = entry
        .options
        .iter()
        .find(|o| o.name == name || o.name.trim_start_matches('-') == name.trim_start_matches('-'))
    {
        if !opt.description.trim().is_empty() {
            return opt.description.clone();
        }
    }
    fallback_arg_help(name).to_string()
}

pub fn sample_value(name: &str) -> &'static str {
    match name {
        "site_name" | "site" => "acme-wp",
        "site_id" => "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
        "site_env" | "site_env_id" => "acme-wp.test",
        "organization" | "org" | "org_id" => "my-org",
        "tag" => "production",
        "to_environment" | "target_env" | "from_environment" => "live",
        "domain" => "www.example.com",
        "mode" => "git",
        "name" => "feat-x",
        "message" | "note" => "deploy from dd_pantheon",
        "element" | "--element" => "all",
        _ => "<value>",
    }
}

fn fallback_arg_help(name: &str) -> &'static str {
    match name {
        "site_name" => "Pantheon site machine name.",
        "site_id" => "Pantheon site UUID.",
        "site_env" | "site_env_id" => "Site and environment as site.env.",
        "organization" | "org" | "org_id" => "Organization name or UUID that owns the tag.",
        "tag" => "Tag label to add or remove.",
        "to_environment" | "target_env" => {
            "Destination environment id (dev, test, live, or multidev)."
        }
        "from_environment" => "Source environment to copy from.",
        "domain" => "Custom hostname to add or remove.",
        "mode" => "Connection mode: git or sftp.",
        "name" => "Multidev or site name.",
        "element" | "--element" => "Backup element: all, code, database, or files.",
        _ => "Required value for this command.",
    }
}

fn curated_example(entry: &CatalogEntry) -> Option<&'static str> {
    Some(match entry.name.as_str() {
        "tag:add" => "terminus tag:add acme-wp my-org production",
        "tag:remove" | "tag:rm" => "terminus tag:remove acme-wp my-org production",
        "tag:list" => "terminus tag:list acme-wp my-org",
        "env:deploy" => "terminus env:deploy acme-wp.test --updatedb --note=\"release\"",
        "env:clone-content" => "terminus env:clone-content acme-wp.live dev --cc --updatedb",
        "env:wipe" => "terminus env:wipe acme-wp.dev",
        "env:clear-cache" => "terminus env:clear-cache acme-wp.live",
        "backup:create" => "terminus backup:create acme-wp.test --element=all",
        "backup:restore" => "terminus backup:restore acme-wp.dev --element=database",
        "connection:set" => "terminus connection:set acme-wp.dev git",
        "multidev:create" => "terminus multidev:create acme-wp feat-x",
        "domain:add" => "terminus domain:add acme-wp.live www.example.com",
        "remote:wp" => "terminus remote:wp acme-wp.live -- plugin list",
        "remote:drush" => "terminus remote:drush acme-wp.live -- status",
        "lando pull" => "lando pull --code=live --database=live --files=live",
        "lando push" => "lando push --code --database=none --files=none",
        "lando start" => "lando start",
        "lando stop" => "lando stop",
        "lando rebuild" => "lando rebuild -y",
        "lando init" => "lando init --source pantheon",
        _ => return None,
    })
}

fn curated_extra(entry: &CatalogEntry) -> Option<&'static str> {
    match entry.name.as_str() {
        "lando pull" => Some("--code=live --database=live --files=live"),
        "lando push" => Some("--code --database=none --files=none"),
        "remote:wp" => Some("-- plugin list"),
        "remote:drush" => Some("-- status"),
        "env:deploy" => Some("--note=\"release\""),
        _ => None,
    }
}

pub fn visible<'a>(catalog: &'a [CatalogEntry]) -> impl Iterator<Item = &'a CatalogEntry> {
    catalog.iter().filter(|e| e.kind != CatalogKind::Hidden)
}

/// Small catalog so `--demo` palette can run without Terminus.
pub fn demo_catalog() -> Vec<CatalogEntry> {
    let base = vec![
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
    ];
    let mut out = base;
    ensure_lando_catalog(&mut out);
    out
}

fn req(name: &str) -> CatalogArg {
    CatalogArg {
        name: name.into(),
        required: true,
        description: fallback_arg_help(name).into(),
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
    fn seed_lando_names_start_with_lando_and_include_init() {
        let seeded = seed_lando_catalog();
        assert!(seeded.iter().any(|e| e.name == "lando init"));
        assert!(seeded.iter().any(|e| e.name == "lando start"));
        assert!(seeded.iter().any(|e| e.name == "lando pull"));
        assert!(
            seeded
                .iter()
                .all(|e| e.tool == ToolKind::Lando && e.name.starts_with("lando "))
        );
        let mut catalog = vec![];
        ensure_lando_catalog(&mut catalog);
        let n = catalog.len();
        ensure_lando_catalog(&mut catalog);
        assert_eq!(catalog.len(), n, "ensure is idempotent");
    }

    #[test]
    fn command_usage_and_example_cover_terminus_and_lando() {
        let tag = terminus(
            "tag:add",
            "Adds a tag on a site within an organization.",
            vec![req("site_name"), req("organization"), req("tag")],
            &[],
        );
        assert_eq!(
            command_usage(&tag),
            "terminus tag:add <site_name> <organization> <tag>"
        );
        assert_eq!(
            command_example(&tag, &[]),
            "terminus tag:add acme-wp my-org production"
        );
        let filled = vec![
            ("site_name".into(), "dd-wordpress".into()),
            ("organization".into(), "ldnd".into()),
            ("tag".into(), "test".into()),
        ];
        assert_eq!(
            command_example(&tag, &filled),
            "terminus tag:add dd-wordpress ldnd test"
        );
        let pull = lando_entry("pull", "Pull code/db/files from Pantheon".into());
        assert!(command_usage(&pull).starts_with("lando pull"));
        assert!(command_example(&pull, &[]).contains("lando pull"));
        assert_eq!(arg_placeholder("tag"), "type tag name…  e.g. production");
        assert_eq!(
            command_preview(&tag, &filled, &[]),
            "terminus tag:add dd-wordpress ldnd test"
        );
        assert!(command_preview(&tag, &[], &[]).contains("<tag>"));
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
