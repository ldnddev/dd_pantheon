use crate::models::{LayoutId, MetricsPeriod, SiteOverlay};
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
    /// File debug log at XDG state `…/dd_pantheon/app.log`. Also honors `RUST_LOG`.
    #[serde(default)]
    pub debug_log: bool,
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
            debug_log: false,
            history: History::default(),
            orgs: HashMap::new(),
            locals: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SitesFile {
    #[serde(default)]
    pub sites: HashMap<String, SiteOverlay>,
}

#[derive(Clone, Debug)]
pub struct ConfigStore {
    pub path: PathBuf,
    pub config: AppConfig,
    pub dirty_since: Option<Instant>,
    pub load_warning: Option<String>,
    pub sites_path: PathBuf,
    pub sites: HashMap<String, SiteOverlay>,
    pub sites_dirty_since: Option<Instant>,
}

impl ConfigStore {
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("config.toml");
        let (config, mut load_warning) = if !path.exists() {
            (AppConfig::default(), None)
        } else {
            match fs::read_to_string(&path) {
                Ok(raw) => match toml::from_str::<AppConfig>(&raw) {
                    Ok(config) => (config, None),
                    Err(err) => (
                        AppConfig::default(),
                        Some(format!(
                            "config.toml parse failed ({err}); using defaults, file not overwritten"
                        )),
                    ),
                },
                Err(err) => (
                    AppConfig::default(),
                    Some(format!("could not read config.toml: {err}")),
                ),
            }
        };
        let sites_path = dir.join("sites.toml");
        let sites = match load_sites_file(&sites_path) {
            Ok(map) => map,
            Err(warn) => {
                if load_warning.is_none() {
                    load_warning = Some(warn);
                }
                HashMap::new()
            }
        };
        Self {
            path,
            config,
            dirty_since: None,
            load_warning,
            sites_path,
            sites,
            sites_dirty_since: None,
        }
    }

    pub fn mark_dirty(&mut self) {
        self.dirty_since = Some(Instant::now());
    }

    pub fn mark_sites_dirty(&mut self) {
        self.sites_dirty_since = Some(Instant::now());
    }

    pub fn flush_if_due(&mut self) {
        if let Some(since) = self.dirty_since {
            if since.elapsed() >= CONFIG_WRITE_DEBOUNCE {
                if let Err(err) = self.write_now() {
                    self.load_warning = Some(format!("failed to write config: {err:#}"));
                }
                self.dirty_since = None;
            }
        }
        if let Some(since) = self.sites_dirty_since {
            if since.elapsed() >= CONFIG_WRITE_DEBOUNCE {
                if let Err(err) = self.write_sites_now() {
                    self.load_warning = Some(format!("failed to write sites.toml: {err:#}"));
                }
                self.sites_dirty_since = None;
            }
        }
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

    pub fn write_sites_now(&self) -> Result<()> {
        if let Some(parent) = self.sites_path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        let file = SitesFile {
            sites: self.sites.clone(),
        };
        let body = toml::to_string_pretty(&file).context("serialize sites.toml")?;
        let tmp = self.sites_path.with_extension("toml.tmp");
        fs::write(&tmp, body).with_context(|| format!("write {}", tmp.display()))?;
        fs::rename(&tmp, &self.sites_path).with_context(|| {
            format!("rename {} -> {}", tmp.display(), self.sites_path.display())
        })?;
        Ok(())
    }

    /// Persist a bound path: overlay entry → `sites.toml`; else `[locals]`.
    pub fn persist_local_path(&mut self, site: &str, path: &Path) {
        if self.sites.contains_key(site) {
            if let Some(entry) = self.sites.get_mut(site) {
                entry.local_path = Some(path.to_path_buf());
            }
            self.mark_sites_dirty();
            if self.config.locals.remove(site).is_some() {
                self.mark_dirty();
            }
        } else {
            self.config
                .locals
                .insert(site.to_string(), path.display().to_string());
            self.mark_dirty();
        }
    }
}

fn load_sites_file(path: &Path) -> Result<HashMap<String, SiteOverlay>, String> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let raw =
        fs::read_to_string(path).map_err(|err| format!("could not read sites.toml: {err}"))?;
    let file: SitesFile = toml::from_str(&raw).map_err(|err| {
        format!("sites.toml parse failed ({err}); using empty overlay, file not overwritten")
    })?;
    Ok(file.sites)
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
            debug_log: true,
            history: History {
                palette: vec!["env:clear-cache".into()],
                cms: vec!["status".into()],
            },
            orgs: HashMap::from([("acme-wp".into(), "uuid".into())]),
            ..AppConfig::default()
        };
        let raw = toml::to_string_pretty(&cfg).expect("toml");
        let debug = raw.find("debug_log").expect("debug_log scalar");
        let hist = raw.find("[history]").expect("history table");
        let orgs = raw.find("[orgs]").expect("orgs table");
        assert!(
            debug < hist && hist < orgs,
            "scalars then [history] then [orgs]: {raw}"
        );
    }

    #[test]
    fn sites_toml_roundtrip_and_persist_prefers_overlay() {
        let dir = std::env::temp_dir().join(format!(
            "dd_pantheon_sites_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("sites.toml"),
            "[sites.acme-wp]\ncms = \"wordpress\"\nmultidev_ok = false\n",
        )
        .unwrap();
        let mut store = ConfigStore::load(&dir);
        let overlay = store.sites.get("acme-wp").expect("overlay");
        assert_eq!(overlay.cms, Some(crate::models::Framework::WordPress));
        assert!(!overlay.multidev_ok);
        store.persist_local_path("acme-wp", Path::new("/tmp/acme-wp"));
        assert_eq!(
            store.sites["acme-wp"].local_path.as_deref(),
            Some(Path::new("/tmp/acme-wp"))
        );
        assert!(!store.config.locals.contains_key("acme-wp"));
        store.persist_local_path("other-site", Path::new("/tmp/other"));
        assert_eq!(store.config.locals["other-site"], "/tmp/other");
        let _ = fs::remove_dir_all(dir);
    }
}

pub fn default_config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/ldnddev/dd_pantheon")
}
