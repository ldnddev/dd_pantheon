use crate::catalog::parse_terminus_list;
use crate::config::ConfigStore;
use crate::fixtures::demo_data;
use crate::input::{handle_key, handle_mouse};
use crate::jobs::{JobEvent, JobKind, JobStatus, Stream};
use crate::plan::{PlanTarget, StagedPlan};
use crate::state::AppState;
use crate::theme::{ThemeStatus, load_theme};
use crate::toast::ToastLevel;
use crate::tools::{detect_which, parse_version_line};
use crate::ui;
use crate::workflows;
use crate::workflows::auth::auth_from_output;
use anyhow::Result;
use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::Frame;
use std::path::{Path, PathBuf};

pub struct LaunchOpts {
    pub demo: bool,
    pub root: Option<PathBuf>,
    pub project_root: PathBuf,
    pub config_dir: PathBuf,
    pub skip_detect: bool,
}

pub struct App {
    pub state: AppState,
}

impl App {
    pub fn new(opts: LaunchOpts) -> Result<Self> {
        let theme = load_theme(&opts.project_root);
        let config = ConfigStore::load(&opts.config_dir);
        let data = demo_data();
        let mut state = AppState::from_demo(theme, config, data);
        state.demo = opts.demo;
        state.skip_detect = opts.skip_detect;
        if !opts.demo {
            state.sites.clear();
            state.envs.clear();
            state.expanded.clear();
            state.selected = crate::state::TreeSel::None;
            state.current = None;
            state.log_lines.clear();
            state.metrics.clear();
            state.backups.clear();
            state.domains.clear();
            state.https.clear();
            state.locks.clear();
            state.rebuild_tree();
        }

        crate::debuglog::init(state.config.config.debug_log);
        tracing::info!(
            source = state.theme.source.label(),
            demo = state.demo,
            "theme source"
        );

        if let Some(warn) = state.config.load_warning.clone() {
            state.show_toast(ToastLevel::Warning, warn);
        }
        if let Some(warn) = state.theme.warning.clone() {
            state.theme_status = ThemeStatus::warning(warn.clone());
            state.show_toast(ToastLevel::Warning, warn);
        }

        if let Some(root) = &opts.root {
            workflows::local::bind_root(&mut state, root);
        }

        if !opts.skip_detect {
            state.tools = detect_which();
            state.tools_enabled = state.tools.terminus_ok();
            if !state.tools_enabled {
                state.show_toast(
                    ToastLevel::Error,
                    "terminus not on PATH — inventory and login disabled",
                );
            }
            if !opts.demo {
                workflows::start_doctor_jobs(&mut state);
            }
        }

        Ok(Self { state })
    }

    pub fn new_demo_in(project_root: &Path, config_dir: &Path) -> Result<Self> {
        Self::new(LaunchOpts {
            demo: true,
            root: None,
            project_root: project_root.to_path_buf(),
            config_dir: config_dir.to_path_buf(),
            skip_detect: true,
        })
    }

    pub fn draw(&mut self, f: &mut Frame) {
        ui::draw(f, &mut self.state);
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        handle_key(&mut self.state, key)
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent) -> Result<bool> {
        handle_mouse(&mut self.state, mouse)
    }

    pub fn tick(&mut self) {
        self.state.clear_expired_toast();
        self.state.config.flush_if_due();
        for _ in 0..64 {
            match self.state.job_hub.rx.try_recv() {
                Ok(ev) => apply_event(&mut self.state, ev),
                Err(_) => break,
            }
        }
        self.state.job_running = self.state.jobs.values().any(|j| j.status.is_live());
        crate::workflows::inventory::flush_debounce(&mut self.state);
        crate::workflows::tags::flush_debounce(&mut self.state);
        crate::workflows::metrics::flush_debounce(&mut self.state);
        crate::workflows::backup::flush_debounce(&mut self.state);
        crate::workflows::local::flush_debounce(&mut self.state);
        crate::workflows::domains::flush_debounce(&mut self.state);
    }

