use crate::models::{LayoutId, MetricsPeriod};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const CONFIG_WRITE_DEBOUNCE: Duration = Duration::from_millis(500);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub layout: LayoutId,
    #[serde(default)]
    pub last_site: Option<String>,
    #[serde(default)]
    pub last_env: Option<String>,
    #[serde(default)]
    pub metrics_period: MetricsPeriod,
    /// Last 50 palette/CMS lines. Table (must sit before `[orgs]` / `[locals]`).
    #[serde(default)]
    pub history: History,
    /// Last-picked org **id** (UUID) per terminus site name.
    #[serde(default)]
    pub orgs: HashMap<String, String>,
    /// Fallback local path bindings (site name → path).
    #[serde(default)]
    pub locals: HashMap<String, String>,
}

pub const HISTORY_CAP: usize = 50;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct History {
    #[serde(default)]
    pub palette: Vec<String>,
    #[serde(default)]
    pub cms: Vec<String>,
}

pub fn looks_like_machine_token(line: &str) -> bool {
    line.to_ascii_lowercase().contains("--machine-token")
}

pub fn push_history(list: &mut Vec<String>, line: impl Into<String>) {
    let line = line.into().trim().to_string();
    if line.is_empty() || looks_like_machine_token(&line) {
        return;
    }
    list.retain(|e| e != &line);
    list.insert(0, line);
    list.truncate(HISTORY_CAP);
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            layout: LayoutId::ClassicStack,
            last_site: None,
            last_env: None,
            metrics_period: MetricsPeriod::Day,
            history: History::default(),
            orgs: HashMap::new(),
            locals: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ConfigStore {
    pub path: PathBuf,
    pub config: AppConfig,
    pub dirty_since: Option<Instant>,
    pub load_warning: Option<String>,
}

impl ConfigStore {
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("config.toml");
        if !path.exists() {
            return Self {
                path,
                config: AppConfig::default(),
                dirty_since: None,
                load_warning: None,
            };
        }
        match fs::read_to_string(&path) {
            Ok(raw) => match toml::from_str::<AppConfig>(&raw) {
                Ok(config) => Self {
                    path,
                    config,
                    dirty_since: None,
                    load_warning: None,
                },
                Err(err) => Self {
                    path,
                    config: AppConfig::default(),
                    dirty_since: None,
                    load_warning: Some(format!(
                        "config.toml parse failed ({err}); using defaults, file not overwritten"
                    )),
                },
            },
            Err(err) => Self {
                path,
                config: AppConfig::default(),
                dirty_since: None,
                load_warning: Some(format!("could not read config.toml: {err}")),
            },
        }
    }

    pub fn mark_dirty(&mut self) {
        self.dirty_since = Some(Instant::now());
    }

    pub fn flush_if_due(&mut self) {
        let Some(since) = self.dirty_since else {
            return;
        };
        if since.elapsed() < CONFIG_WRITE_DEBOUNCE {
            return;
        }
        if let Err(err) = self.write_now() {
            self.load_warning = Some(format!("failed to write config: {err:#}"));
        }
        self.dirty_since = None;
    }

    pub fn write_now(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        let body = toml::to_string_pretty(&self.config).context("serialize config.toml")?;
        let tmp = self.path.with_extension("toml.tmp");
        fs::write(&tmp, body).with_context(|| format!("write {}", tmp.display()))?;
        fs::rename(&tmp, &self.path)
            .with_context(|| format!("rename {} -> {}", tmp.display(), self.path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_drops_machine_token_and_caps() {
        let mut list = Vec::new();
        push_history(&mut list, "env:clear-cache");
        push_history(&mut list, "auth:login --machine-token=secret");
        push_history(&mut list, "remote:wp -- plugin list");
        push_history(&mut list, "env:clear-cache");
        assert_eq!(
            list,
            vec![
                "env:clear-cache".to_string(),
                "remote:wp -- plugin list".into()
            ]
        );
        for i in 0..60 {
            push_history(&mut list, format!("cmd-{i}"));
        }
        assert_eq!(list.len(), HISTORY_CAP);
        assert_eq!(list[0], "cmd-59");
    }

    #[test]
    fn history_table_serializes_before_orgs() {
        let cfg = AppConfig {
            history: History {
                palette: vec!["env:clear-cache".into()],
                cms: vec!["status".into()],
            },
            orgs: HashMap::from([("acme-wp".into(), "uuid".into())]),
            ..AppConfig::default()
        };
        let raw = toml::to_string_pretty(&cfg).expect("toml");
        let hist = raw.find("[history]").expect("history table");
        let orgs = raw.find("[orgs]").expect("orgs table");
        assert!(hist < orgs, "scalars/history must precede [orgs]: {raw}");
    }
}

pub fn default_config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/ldnddev/dd_pantheon")
}
