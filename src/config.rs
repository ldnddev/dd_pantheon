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
    /// Last-picked org **id** (UUID) per terminus site name.
    #[serde(default)]
    pub orgs: HashMap<String, String>,
    /// Fallback local path bindings (site name → path).
    #[serde(default)]
    pub locals: HashMap<String, String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            layout: LayoutId::ClassicStack,
            last_site: None,
            last_env: None,
            metrics_period: MetricsPeriod::Day,
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

pub fn default_config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/ldnddev/dd_pantheon")
}
