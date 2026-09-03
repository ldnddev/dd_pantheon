pub mod auth;
pub mod backup;
pub mod deploy;
pub mod domains;
pub mod inventory;
pub mod local;
pub mod metrics;
pub mod tags;

use crate::jobs::{self, JobKind};
use crate::plan::{SafetyTier, StagedPlan};
use crate::safety;
use crate::state::{AppState, Modal};
use crate::toast::ToastLevel;

pub fn stage_action(state: &mut AppState, action_id: &str) -> bool {
    match action_id {
        "backup" | "b" => backup::stage_create(state),
        "cache" | "c" => domains::stage_clear_cache(state),
        "wake" => domains::stage_wake(state),
        "deploy" | "e" => deploy::stage_deploy(state),
        "lando-start" | "s" => local::stage_start(state),
        "lando-stop" | "S" => local::stage_stop(state),
        "lando-restart" => local::stage_restart(state),
        "lando-logs" => local::stage_logs(state),
        "lando-rebuild" => local::stage_rebuild(state),
        "lando-destroy" => local::stage_destroy(state),
        "lando-pull" => local::stage_pull(state),
        "lando-push" => local::stage_push(state, false),
        "lando-push-db" => local::stage_push(state, true),
        "lando-poweroff" => local::stage_poweroff(state),
        "login" => {
            stage_login(state);
            false
        }
        "cms" | "m" => {
            state.show_toast(ToastLevel::Info, "CMS form lands in PR 13");
            false
        }
        "n" => {
            state.show_toast(ToastLevel::Info, "site create lands in PR 10");
            false
        }
        "a" => {
            tags::open_add(state);
            false
        }
        "r" => {
            inventory::refresh(state);
            false
        }
        other => {
            state.show_toast(ToastLevel::Warning, format!("unknown action: {other}"));
            false
        }
    }
}

fn stage_login(state: &mut AppState) {
    open_login(state);
}

pub fn open_login(state: &mut AppState) {
    if !state.tools.terminus_ok() {
        state.show_toast(ToastLevel::Error, "terminus not on PATH");
        return;
    }
    let env_available = std::env::var("TERMINUS_MACHINE_TOKEN")
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    if env_available {
        state.show_toast(ToastLevel::Info, "env token detected");
    }
    state.modal = Some(Modal::Login {
        token: String::new(),
        use_env_token: env_available,
        env_available,
    });
}

pub fn submit_login(state: &mut AppState, token: String) {
    if state.demo {
        maybe_demo_toast(state);
        return;
    }
    let plan = auth::plan_login(state.tools.terminus_path(), &token);
    state.current = Some(StagedPlan::One(plan.clone()));
    start_user_job(state, plan);
}

pub fn maybe_demo_toast(state: &mut AppState) {
    if state.demo {
        state.show_toast(crate::toast::ToastLevel::Warning, "demo: no spawn");
    }
}

pub fn request_run(state: &mut AppState) {
    if state.demo {
        maybe_demo_toast(state);
        return;
    }
    let Some(staged) = state.current.clone() else {
        state.show_toast(ToastLevel::Warning, "no plan staged");
        return;
    };
    let plan = match staged.current() {
        Some(p) => p.clone(),
        None => {
            state.show_toast(ToastLevel::Warning, "no plan staged");
            return;
        }
    };
    if plan.dry_run {
        state.show_toast(ToastLevel::Warning, "dry_run: no spawn");
        return;
    }
    match staged {
        StagedPlan::Workflow { plan: wf, step } => {
            let Some(plan) = wf.steps.get(step).cloned() else {
                return;
            };
            let total = wf.steps.len();
            let wf_safety = wf.safety;
            let staged = StagedPlan::Workflow { plan: wf, step };
            match wf_safety {
                SafetyTier::ReadOnly | SafetyTier::Mutating => {
                    state.pending_workflow = Some(staged);
                    start_job(state, plan, JobKind::Workflow { step, total });
                }
                SafetyTier::Destructive => {
                    state.pending_workflow = Some(staged);
                    state.modal = Some(Modal::ConfirmDestructive { plan });
                }
                SafetyTier::LiveGate => {
                    let expected = safety::live_gate_word(&plan).unwrap_or("live").to_string();
                    state.pending_workflow = Some(staged);
                    state.modal = Some(Modal::LiveGate {
                        plan,
                        expected,
                        typed: String::new(),
                    });
                }
            }
        }
        StagedPlan::One(_) => match plan.safety {
            SafetyTier::ReadOnly | SafetyTier::Mutating => start_user_job(state, plan),
            SafetyTier::Destructive => {
                state.modal = Some(Modal::ConfirmDestructive { plan });
            }
            SafetyTier::LiveGate => {
                let expected = safety::live_gate_word(&plan).unwrap_or("live").to_string();
                state.modal = Some(Modal::LiveGate {
                    plan,
                    expected,
                    typed: String::new(),
                });
            }
        },
    }
}

