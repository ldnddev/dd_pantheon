use crate::plan::PlanTarget;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub const METRICS_STALE: Duration = Duration::from_secs(15 * 60);
pub const CACHE_OK: f64 = 0.80;
pub const CACHE_WARN: f64 = 0.50;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutId {
    #[default]
    ClassicStack,
    ThreeColumn,
    TabbedInspector,
}

impl LayoutId {
    pub fn cycle(self) -> Self {
        match self {
            Self::ClassicStack => Self::ThreeColumn,
            Self::ThreeColumn => Self::TabbedInspector,
            Self::TabbedInspector => Self::ClassicStack,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ClassicStack => "classic",
            Self::ThreeColumn => "three-column",
            Self::TabbedInspector => "tabbed",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InspectorTab {
    #[default]
    Info,
    Metrics,
    Local,
    Actions,
}

impl InspectorTab {
    pub fn from_digit(n: u8) -> Option<Self> {
        match n {
            1 => Some(Self::Info),
            2 => Some(Self::Metrics),
            3 => Some(Self::Local),
            4 => Some(Self::Actions),
            _ => None,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Info => Self::Metrics,
            Self::Metrics => Self::Local,
            Self::Local => Self::Actions,
            Self::Actions => Self::Info,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Info => Self::Actions,
            Self::Metrics => Self::Info,
            Self::Local => Self::Metrics,
            Self::Actions => Self::Local,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "Info",
            Self::Metrics => "Metrics",
            Self::Local => "Local",
            Self::Actions => "Actions",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Framework {
    WordPress,
    Drupal,
    Other,
}

impl Framework {
    pub fn from_terminus(s: &str) -> Self {
        let s = s.to_ascii_lowercase();
        if s.contains("wordpress") || s == "wp" {
            Self::WordPress
        } else if s.contains("drupal") {
            Self::Drupal
        } else {
            Self::Other
        }
    }
}

impl Framework {
    pub fn label(self) -> &'static str {
        match self {
            Self::WordPress => "wordpress",
            Self::Drupal => "drupal",
            Self::Other => "other",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::WordPress => "wp",
            Self::Drupal => "d10",
            Self::Other => "cms",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionMode {
    Git,
    Sftp,
    Unknown,
}

impl ConnectionMode {
    pub fn from_terminus(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "git" => Self::Git,
            "sftp" => Self::Sftp,
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Git => "git",
            Self::Sftp => "sftp",
            Self::Unknown => "?",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Tag {
    pub name: String,
    pub org: String,
}

#[derive(Clone, Debug)]
pub struct OrgRef {
    pub org_id: String,
    pub org_name: String,
}

#[derive(Clone, Debug)]
pub struct UpstreamRef {
    pub id: String,
    pub label: String,
    pub machine_name: String,
    pub framework: Option<String>,
}

impl UpstreamRef {
    pub fn create_id(&self) -> &str {
        if !self.machine_name.is_empty() {
            &self.machine_name
        } else {
            &self.id
        }
    }
}

#[derive(Clone, Debug)]
pub struct LocalApp {
    pub path: PathBuf,
    pub lando_name: Option<String>,
    pub recipe: Option<String>,
    pub framework: Option<Framework>,
    pub terminus_site: Option<String>,
    pub running: Option<bool>,
    pub url: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SiteOverlay {
    pub cms: Option<Framework>,
    pub multidev_ok: bool,
    pub composer_managed: bool,
    pub git_branch: Option<String>,
    pub local_path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct Site {
    pub name: String,
    pub id: String,
    pub label: Option<String>,
    pub framework: Framework,
    pub region: Option<String>,
    pub frozen: bool,
    pub plan_name: Option<String>,
    pub owner: Option<String>,
    pub upstream: Option<String>,
    pub upstream_label: Option<String>,
    pub memberships: Option<String>,
    pub tags: Vec<Tag>,
    pub orgs: Vec<OrgRef>,
    pub local: Option<LocalApp>,
    pub overlay: Option<SiteOverlay>,
}

#[derive(Clone, Debug)]
pub struct Env {
    pub id: String,
    pub site: String,
    pub domain: Option<String>,
    pub connection_mode: ConnectionMode,
    pub locked: bool,
    pub initialized: bool,
    pub php_version: Option<String>,
    pub php_runtime_generation: Option<String>,
    pub drush_version: Option<String>,
    pub created: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MetricsPeriod {
    #[default]
    Day,
    Week,
    Month,
}

impl MetricsPeriod {
    pub fn label(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
        }
    }

    pub fn key_label(self) -> &'static str {
        match self {
            Self::Day => "d",
            Self::Week => "w",
            Self::Month => "M",
        }
    }
}

#[derive(Clone, Debug)]
pub struct MetricsPoint {
    pub datetime: String,
    pub visits: u64,
    pub pages_served: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_hit_ratio: f64,
}

#[derive(Clone, Debug)]
pub struct MetricsSeries {
    pub target: PlanTarget,
    pub period: MetricsPeriod,
    pub datapoints: String,
    pub points: Vec<MetricsPoint>,
    pub fetched_at: Instant,
}

pub fn cache_ratio_level(ratio: f64) -> CacheLevel {
    if ratio >= CACHE_OK {
        CacheLevel::Ok
    } else if ratio >= CACHE_WARN {
        CacheLevel::Warn
    } else {
        CacheLevel::Bad
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheLevel {
    Ok,
    Warn,
    Bad,
}

#[derive(Clone, Debug)]
pub struct Backup {
    pub file: String,
    pub size: String,
    pub date: String,
    pub expiry: String,
    pub initiator: String,
}

#[derive(Clone, Debug)]
pub struct ActionItem {
    pub id: &'static str,
    pub label: &'static str,
}

pub fn default_actions() -> Vec<ActionItem> {
    vec![
        ActionItem {
            id: "backup",
            label: "backup",
        },
        ActionItem {
            id: "cache",
            label: "clear cache",
        },
        ActionItem {
            id: "wake",
            label: "wake",
        },
        ActionItem {
            id: "deploy",
            label: "deploy",
        },
        ActionItem {
            id: "cms",
            label: "CMS",
        },
        ActionItem {
            id: "create",
            label: "create site",
        },
        ActionItem {
            id: "login",
            label: "login",
        },
        ActionItem {
            id: "lando-start",
            label: "lando start",
        },
    ]
}
