use crate::catalog::CatalogEntry;
use crate::config::ConfigStore;
use crate::fixtures::{DemoData, dummy_backup_plan};
use crate::jobs::{Job, JobHub, JobId};
use crate::models::{
    ActionItem, Backup, Env, InspectorTab, LayoutId, LocalApp, MetricsPeriod, MetricsSeries,
    OrgRef, Site, UpstreamRef, default_actions,
};
use crate::plan::{CommandPlan, StagedPlan, ToolKind};
use crate::theme::{Theme, ThemeStatus};
use crate::toast::{Toast, ToastLevel};
use crate::tools::Toolset;
use ratatui::layout::Rect;
use ratatui::widgets::ListState;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

pub use crate::doctor::AuthState;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FocusPane {
    #[default]
    Tree,
    Inspector,
    Preview,
    Log,
}

impl FocusPane {
    pub fn next(self) -> Self {
        match self {
            Self::Tree => Self::Inspector,
            Self::Inspector => Self::Preview,
            Self::Preview => Self::Log,
            Self::Log => Self::Tree,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Tree => Self::Log,
            Self::Inspector => Self::Tree,
            Self::Preview => Self::Inspector,
            Self::Log => Self::Preview,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeSel {
    None,
    Site(String),
    Env { site: String, env: String },
}

#[derive(Clone, Debug)]
pub enum Modal {
    Help {
        scroll: u16,
    },
    Theme {
        scroll: u16,
    },
    Doctor {
        scroll: u16,
    },
    Login {
        token: String,
        use_env_token: bool,
        env_available: bool,
    },
    ConfirmDestructive {
        plan: CommandPlan,
    },
    LiveGate {
        plan: CommandPlan,
        expected: String,
        typed: String,
    },
    Filter {
        query: String,
    },
    TagAdd {
        value: String,
    },
    OrgPicker {
        site: String,
        orgs: Vec<OrgRef>,
        selected: usize,
    },
    TagPicker {
        tags: Vec<String>,
        selected: usize,
    },
    DiffstatDirty {
        site: String,
        env: String,
        files: Vec<String>,
    },
    SiteCreate {
        form: SiteCreateForm,
    },
    MultidevCreate {
        site: String,
        name: String,
        sources: Vec<String>,
        source_idx: usize,
    },
    CloneContent {
        site: String,
        target: String,
        origins: Vec<String>,
        origin_idx: usize,
        cc: bool,
        db_only: bool,
        files_only: bool,
        updatedb: bool,
    },
    Error {
        msg: String,
    },
    QuitConfirm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateField {
    Name,
    Label,
    Org,
    Upstream,
    Bind,
    Path,
}

impl CreateField {
    pub fn next(self) -> Self {
        match self {
            Self::Name => Self::Label,
            Self::Label => Self::Org,
            Self::Org => Self::Upstream,
            Self::Upstream => Self::Bind,
            Self::Bind => Self::Path,
            Self::Path => Self::Name,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Name => Self::Path,
            Self::Label => Self::Name,
            Self::Org => Self::Label,
            Self::Upstream => Self::Org,
            Self::Bind => Self::Upstream,
            Self::Path => Self::Bind,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SiteCreateForm {
    pub name: String,
    pub label: String,
    pub orgs: Vec<OrgRef>,
    pub org_idx: usize,
    pub upstreams: Vec<UpstreamRef>,
    pub upstream_idx: usize,
    pub bind_local: bool,
    pub local_path: String,
    pub focus: CreateField,
}

#[derive(Clone, Debug)]
pub struct TagChipHit {
    pub name: String,
    pub body: Rect,
    pub close: Rect,
}

#[derive(Clone, Copy, Debug)]
pub struct PeriodHit {
    pub period: MetricsPeriod,
    pub area: Rect,
}

#[derive(Clone, Debug)]
pub struct TreeRow {
    pub depth: u16,
    pub site: String,
    pub env: Option<String>,
    pub expanded: bool,
}

impl TreeRow {
    pub fn is_site(&self) -> bool {
        self.env.is_none()
    }
}

pub struct AppState {
    pub theme: Theme,
    pub theme_status: ThemeStatus,
    pub header_copy: String,
    pub demo: bool,
    pub layout: LayoutId,
    pub focus: FocusPane,
    pub inspector_tab: InspectorTab,
    pub modal: Option<Modal>,
    pub toast: Option<Toast>,
    pub toast_area: Option<Rect>,
    pub modal_area: Option<Rect>,

    pub sites: Vec<Site>,
    pub envs: HashMap<String, Vec<Env>>,
    pub expanded: HashSet<String>,
    pub selected: TreeSel,
    pub filter: String,
    pub tag_filter: Option<String>,
    pub tree_rows: Vec<TreeRow>,
    pub tree_state: ListState,
    pub action_state: ListState,
    pub actions: Vec<ActionItem>,

    pub log_scroll: u16,
    pub preview_scroll: u16,
    pub inspector_scroll: u16,
    pub log_lines: Vec<String>,
    pub job_running: bool,

    pub current: Option<StagedPlan>,
    pub metrics: HashMap<(String, MetricsPeriod), MetricsSeries>,
    pub metrics_period: MetricsPeriod,
    pub backups: HashMap<String, Vec<Backup>>,

    pub config: ConfigStore,
    pub should_quit: bool,
    pub last_frame: Rect,

    pub tools: Toolset,
    pub tools_enabled: bool,
    pub auth: AuthState,
    pub catalog: Vec<CatalogEntry>,
    pub doctor_warnings: Vec<String>,
    pub skip_detect: bool,

    pub job_hub: JobHub,
    pub jobs: HashMap<JobId, Job>,
    pub mutating_slots: HashSet<String>,
    pub pending_workflow: Option<StagedPlan>,
    pub inflight_readonly: HashMap<String, JobId>,
    pub pending_env_list: Option<(String, Instant)>,
    pub pending_env_info: Option<(String, String, Instant)>,
    pub pending_org_list: Option<(String, Instant)>,
    pub pending_tag_list: Option<(String, String, Instant)>,
    pub pending_metrics: Option<(String, String, Instant)>,
    pub pending_backup_list: Option<(String, String, Instant)>,
    pub pending_lando: Option<Instant>,
    pub pending_local: Option<LocalApp>,
    pub create_orgs: Vec<OrgRef>,
    pub create_upstreams: Vec<UpstreamRef>,
    pub pending_create_bind: Option<(String, PathBuf)>,
    pub metrics_error: Option<String>,
    pub period_hits: Vec<PeriodHit>,
    pub org_prompted: HashSet<String>,
    pub selected_chip: Option<String>,
    pub tag_chips: Vec<TagChipHit>,

    pub tree_area: Rect,
    pub inspector_area: Rect,
    pub preview_area: Rect,
    pub log_area: Rect,
    pub footer_area: Rect,
}

impl AppState {
    pub fn from_demo(theme: Theme, config: ConfigStore, data: DemoData) -> Self {
        let theme_status = if let Some(warn) = theme.warning.clone() {
            ThemeStatus::warning(warn)
        } else {
            ThemeStatus::healthy(theme.source, theme.version)
        };

        let layout = config.config.layout;

        let mut state = Self {
            header_copy: String::new(),
            demo: true,
            layout,
            focus: FocusPane::Tree,
            inspector_tab: InspectorTab::Info,
            modal: None,
            toast: None,
            toast_area: None,
            modal_area: None,
            sites: data.sites,
            envs: data.envs,
            expanded: HashSet::from(["acme-wp".into()]),
            selected: TreeSel::Env {
                site: "acme-wp".into(),
                env: "test".into(),
            },
            filter: String::new(),
            tag_filter: None,
            tree_rows: Vec::new(),
            tree_state: ListState::default(),
            action_state: ListState::default(),
            actions: default_actions(),
            log_scroll: 0,
            preview_scroll: 0,
            inspector_scroll: 0,
            log_lines: data.log_lines,
            job_running: false,
            current: Some(data.plan),
            metrics: data.metrics,
            metrics_period: config.config.metrics_period,
            backups: data.backups,
            config,
            should_quit: false,
            last_frame: Rect::default(),
            tools: Toolset::default(),
            tools_enabled: false,
            auth: AuthState::Unknown,
            catalog: Vec::new(),
            doctor_warnings: Vec::new(),
            skip_detect: false,
            job_hub: JobHub::new(),
            jobs: HashMap::new(),
            mutating_slots: HashSet::new(),
            pending_workflow: None,
            inflight_readonly: HashMap::new(),
            pending_env_list: None,
            pending_env_info: None,
            pending_org_list: None,
            pending_tag_list: None,
            pending_metrics: None,
            pending_backup_list: None,
            pending_lando: None,
            pending_local: None,
            create_orgs: Vec::new(),
            create_upstreams: Vec::new(),
            pending_create_bind: None,
            metrics_error: None,
            period_hits: Vec::new(),
            org_prompted: HashSet::new(),
            selected_chip: None,
            tag_chips: Vec::new(),
            tree_area: Rect::default(),
            inspector_area: Rect::default(),
            preview_area: Rect::default(),
            log_area: Rect::default(),
            footer_area: Rect::default(),
            theme,
            theme_status,
        };
        state.header_copy = random_header_copy(&state.theme.header_quotes);
        state.rebuild_tree();
        state.select_matching_row();
        state.action_state.select(Some(0));
        crate::workflows::local::refresh_actions(&mut state);
        state
    }

    pub fn show_toast(&mut self, level: ToastLevel, message: impl Into<String>) {
        self.toast = Some(Toast::new(level, message));
    }

    pub fn clear_expired_toast(&mut self) {
        if self.toast.as_ref().is_some_and(|t| t.is_expired()) {
            self.toast = None;
        }
    }

    pub fn cycle_layout(&mut self) {
        let selected = self.selected.clone();
        let expanded = self.expanded.clone();
        let current = self.current.clone();
        let filter = self.filter.clone();
        let tab = self.inspector_tab;
        let log = self.log_lines.clone();

        self.layout = self.layout.cycle();
        self.config.config.layout = self.layout;
        self.config.mark_dirty();

        self.selected = selected;
        self.expanded = expanded;
        self.current = current;
        self.filter = filter;
        self.inspector_tab = tab;
        self.log_lines = log;
        self.rebuild_tree();
        self.select_matching_row();
        self.show_toast(ToastLevel::Info, format!("layout: {}", self.layout.label()));
    }

    pub fn rebuild_tree(&mut self) {
        let filter = self.filter.to_ascii_lowercase();
        let tag_filter = self.tag_filter.clone();
        let mut rows = Vec::new();
        for site in &self.sites {
            if !site_matches(site, &filter, tag_filter.as_deref()) {
                let envs = self.envs.get(&site.name).cloned().unwrap_or_default();
                if !envs.iter().any(|e| env_matches(e, &filter)) && !filter.is_empty() {
                    continue;
                }
                if tag_filter.is_some() && !site_matches(site, "", tag_filter.as_deref()) {
                    continue;
                }
            }
            let expanded = self.expanded.contains(&site.name);
            rows.push(TreeRow {
                depth: 0,
                site: site.name.clone(),
                env: None,
                expanded,
            });
            if expanded {
                if let Some(envs) = self.envs.get(&site.name) {
                    for env in envs {
                        if !filter.is_empty()
                            && !site_matches(site, &filter, None)
                            && !env_matches(env, &filter)
                        {
                            continue;
                        }
                        rows.push(TreeRow {
                            depth: 1,
                            site: site.name.clone(),
                            env: Some(env.id.clone()),
                            expanded: false,
                        });
                    }
                }
            }
        }
        self.tree_rows = rows;
        if self.tree_rows.is_empty() {
            self.tree_state.select(None);
        }
    }

    pub fn select_matching_row(&mut self) {
        let idx = self.tree_rows.iter().position(|row| match &self.selected {
            TreeSel::Site(site) => row.env.is_none() && row.site == *site,
            TreeSel::Env { site, env } => {
                row.site == *site && row.env.as_deref() == Some(env.as_str())
            }
            TreeSel::None => false,
        });
        if let Some(idx) = idx {
            self.tree_state.select(Some(idx));
        } else if !self.tree_rows.is_empty() {
            self.tree_state.select(Some(0));
            self.apply_row_selection(0);
        }
    }

    pub fn apply_row_selection(&mut self, idx: usize) {
        let Some(row) = self.tree_rows.get(idx) else {
            return;
        };
        self.selected = match &row.env {
            Some(env) => TreeSel::Env {
                site: row.site.clone(),
                env: env.clone(),
            },
            None => TreeSel::Site(row.site.clone()),
        };
        crate::workflows::local::on_selection_changed(self);
        if self.demo {
            self.sync_dummy_plan();
        } else {
            crate::workflows::inventory::on_selection_changed(self);
            crate::workflows::tags::on_selection_changed(self);
            crate::workflows::metrics::on_selection_changed(self);
            crate::workflows::backup::on_selection_changed(self);
        }
        self.persist_selection();
    }

    pub fn move_tree(&mut self, delta: isize) {
        if self.tree_rows.is_empty() {
            return;
        }
        let len = self.tree_rows.len() as isize;
        let cur = self.tree_state.selected().unwrap_or(0) as isize;
        let next = (cur + delta).clamp(0, len - 1) as usize;
        self.tree_state.select(Some(next));
        self.apply_row_selection(next);
    }

    pub fn jump_tree(&mut self, end: bool) {
        if self.tree_rows.is_empty() {
            return;
        }
        let idx = if end { self.tree_rows.len() - 1 } else { 0 };
        self.tree_state.select(Some(idx));
        self.apply_row_selection(idx);
    }

    pub fn toggle_expand(&mut self) {
        let Some(idx) = self.tree_state.selected() else {
            return;
        };
        let Some(row) = self.tree_rows.get(idx) else {
            return;
        };
        if row.env.is_some() {
            return;
        }
        let site = row.site.clone();
        if !self.expanded.remove(&site) {
            self.expanded.insert(site.clone());
            crate::workflows::inventory::on_expand(self, &site);
        }
        self.rebuild_tree();
        self.select_matching_row();
    }

    pub fn expand_or_select(&mut self) {
        let Some(idx) = self.tree_state.selected() else {
            return;
        };
        let Some(row) = self.tree_rows.get(idx).cloned() else {
            return;
        };
        if row.env.is_none() {
            self.expanded.insert(row.site.clone());
            crate::workflows::inventory::on_expand(self, &row.site);
            self.rebuild_tree();
            self.select_matching_row();
        }
    }

    pub fn move_actions(&mut self, delta: isize) {
        if self.actions.is_empty() {
            return;
        }
        let len = self.actions.len() as isize;
        let cur = self.action_state.selected().unwrap_or(0) as isize;
        let next = (cur + delta).clamp(0, len - 1) as usize;
        self.action_state.select(Some(next));
    }

    pub fn site(&self, name: &str) -> Option<&Site> {
        self.sites.iter().find(|s| s.name == name)
    }

    pub fn selected_site(&self) -> Option<&Site> {
        match &self.selected {
            TreeSel::Site(name) | TreeSel::Env { site: name, .. } => self.site(name),
            TreeSel::None => None,
        }
    }

    pub fn selected_env(&self) -> Option<&Env> {
        match &self.selected {
            TreeSel::Env { site, env } => self
                .envs
                .get(site)
                .and_then(|envs| envs.iter().find(|e| e.id == *env)),
            _ => None,
        }
    }

    pub fn selected_backups(&self) -> &[Backup] {
        match &self.selected {
            TreeSel::Env { site, env } => self
                .backups
                .get(&format!("{site}.{env}"))
                .map(|v| v.as_slice())
                .unwrap_or(&[]),
            _ => &[],
        }
    }

    pub fn selected_metrics(&self) -> Option<&MetricsSeries> {
        let env = self.selected_env()?;
        let key = (format!("{}.{}", env.site, env.id), self.metrics_period);
        self.metrics.get(&key)
    }

    pub fn sync_dummy_plan(&mut self) {
        let (site, env) = match &self.selected {
            TreeSel::Env { site, env } => (site.clone(), env.clone()),
            TreeSel::Site(site) => (site.clone(), "dev".into()),
            TreeSel::None => ("acme-wp".into(), "test".into()),
        };
        let mut plan = dummy_backup_plan(&site, &env);
        plan.binary = self.tools.terminus_path();
        plan.dry_run = true;
        self.current = Some(StagedPlan::One(plan));
    }

    pub fn selected_action_id(&self) -> Option<&'static str> {
        self.action_state
            .selected()
            .and_then(|i| self.actions.get(i).map(|a| a.id))
    }

    pub fn apply_doctor(&mut self, report: crate::doctor::DoctorReport) {
        self.tools = report.tools;
        self.tools_enabled = self.tools.terminus_ok();
        self.auth = report.auth;
        self.catalog = report.catalog;
        self.doctor_warnings = report.warnings;
        if let Some(StagedPlan::One(plan)) = &mut self.current {
            if plan.tool == ToolKind::Terminus {
                plan.binary = self.tools.terminus_path();
            }
        }
    }

    pub fn persist_selection(&mut self) {
        match &self.selected {
            TreeSel::Site(site) => {
                self.config.config.last_site = Some(site.clone());
                self.config.config.last_env = None;
            }
            TreeSel::Env { site, env } => {
                self.config.config.last_site = Some(site.clone());
                self.config.config.last_env = Some(env.clone());
            }
            TreeSel::None => {}
        }
        self.config.mark_dirty();
    }

    pub fn log_collapsed(&self) -> bool {
        self.layout == LayoutId::TabbedInspector
            && !self.job_running
            && self.focus != FocusPane::Log
    }

    pub fn local_root_hint(&self) -> Option<PathBuf> {
        self.selected_site()
            .and_then(|s| s.local.as_ref().map(|l| l.path.clone()))
    }
}

fn site_matches(site: &Site, filter: &str, tag: Option<&str>) -> bool {
    if let Some(tag) = tag {
        if !site.tags.iter().any(|t| t.name == tag) {
            return false;
        }
    }
    if filter.is_empty() {
        return true;
    }
    site.name.to_ascii_lowercase().contains(filter)
        || site
            .label
            .as_ref()
            .is_some_and(|l| l.to_ascii_lowercase().contains(filter))
        || site
            .tags
            .iter()
            .any(|t| t.name.to_ascii_lowercase().contains(filter))
}

fn env_matches(env: &Env, filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    env.id.to_ascii_lowercase().contains(filter)
        || env
            .domain
            .as_ref()
            .is_some_and(|d| d.to_ascii_lowercase().contains(filter))
}

pub fn random_header_copy(quotes: &[String]) -> String {
    if quotes.is_empty() {
        return "dd_pantheon".to_string();
    }
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as usize)
        .unwrap_or(0)
        ^ std::process::id() as usize;
    quotes[seed % quotes.len()].clone()
}
