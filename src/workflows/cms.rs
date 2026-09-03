use crate::config::push_history;
use crate::models::Framework;
use crate::plan::{CommandPlan, PlanTarget, SafetyTier, StagedPlan, ToolKind};
use crate::state::{AppState, CmsFocus, CmsForm, CmsKind, CmsTarget, Modal, TreeSel};
use crate::toast::ToastLevel;
use std::time::Duration;

const MUTATE_TIMEOUT: Duration = Duration::from_secs(600);

pub fn open(state: &mut AppState) {
    let cms = infer_cms(state);
    let target = if local_running(state) {
        CmsTarget::Local
    } else {
        CmsTarget::Remote
    };
    let mut history = state.config.config.history.cms.clone();
    if history.is_empty() {
        history = vec!["plugin list".into(), "core version".into(), "status".into()];
    }
    state.modal = Some(Modal::Cms {
        form: CmsForm {
            target,
            cms,
            command: String::new(),
            history,
            history_idx: None,
            focus: CmsFocus::Command,
        },
    });
}

fn infer_cms(state: &AppState) -> CmsKind {
    match state.selected_site().map(|s| s.framework) {
        Some(Framework::Drupal) => CmsKind::Drush,
        _ => CmsKind::Wp,
    }
}

fn local_running(state: &AppState) -> bool {
    state
        .selected_site()
        .and_then(|s| s.local.as_ref())
        .is_some_and(|l| l.running == Some(true))
}

pub fn cms_safety(command: &str) -> SafetyTier {
    let n = command.to_ascii_lowercase();
    if n.contains("sql-drop") || n.contains("site-install") {
        SafetyTier::Destructive
    } else {
        SafetyTier::Mutating
    }
}

pub fn plan_cms(
    state: &AppState,
    target: CmsTarget,
    cms: CmsKind,
    command: &str,
) -> Result<CommandPlan, String> {
    let cmd = command.trim();
    if cmd.is_empty() {
        return Err("enter a CMS command".into());
    }
    let argv_tail = shlex::split(cmd).ok_or_else(|| "could not parse CMS command".to_string())?;
    if argv_tail.is_empty() {
        return Err("enter a CMS command".into());
    }
    let safety = cms_safety(cmd);
    match target {
        CmsTarget::Remote => plan_remote(state, cms, argv_tail, cmd, safety),
        CmsTarget::Local => plan_local(state, cms, argv_tail, cmd, safety),
    }
}

fn plan_remote(
    state: &AppState,
    cms: CmsKind,
    tail: Vec<String>,
    cmd: &str,
    safety: SafetyTier,
) -> Result<CommandPlan, String> {
    let (site, env) = match &state.selected {
        TreeSel::Env { site, env } => (site.clone(), env.clone()),
        _ => return Err("select an environment".into()),
    };
    let site_env = format!("{site}.{env}");
    let bin = match cms {
        CmsKind::Wp => "remote:wp",
        CmsKind::Drush => "remote:drush",
    };
    let mut argv = vec![bin.into(), site_env.clone(), "--".into()];
    argv.extend(tail);
    let safety = if env == "live" && safety >= SafetyTier::Mutating {
        SafetyTier::LiveGate
    } else {
        safety
    };
    Ok(CommandPlan {
        tool: ToolKind::Terminus,
        binary: state.tools.terminus_path(),
        argv,
        cwd: None,
        why: format!("{bin} {site_env} -- {cmd}"),
        safety,
        target: PlanTarget::Env { site, env },
        dry_run: false,
        timeout: Some(MUTATE_TIMEOUT),
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: true,
    })
}

fn plan_local(
    state: &AppState,
    cms: CmsKind,
    tail: Vec<String>,
    cmd: &str,
    safety: SafetyTier,
) -> Result<CommandPlan, String> {
    let site = state
        .selected_site()
        .ok_or_else(|| "select a site with a local path".to_string())?;
    let local = site
        .local
        .as_ref()
        .ok_or_else(|| "no local path bound".to_string())?;
    let bin = match cms {
        CmsKind::Wp => "wp",
        CmsKind::Drush => "drush",
    };
    let mut argv = vec![bin.into()];
    argv.extend(tail);
    Ok(CommandPlan {
        tool: ToolKind::Lando,
        binary: state.tools.lando_path(),
        argv,
        cwd: Some(local.path.clone()),
        why: format!("lando {bin} {cmd}"),
        safety,
        target: PlanTarget::Local {
            path: local.path.clone(),
            site: Some(site.name.clone()),
        },
        dry_run: false,
        timeout: Some(MUTATE_TIMEOUT),
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: false,
    })
}

pub fn submit(state: &mut AppState, form: CmsForm) {
    match plan_cms(state, form.target, form.cms, &form.command) {
        Ok(plan) => {
            push_history(
                &mut state.config.config.history.cms,
                form.command.trim().to_string(),
            );
            state.config.mark_dirty();
            state.modal = None;
            state.current = Some(StagedPlan::One(plan));
            crate::workflows::request_run(state);
        }
        Err(msg) => {
            state.show_toast(ToastLevel::Warning, msg);
            state.modal = Some(Modal::Cms { form });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wipe_class_cms_is_destructive() {
        assert_eq!(cms_safety("sql-drop"), SafetyTier::Destructive);
        assert_eq!(cms_safety("site-install standard"), SafetyTier::Destructive);
        assert_eq!(cms_safety("plugin list"), SafetyTier::Mutating);
        assert_eq!(cms_safety("sql:cli"), SafetyTier::Mutating);
    }
}