    pub fn save(&mut self) -> Result<()> {
        self.state.config.write_now()?;
        if self.state.config.sites_dirty_since.is_some() || self.state.config.sites_path.exists() {
            let _ = self.state.config.write_sites_now();
        }
        Ok(())
    }
}

fn apply_event(state: &mut AppState, ev: JobEvent) {
    match ev {
        JobEvent::Started { id, pid, pgid } => {
            if let Some(job) = state.jobs.get_mut(&id) {
                job.status = JobStatus::Running { pid, pgid };
                job.pgid.store(pgid, std::sync::atomic::Ordering::SeqCst);
                tracing::info!(
                    line = %job.plan.redacted_shell_line(),
                    pid,
                    pgid,
                    "job started"
                );
            }
        }
        JobEvent::Chunk { id, stream, text } => {
            if let Some(job) = state.jobs.get_mut(&id) {
                if stream == Stream::Stdout {
                    job.stdout_raw.push_str(&text);
                }
                let redacted = job.plan.redact_text(&text);
                job.log.push(&redacted);
                for line in redacted.lines() {
                    if !line.is_empty() {
                        state.log_lines.push(line.to_string());
                    }
                }
                if state.log_lines.len() > 4000 {
                    let extra = state.log_lines.len() - 4000;
                    state.log_lines.drain(..extra);
                }
            }
        }
        JobEvent::Exited {
            id,
            status,
            stdout_raw: _,
        } => {
            let (
                kind,
                stdout,
                plan_safety,
                slot,
                expects_json,
                is_login,
                is_logout,
                is_tag_mutate,
                is_backup_mutate,
                is_site_create,
                is_local_clone,
                env_mutate_site,
                edge_target,
                commit_target,
                connection_target,
            ) = if let Some(job) = state.jobs.get(&id) {
                tracing::info!(
                    line = %job.plan.redacted_shell_line(),
                    status = ?status,
                    "job exit"
                );
                let cmd = job.plan.argv.first().map(|a| a.as_str());
                let connection_target = if cmd == Some("connection:set") {
                    job.plan.argv.get(1).cloned()
                } else {
                    None
                };
                let env_mutate_site = match cmd {
                    Some(
                        "multidev:create"
                        | "multidev:delete"
                        | "multidev:merge-to-dev"
                        | "env:wipe"
                        | "env:clone-content",
                    ) => match &job.plan.target {
                        PlanTarget::Env { site, .. } | PlanTarget::Site { site } => {
                            Some(site.clone())
                        }
                        _ => None,
                    },
                    _ => None,
                };
                let edge_target = match cmd {
                    Some(
                        "domain:add" | "domain:remove" | "https:set" | "lock:enable"
                        | "lock:disable",
                    ) => match &job.plan.target {
                        PlanTarget::Env { site, env } => Some((site.clone(), env.clone())),
                        _ => None,
                    },
                    _ => None,
                };
                let commit_target = if cmd == Some("env:commit") {
                    match &job.plan.target {
                        PlanTarget::Env { site, env } => Some((site.clone(), env.clone())),
                        _ => None,
                    }
                } else {
                    None
                };
                (
                    job.kind.clone(),
                    job.stdout_raw.clone(),
                    job.plan.safety,
                    job.plan.target.mutating_slot(),
                    job.plan.expects_json,
                    cmd == Some("auth:login"),
                    cmd == Some("auth:logout"),
                    matches!(cmd, Some("tag:add" | "tag:remove" | "tag:rm")),
                    matches!(cmd, Some("backup:create" | "backup:restore")),
                    cmd == Some("site:create"),
                    cmd == Some("local:clone"),
                    env_mutate_site,
                    edge_target,
                    commit_target,
                    connection_target,
                )
            } else {
                return;
            };
            if let Some(job) = state.jobs.get_mut(&id) {
                job.status = status.clone();
                if expects_json {
                    if let JobStatus::Succeeded { .. } = &status {
                        match serde_json::from_str(&job.stdout_raw) {
                            Ok(v) => job.json = Some(v),
                            Err(err) => {
                                if !kind.quiet() {
                                    state.show_toast(
                                        ToastLevel::Error,
                                        format!("JSON parse failed: {err}"),
                                    );
                                }
                            }
                        }
                    }
                }
            }
            if matches!(
                plan_safety,
                crate::plan::SafetyTier::Mutating
                    | crate::plan::SafetyTier::Destructive
                    | crate::plan::SafetyTier::LiveGate
            ) {
                state.mutating_slots.remove(&slot);
            }
            crate::workflows::inventory::clear_inflight(state, &kind);
            crate::workflows::metrics::clear_inflight(state, &kind);
            crate::workflows::backup::clear_inflight(state, &kind);
            crate::workflows::local::clear_inflight(state, &kind);
            crate::workflows::create::clear_inflight(state, &kind);
            crate::workflows::domains::clear_inflight(state, &kind);
            match &status {
                JobStatus::Succeeded { .. } => {
                    if !kind.quiet() {
                        state.show_toast(ToastLevel::Success, "job ok");
                    }
                    apply_doctor_result(state, &kind, &stdout, 0, "");
                    apply_inventory_result(state, &kind, &stdout);
                    if is_login || is_logout {
                        if let Some(t) = state.tools.terminus.clone() {
                            workflows::start_job(
                                state,
                                crate::workflows::auth::plan_whoami(t.path),
                                JobKind::DoctorWhoami,
                            );
                        }
                    }
                    if is_tag_mutate {
                        crate::workflows::tags::on_tag_mutate_done(state);
                    }
                    if is_backup_mutate {
                        crate::workflows::backup::refresh_selected(state);
                    }
                    if is_site_create {
                        crate::workflows::create::on_created(state);
                    }
                    if is_local_clone {
                        crate::workflows::create::on_cloned(state);
                    }
                    if let Some(site) = &env_mutate_site {
                        crate::workflows::multidev::on_env_mutate(state, site);
                    }
                    if let Some((site, env)) = &edge_target {
                        crate::workflows::domains::on_edge_mutate(state, site, env);
                    }
                    if let Some((site, env)) = &commit_target {
                        crate::workflows::deploy::on_commit_done(state, site, env);
                    }
                    if let Some(target) = connection_target.as_deref() {
                        if let Some((site, env)) = target.split_once('.') {
                            crate::workflows::deploy::on_connection_set_done(state, site, env);
                        }
                    }
                    advance_workflow(state);
                }
                JobStatus::Failed { err, exit } => {
                    if let JobKind::Metrics { .. } = &kind {
                        state.metrics_error = Some(err.clone());
                    } else if kind.quiet() {
                        if matches!(kind, JobKind::DoctorWhoami) {
                            apply_doctor_result(state, &kind, &stdout, exit.unwrap_or(1), "");
                        }
                    } else {
                        state.show_toast(ToastLevel::Error, format!("job failed: {err}"));
                    }
                }
                JobStatus::Cancelled => {
                    if matches!(kind, JobKind::Metrics { .. }) {
                        state.metrics_error = Some("metrics cancelled".into());
                    } else {
                        state.show_toast(ToastLevel::Warning, "job cancelled");
                    }
                }
                JobStatus::TimedOut => {
                    if matches!(kind, JobKind::Metrics { .. }) {
                        state.metrics_error = Some("metrics timed out".into());
                    } else {
                        state.show_toast(ToastLevel::Error, "job timed out");
                    }
                }
                _ => {}
            }
        }
    }
}

