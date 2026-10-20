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

    /// Drain jobs, timers, and toasts. Returns whether the UI should repaint.
    pub fn tick(&mut self) -> bool {
        let mut dirty = self.state.clear_expired_toast();

        let warning_before = self.state.config.load_warning.clone();
        self.state.config.flush_if_due();
        if self.state.config.load_warning != warning_before {
            if let Some(warn) = self.state.config.load_warning.clone() {
                self.state.show_toast(ToastLevel::Warning, warn);
            }
            dirty = true;
        }

        let mut got_job = false;
        for _ in 0..64 {
            match self.state.job_hub.rx.try_recv() {
                Ok(ev) => {
                    apply_event(&mut self.state, ev);
                    got_job = true;
                }
                Err(_) => break,
            }
        }
        if got_job {
            dirty = true;
        }

        let running = self.state.jobs.values().any(|j| j.status.is_live());
        if running != self.state.job_running {
            self.state.job_running = running;
            dirty = true;
        }

        let frame = crate::ui::loader::frame_index(&self.state);
        if frame != self.state.loader_frame {
            self.state.loader_frame = frame;
            dirty = true;
        }

        let jobs_before = self.state.jobs.len();
        crate::workflows::inventory::flush_debounce(&mut self.state);
        crate::workflows::tags::flush_debounce(&mut self.state);
        crate::workflows::metrics::flush_debounce(&mut self.state);
        crate::workflows::backup::flush_debounce(&mut self.state);
        crate::workflows::local::flush_debounce(&mut self.state);
        crate::workflows::domains::flush_debounce(&mut self.state);
        if self.state.jobs.len() != jobs_before {
            self.state.job_running = self.state.jobs.values().any(|j| j.status.is_live());
            dirty = true;
        }

        dirty
    }

    /// How long the event loop can sleep before a timer (toast, loader, debounce) is due.
    /// `None` means block on input — nothing in the UI is animating.
    pub fn next_poll_timeout(&self) -> Option<std::time::Duration> {
        use std::time::{Duration, Instant};
        let mut wait: Option<Duration> = None;
        let push = |wait: &mut Option<Duration>, d: Duration| {
            *wait = Some(wait.map_or(d, |w| w.min(d)));
        };
        let since = |at: Instant, debounce: Duration| debounce.saturating_sub(at.elapsed());

        if self.state.job_running {
            push(&mut wait, Duration::from_millis(50));
            if let Some(rem) = crate::ui::loader::next_frame_in(&self.state) {
                push(&mut wait, rem);
            }
        }
        if let Some(toast) = &self.state.toast {
            push(
                &mut wait,
                toast.duration.saturating_sub(toast.created_at.elapsed()),
            );
        }
        if let Some((_, at)) = &self.state.pending_env_list {
            push(&mut wait, since(*at, crate::workflows::inventory::DEBOUNCE));
        }
        if let Some((_, _, at)) = &self.state.pending_env_info {
            push(&mut wait, since(*at, crate::workflows::inventory::DEBOUNCE));
        }
        if let Some((_, at)) = &self.state.pending_org_list {
            push(&mut wait, since(*at, crate::workflows::tags::DEBOUNCE));
        }
        if let Some((_, _, at)) = &self.state.pending_tag_list {
            push(&mut wait, since(*at, crate::workflows::tags::DEBOUNCE));
        }
        if let Some((_, _, at)) = &self.state.pending_metrics {
            push(&mut wait, since(*at, crate::workflows::metrics::DEBOUNCE));
        }
        if let Some((_, _, at)) = &self.state.pending_backup_list {
            push(&mut wait, since(*at, crate::workflows::backup::DEBOUNCE));
        }
        if let Some((_, _, at)) = &self.state.pending_edge {
            push(&mut wait, since(*at, crate::workflows::domains::DEBOUNCE));
        }
        if let Some(at) = &self.state.pending_lando {
            push(&mut wait, since(*at, crate::workflows::local::DEBOUNCE));
        }
        if let Some(at) = self.state.config.dirty_since {
            push(&mut wait, since(at, crate::config::CONFIG_WRITE_DEBOUNCE));
        }
        if let Some(at) = self.state.config.sites_dirty_since {
            push(&mut wait, since(at, crate::config::CONFIG_WRITE_DEBOUNCE));
        }
        wait
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
            append_job_chunk(state, id, stream, &text);
        }
        JobEvent::Exited {
            id,
            status,
            stdout_raw: _,
        } => {
            flush_job_pending(state, id);
            let (
                kind,
                mut stdout,
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
            scrub_retained_secrets(state, id, &mut stdout);
        }
    }
}