pub fn start_user_job(state: &mut AppState, plan: crate::plan::CommandPlan) {
    start_job(state, plan, JobKind::User);
}

pub fn start_job(
    state: &mut AppState,
    plan: crate::plan::CommandPlan,
    kind: JobKind,
) -> Option<crate::jobs::JobId> {
    if plan.dry_run {
        state.show_toast(ToastLevel::Warning, "dry_run: no spawn");
        return None;
    }
    let mutating = matches!(
        plan.safety,
        SafetyTier::Mutating | SafetyTier::Destructive | SafetyTier::LiveGate
    );
    if mutating {
        let slot = plan.target.mutating_slot();
        if state.mutating_slots.contains(&slot) {
            state.show_toast(
                ToastLevel::Warning,
                format!("a mutating job is already running on {slot}"),
            );
            return None;
        }
    }
    if mutating {
        let live = state
            .jobs
            .values()
            .filter(|j| {
                j.status.is_live()
                    && matches!(
                        j.plan.safety,
                        SafetyTier::Mutating | SafetyTier::Destructive | SafetyTier::LiveGate
                    )
            })
            .count();
        if live >= 4 {
            state.show_toast(ToastLevel::Warning, "job cap (4) reached");
            return None;
        }
    }
    let quiet = kind.quiet();
    match jobs::spawn(state.job_hub.tx.clone(), plan, kind) {
        Ok(job) => {
            if mutating {
                state.mutating_slots.insert(job.plan.target.mutating_slot());
            }
            state.job_running = true;
            if !quiet {
                state.log_lines.clear();
                state.log_scroll = 0;
            }
            let id = job.id;
            state.jobs.insert(id, job);
            Some(id)
        }
        Err(err) => {
            state.show_toast(ToastLevel::Error, format!("spawn failed: {err:#}"));
            None
        }
    }
}

pub fn start_doctor_jobs(state: &mut AppState) {
    if state.skip_detect {
        return;
    }
    if state
        .jobs
        .values()
        .any(|j| matches!(j.kind, JobKind::DoctorList | JobKind::DoctorWhoami))
    {
        return;
    }
    let which = crate::tools::detect_which();
    state.tools = which.clone();
    state.tools_enabled = state.tools.terminus_ok();
    if let Some(t) = &which.terminus {
        start_job(
            state,
            auth::plan_version(crate::plan::ToolKind::Terminus, t.path.clone()),
            JobKind::DoctorVersion {
                tool: crate::plan::ToolKind::Terminus,
            },
        );
        start_job(
            state,
            auth::plan_terminus_list(t.path.clone()),
            JobKind::DoctorList,
        );
        start_job(
            state,
            auth::plan_whoami(t.path.clone()),
            JobKind::DoctorWhoami,
        );
    } else {
        state.show_toast(
            ToastLevel::Error,
            "terminus not on PATH — inventory and login disabled",
        );
    }
    if let Some(t) = &which.lando {
        start_job(
            state,
            auth::plan_version(crate::plan::ToolKind::Lando, t.path.clone()),
            JobKind::DoctorVersion {
                tool: crate::plan::ToolKind::Lando,
            },
        );
    }
    if let Some(t) = &which.git {
        start_job(
            state,
            auth::plan_version(crate::plan::ToolKind::Git, t.path.clone()),
            JobKind::DoctorVersion {
                tool: crate::plan::ToolKind::Git,
            },
        );
    }
}

pub fn cancel_jobs(state: &mut AppState) {
    let mut any = false;
    let mut lando = false;
    for job in state.jobs.values() {
        if job.status.is_live() {
            if job.plan.tool == crate::plan::ToolKind::Lando {
                lando = true;
            }
            jobs::request_cancel(job);
            any = true;
        }
    }
    if !any {
        state.show_toast(ToastLevel::Info, "no job to cancel");
    } else if lando {
        state.show_toast(ToastLevel::Warning, "if containers still run: lando stop");
    }
}