fn apply_doctor_result(
    state: &mut AppState,
    kind: &JobKind,
    stdout: &str,
    exit: i32,
    stderr: &str,
) {
    match kind {
        JobKind::DoctorWhoami => {
            state.auth = auth_from_output(exit, stdout, stderr);
            crate::workflows::local::refresh_actions(state);
            if matches!(state.auth, crate::doctor::AuthState::LoggedIn { .. }) {
                crate::workflows::inventory::request_site_list(state, true);
            }
        }
        JobKind::DoctorList => match parse_terminus_list(stdout) {
            Ok(entries) => {
                // Replace terminus entries, keep any lando extras already merged.
                state
                    .catalog
                    .retain(|e| e.tool != crate::plan::ToolKind::Terminus);
                state.catalog.extend(entries);
                tracing::info!(n = state.catalog.len(), "catalog size");
            }
            Err(err) => state
                .doctor_warnings
                .push(format!("terminus list JSON parse failed: {err:#}")),
        },
        JobKind::DoctorVersion { tool } => {
            let ver = parse_version_line(stdout);
            if ver.is_empty() {
                return;
            }
            let slot = match tool {
                crate::plan::ToolKind::Terminus => &mut state.tools.terminus,
                crate::plan::ToolKind::Lando => &mut state.tools.lando,
                crate::plan::ToolKind::Git => &mut state.tools.git,
            };
            if let Some(bin) = slot {
                bin.version = Some(ver);
            }
        }
        _ => {}
    }
}