fn append_job_chunk(state: &mut AppState, id: crate::jobs::JobId, stream: Stream, text: &str) {
    let ready = {
        let Some(job) = state.jobs.get_mut(&id) else {
            return;
        };
        if stream == Stream::Stdout {
            job.stdout_raw.push_str(text);
        }
        match stream {
            Stream::Stdout => job.stdout_pending.push_chunk(text),
            Stream::Stderr => job.stderr_pending.push_chunk(text),
        }
    };
    commit_raw_output(state, id, ready);
}

fn flush_job_pending(state: &mut AppState, id: crate::jobs::JobId) {
    let (stdout, stderr) = {
        let Some(job) = state.jobs.get_mut(&id) else {
            return;
        };
        (job.stdout_pending.finish(), job.stderr_pending.finish())
    };
    commit_raw_output(state, id, stdout);
    commit_raw_output(state, id, stderr);
}

fn commit_raw_output(state: &mut AppState, id: crate::jobs::JobId, mut raw: String) {
    if raw.is_empty() {
        return;
    }
    let redacted = {
        let Some(job) = state.jobs.get_mut(&id) else {
            crate::plan::wipe_secret(&mut raw);
            return;
        };
        let redacted = job.plan.redact_text(&raw);
        job.log.push(&redacted);
        redacted
    };
    crate::plan::wipe_secret(&mut raw);
    for line in redacted.lines() {
        if !line.is_empty() {
            state.log_lines.push(line.to_string());
        }
    }
    if state.log_lines.len() > 4000 {
        let extra = state.log_lines.len() - 4000;
        state.log_lines.drain(..extra);
    }
    state.snap_log_follow();
}

fn scrub_retained_secrets(state: &mut AppState, id: crate::jobs::JobId, stdout: &mut String) {
    let identity = state.jobs.get(&id).map(|job| {
        (
            job.plan.binary.clone(),
            job.plan.argv.clone(),
            job.plan.cwd.clone(),
        )
    });
    if let Some((binary, mut argv, cwd)) = identity {
        scrub_staged_if_same(state.current.as_mut(), &binary, &argv, &cwd);
        scrub_staged_if_same(state.pending_workflow.as_mut(), &binary, &argv, &cwd);
        for arg in &mut argv {
            crate::plan::wipe_secret(arg);
        }
    }
    if let Some(job) = state.jobs.get_mut(&id) {
        crate::jobs::scrub_finished_job(job);
    }
    crate::plan::wipe_secret(stdout);
}

