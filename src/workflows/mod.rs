pub mod auth;
pub mod backup;
pub mod domains;
pub mod inventory;
pub mod metrics;
pub mod tags;

use crate::jobs::{self, JobKind};
use crate::plan::{PlanTarget, SafetyTier, StagedPlan};
use crate::safety;
use crate::state::{AppState, Modal, TreeSel};
use crate::toast::ToastLevel;

pub fn stage_action(state: &mut AppState, action_id: &str) -> bool {
    match action_id {
        "backup" | "b" => backup::stage_create(state),
        "cache" | "c" => domains::stage_clear_cache(state),
        "wake" => domains::stage_wake(state),
        "deploy" | "e" => stage_deploy(state),
        "lando-start" | "s" => stage_lando(state, "start"),
        "lando-stop" | "S" => stage_lando(state, "stop"),
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

fn env_target(state: &AppState) -> Option<PlanTarget> {
    match &state.selected {
        TreeSel::Env { site, env } => Some(PlanTarget::Env {
            site: site.clone(),
            env: env.clone(),
        }),
        TreeSel::Site(site) => state.envs.get(site).and_then(|envs| {
            envs.first().map(|e| PlanTarget::Env {
                site: site.clone(),
                env: e.id.clone(),
            })
        }),
        TreeSel::None => None,
    }
}

fn stage_deploy(state: &mut AppState) -> bool {
    let Some(target) = env_target(state) else {
        state.show_toast(ToastLevel::Warning, "select an environment");
        return false;
    };
    let site_env = target.label();
    let live = matches!(&target, PlanTarget::Env { env, .. } if env == "live");
    if live {
        state.show_toast(
            ToastLevel::Warning,
            "live deploy is LiveGate — type live in PR 3; dry preview only",
        );
    }
    let mut argv = vec!["env:deploy".into(), site_env.clone(), "--cc".into()];
    let mut safety_tier = if live {
        SafetyTier::LiveGate
    } else {
        SafetyTier::Mutating
    };
    if matches!(&target, PlanTarget::Env { env, .. } if env == "test") {
        argv.push("--sync-content".into());
        safety_tier = SafetyTier::Destructive;
        let mut steps = safety::backup_first(state.tools.terminus_path(), &target);
        let deploy = crate::tools::terminus::plan(
            state.tools.terminus_path(),
            argv,
            format!("deploy code to {site_env} (sync-content from live)"),
            safety_tier,
            target.clone(),
        );
        steps.push(deploy);
        let max_safety = steps
            .iter()
            .map(|s| s.safety)
            .max()
            .unwrap_or(SafetyTier::Mutating);
        state.current = Some(StagedPlan::Workflow {
            plan: crate::plan::WorkflowPlan {
                title: format!("deploy {site_env}"),
                why: format!("backup-first deploy to {site_env}"),
                safety: max_safety,
                steps,
                stop_on_failure: true,
            },
            step: 0,
        });
        return true;
    }
    let plan = crate::tools::terminus::plan(
        state.tools.terminus_path(),
        argv,
        format!("deploy code to {site_env}"),
        safety_tier,
        target,
    );
    state.current = Some(StagedPlan::One(plan));
    true
}

fn stage_lando(state: &mut AppState, cmd: &str) -> bool {
    let Some(site) = state.selected_site() else {
        state.show_toast(ToastLevel::Warning, "select a site");
        return false;
    };
    let Some(local) = site.local.as_ref() else {
        state.show_toast(ToastLevel::Warning, "no local path bound");
        return false;
    };
    let target = PlanTarget::Local {
        path: local.path.clone(),
        site: Some(site.name.clone()),
    };
    let plan = crate::tools::lando::plan(
        state.tools.lando_path(),
        vec![cmd.into()],
        format!("lando {cmd} in {}", local.path.display()),
        SafetyTier::Mutating,
        target,
    );
    state.current = Some(StagedPlan::One(plan));
    true
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
    for job in state.jobs.values() {
        if job.status.is_live() {
            jobs::request_cancel(job);
            any = true;
        }
    }
    if !any {
        state.show_toast(ToastLevel::Info, "no job to cancel");
    }
}