fn apply_inventory_result(state: &mut AppState, kind: &JobKind, stdout: &str) {
    match kind {
        JobKind::SiteList => crate::workflows::inventory::apply_site_list(state, stdout),
        JobKind::EnvList { site } => {
            crate::workflows::inventory::apply_env_list(state, site, stdout)
        }
        JobKind::EnvInfo { site, env } => {
            crate::workflows::inventory::apply_env_info(state, site, env, stdout)
        }
        JobKind::OrgList { site } => crate::workflows::tags::apply_org_list(state, site, stdout),
        JobKind::TagList { site, org } => {
            crate::workflows::tags::apply_tag_list(state, site, org, stdout)
        }
        JobKind::Metrics { site, env, period } => {
            crate::workflows::metrics::apply_metrics(state, site, env, period, stdout)
        }
        JobKind::BackupList { site, env } => {
            crate::workflows::backup::apply_list(state, site, env, stdout)
        }
        JobKind::Diffstat { site, env } => {
            crate::workflows::deploy::apply_diffstat(state, site, env, stdout)
        }
        JobKind::LandoList => crate::workflows::local::apply_list(state, stdout),
        JobKind::LandoInfo { site } => crate::workflows::local::apply_info(state, site, stdout),
        JobKind::CreateOrgList => crate::workflows::create::apply_org_catalog(state, stdout),
        JobKind::CreateUpstreamList => crate::workflows::create::apply_upstream_list(state, stdout),
        JobKind::DomainList { site, env } => {
            crate::workflows::domains::apply_domains(state, site, env, stdout)
        }
        JobKind::HttpsInfo { site, env } => {
            crate::workflows::domains::apply_https(state, site, env, stdout)
        }
        JobKind::LockInfo { site, env } => {
            crate::workflows::domains::apply_lock(state, site, env, stdout)
        }
        _ => {}
    }
}

fn advance_workflow(state: &mut AppState) {
    let Some(StagedPlan::Workflow { plan, step }) = state.pending_workflow.clone() else {
        return;
    };
    let next = step + 1;
    if next >= plan.steps.len() {
        state.pending_workflow = None;
        return;
    }
    if plan.stop_on_failure {
        // caller only advances on success
    }
    let next_plan = plan.steps[next].clone();
    let total = plan.steps.len();
    state.pending_workflow = Some(StagedPlan::Workflow { plan, step: next });
    if let Some(StagedPlan::Workflow { step, .. }) = &mut state.current {
        *step = next;
    }
    workflows::start_job(state, next_plan, JobKind::Workflow { step: next, total });
}