fn scrub_staged_if_same(
    staged: Option<&mut StagedPlan>,
    binary: &Path,
    argv: &[String],
    cwd: &Option<PathBuf>,
) {
    let Some(plan) = staged.and_then(StagedPlan::current_mut) else {
        return;
    };
    if plan.binary.as_path() == binary && plan.argv == argv && plan.cwd == *cwd {
        plan.scrub_secrets();
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
                crate::catalog::ensure_lando_catalog(&mut state.catalog);
                tracing::info!(n = state.catalog.len(), "catalog size");
            }
            Err(err) => state
                .doctor_warnings
                .push(format!("terminus list JSON parse failed: {err:#}")),
        },
        JobKind::DoctorLandoHelp => {
            crate::catalog::ensure_lando_catalog(&mut state.catalog);
            for entry in crate::catalog::parse_lando_help(stdout) {
                if !state.catalog.iter().any(|e| e.name == entry.name) {
                    state.catalog.push(entry);
                }
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn demo() -> App {
        let dir = std::env::temp_dir().join(format!(
            "dd_pantheon_tick_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::create_dir_all(&dir);
        App::new_demo_in(&dir, &dir).expect("demo app")
    }

    fn idle(app: &mut App) {
        app.state.toast = None;
        app.state.config.dirty_since = None;
        app.state.config.sites_dirty_since = None;
        app.state.job_running = false;
        app.state.loader_frame = None;
    }

    #[test]
    fn idle_tick_is_clean_and_has_no_wake() {
        let mut app = demo();
        idle(&mut app);
        let _ = app.tick();
        idle(&mut app);
        assert!(!app.tick(), "second idle tick must not request a redraw");
        assert!(
            app.next_poll_timeout().is_none(),
            "idle cockpit blocks on input"
        );
    }

    #[test]
    fn expired_toast_dirties_and_clears() {
        let mut app = demo();
        idle(&mut app);
        app.state.show_toast(ToastLevel::Info, "x");
        if let Some(t) = app.state.toast.as_mut() {
            t.duration = Duration::ZERO;
        }
        assert!(app.tick());
        assert!(app.state.toast.is_none());
    }

    #[test]
    fn toast_schedules_a_wake() {
        let mut app = demo();
        idle(&mut app);
        app.state.show_toast(ToastLevel::Info, "x");
        let wait = app.next_poll_timeout().expect("toast wake");
        assert!(wait <= crate::toast::TOAST_DURATION);
        assert!(wait > Duration::ZERO);
    }

    #[test]
    fn login_job_scrubs_staged_copy_and_finished_output() {
        let mut app = demo();
        let token = "super-secret-token";
        let plan = crate::workflows::auth::plan_login(std::path::PathBuf::from("/bin/echo"), token);
        let later = crate::workflows::auth::plan_login(
            std::path::PathBuf::from("/bin/true"),
            "other-secret-token",
        );
        app.state.current = Some(StagedPlan::Workflow {
            plan: crate::plan::WorkflowPlan {
                title: "login".into(),
                why: "login".into(),
                safety: plan.safety,
                steps: vec![plan.clone(), later],
                stop_on_failure: true,
            },
            step: 0,
        });
        app.state.pending_workflow = app.state.current.clone();
        crate::workflows::start_job(&mut app.state, plan, crate::jobs::JobKind::User);
        let staged = app.state.current.as_ref().expect("staged");
        let StagedPlan::Workflow { plan: steps, .. } = staged else {
            panic!("workflow");
        };
        assert!(
            !steps.steps[0].argv.iter().any(|a| a.contains(token)),
            "staged step was scrubbed after spawn"
        );
        assert!(
            steps.steps[1]
                .argv
                .iter()
                .any(|a| a.contains("other-secret-token")),
            "a later step still has to run"
        );
        let running = app
            .state
            .jobs
            .values()
            .find(|job| job.plan.binary == std::path::PathBuf::from("/bin/echo"))
            .expect("echo job");
        assert!(
            running.plan.argv.iter().any(|a| a.contains(token)),
            "the running job keeps the token so output can be redacted"
        );

        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            app.tick();
            if app.state.jobs.values().all(|job| !job.status.is_live()) {
                app.tick();
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let echo = app
            .state
            .jobs
            .values()
            .find(|job| job.plan.binary == std::path::PathBuf::from("/bin/echo"))
            .expect("echo job");
        assert!(!echo.status.is_live(), "{:?}", echo.status);
        assert!(!echo.plan.argv.iter().any(|arg| arg.contains(token)));
        assert!(!echo.stdout_raw.contains(token), "{}", echo.stdout_raw);
        assert!(!echo.log.text().contains(token), "{}", echo.log.text());
        assert!(
            !app.state.log_lines.iter().any(|line| line.contains(token)),
            "{:?}",
            app.state.log_lines
        );
    }

    #[test]
    fn split_chunks_redact_before_the_log_line_is_shown() {
        use crate::jobs::{Job, JobId, JobKind, JobStatus, LineHold, LogBuffer};
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, AtomicI32};

        let mut app = demo();
        let token = "super-secret-token";
        let plan = crate::workflows::auth::plan_login(std::path::PathBuf::from("/bin/echo"), token);
        let id = JobId::new();
        app.state.jobs.insert(
            id,
            Job {
                id,
                kind: JobKind::User,
                plan,
                status: JobStatus::Running { pid: 1, pgid: 1 },
                started_at: std::time::Instant::now(),
                log: LogBuffer::default(),
                stdout_raw: String::new(),
                stdout_pending: LineHold::default(),
                stderr_pending: LineHold::default(),
                json: None,
                cancel: Arc::new(AtomicBool::new(false)),
                pgid: Arc::new(AtomicI32::new(0)),
            },
        );
        app.state.log_lines.clear();
        append_job_chunk(&mut app.state, id, Stream::Stdout, "hello super-se");
        append_job_chunk(&mut app.state, id, Stream::Stderr, "tail super-se");
        assert!(
            app.state.log_lines.is_empty(),
            "partial output stays buffered: {:?}",
            app.state.log_lines
        );
        append_job_chunk(&mut app.state, id, Stream::Stdout, "cret-token\n");
        append_job_chunk(&mut app.state, id, Stream::Stderr, "cret-token");
        flush_job_pending(&mut app.state, id);
        assert!(
            app.state
                .log_lines
                .iter()
                .any(|line| line.contains("hello") && line.contains("***")),
            "{:?}",
            app.state.log_lines
        );
        assert!(
            app.state
                .log_lines
                .iter()
                .any(|line| line.starts_with("tail ") && line.contains("***")),
            "{:?}",
            app.state.log_lines
        );
        assert!(
            app.state.log_lines.iter().all(|line| !line.contains(token)),
            "{:?}",
            app.state.log_lines
        );
    }
}
