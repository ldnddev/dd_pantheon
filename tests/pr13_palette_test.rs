use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_pantheon::app::App;
use dd_pantheon::config::{looks_like_machine_token, push_history};
use dd_pantheon::plan::{SafetyTier, StagedPlan};
use dd_pantheon::state::{CmsKind, CmsTarget, Modal, TreeSel};
use dd_pantheon::workflows::palette::{PaletteArgs, RAW_PALETTE_WARNING, subsequence_match};
use dd_pantheon::workflows::{cms, plan_from_catalog};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_path(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("{}_{}_{}", prefix, std::process::id(), nanos))
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn demo_app() -> (App, PathBuf) {
    let root = temp_path("dd_pantheon_pr13");
    fs::create_dir_all(&root).expect("root");
    let app = App::new_demo_in(&root, &root).expect("app");
    (app, root)
}

#[test]
fn subsequence_matcher_is_in_order() {
    assert!(subsequence_match("env:deploy", "edp"));
    assert!(!subsequence_match("env:deploy", "pde"));
}

#[test]
fn plan_from_catalog_lando_pull_is_destructive() {
    let (app, root) = demo_app();
    let staged =
        plan_from_catalog(&app.state, "lando pull", &PaletteArgs::default()).expect("plan");
    let plan = staged.current().expect("current");
    assert_eq!(plan.argv[0], "pull");
    assert_eq!(plan.safety, SafetyTier::Destructive);
    assert!(plan.cwd.is_some());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn plan_from_catalog_lando_rebuild_is_destructive() {
    let (app, root) = demo_app();
    let staged =
        plan_from_catalog(&app.state, "lando rebuild", &PaletteArgs::default()).expect("plan");
    let plan = staged.current().expect("current");
    assert_eq!(plan.argv[0], "rebuild");
    assert_eq!(plan.safety, SafetyTier::Destructive);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn plan_from_catalog_env_wipe_keeps_backup_first() {
    let (app, root) = demo_app();
    match plan_from_catalog(&app.state, "env:wipe", &PaletteArgs::default()) {
        Ok(StagedPlan::Workflow { plan, .. }) => {
            assert_eq!(plan.steps[0].argv[0], "backup:create");
            assert_eq!(plan.steps.last().unwrap().argv[0], "env:wipe");
            assert_eq!(plan.safety, SafetyTier::Destructive);
            assert_eq!(plan.steps.last().unwrap().argv[1], "acme-wp.test");
        }
        other => panic!("expected wipe workflow, got {other:?}"),
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn plan_from_catalog_env_wipe_live_is_livegate() {
    let (mut app, root) = demo_app();
    app.state.selected = TreeSel::Env {
        site: "acme-wp".into(),
        env: "live".into(),
    };
    match plan_from_catalog(&app.state, "env:wipe", &PaletteArgs::default()) {
        Ok(StagedPlan::Workflow { plan, .. }) => {
            assert_eq!(plan.steps[0].argv[0], "backup:create");
            assert_eq!(plan.safety, SafetyTier::LiveGate);
            assert_eq!(plan.steps.last().unwrap().safety, SafetyTier::LiveGate);
        }
        other => panic!("expected live wipe workflow, got {other:?}"),
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn plan_from_catalog_env_deploy_keeps_test_guards() {
    let (app, root) = demo_app();
    match plan_from_catalog(&app.state, "env:deploy", &PaletteArgs::default()) {
        Ok(StagedPlan::Workflow { plan, .. }) => {
            assert_eq!(plan.steps[0].argv[0], "backup:create");
            let deploy = plan.steps.last().unwrap();
            assert_eq!(deploy.argv[0], "env:deploy");
            assert!(deploy.argv.iter().any(|a| a == "--sync-content"));
            assert_eq!(plan.safety, SafetyTier::Destructive);
        }
        other => panic!("expected backup-first deploy, got {other:?}"),
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn plan_from_catalog_env_deploy_live_is_livegate_without_sync() {
    let (mut app, root) = demo_app();
    app.state.selected = TreeSel::Env {
        site: "acme-wp".into(),
        env: "live".into(),
    };
    match plan_from_catalog(&app.state, "env:deploy", &PaletteArgs::default()) {
        Ok(StagedPlan::One(plan)) => {
            assert_eq!(plan.argv[0], "env:deploy");
            assert!(!plan.argv.iter().any(|a| a == "--sync-content"));
            assert_eq!(plan.safety, SafetyTier::LiveGate);
        }
        other => panic!("expected live deploy plan, got {other:?}"),
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn unrouted_palette_warns_and_is_not_a_bypass() {
    let (app, root) = demo_app();
    match plan_from_catalog(&app.state, "secret:set", &PaletteArgs::default()) {
        Ok(StagedPlan::One(plan)) => {
            assert!(plan.why.contains(RAW_PALETTE_WARNING));
            assert_eq!(plan.safety, SafetyTier::Mutating);
        }
        other => panic!("expected unrouted plan, got {other:?}"),
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn colon_opens_palette_and_submit_does_not_auto_run() {
    let (mut app, root) = demo_app();
    app.handle_key(key(KeyCode::Char(':'))).unwrap();
    assert!(matches!(
        app.state.modal,
        Some(Modal::Palette { form: None, .. })
    ));
    assert!(!app.state.catalog.is_empty());

    // Select env:wipe (seeded demo catalog) and stage without running.
    if let Some(Modal::Palette {
        query, selected, ..
    }) = &mut app.state.modal
    {
        *query = "env:wipe".into();
        *selected = 0;
    }
    app.handle_key(key(KeyCode::Enter)).unwrap();
    assert!(matches!(
        app.state.modal,
        Some(Modal::Palette { form: Some(_), .. })
    ));
    app.handle_key(key(KeyCode::Enter)).unwrap();
    assert!(app.state.modal.is_none());
    match app.state.current.as_ref() {
        Some(StagedPlan::Workflow { plan, .. }) => {
            assert_eq!(plan.steps.last().unwrap().argv[0], "env:wipe");
        }
        other => panic!("expected staged wipe, got {other:?}"),
    }
    assert!(
        !matches!(app.state.modal, Some(Modal::ConfirmDestructive { .. })),
        "palette must not auto-run / auto-confirm"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn m_opens_cms_form_not_month() {
    let (mut app, root) = demo_app();
    let before = app.state.metrics_period;
    app.handle_key(key(KeyCode::Char('m'))).unwrap();
    assert!(matches!(app.state.modal, Some(Modal::Cms { .. })));
    assert_eq!(app.state.metrics_period, before);
    if let Some(Modal::Cms { form }) = &app.state.modal {
        assert_eq!(form.target, CmsTarget::Local);
        assert_eq!(form.cms.label(), "wp");
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cms_sql_drop_is_destructive_and_does_not_put_yes_after_ddash() {
    let (app, root) = demo_app();
    let plan = cms::plan_cms(&app.state, CmsTarget::Remote, CmsKind::Wp, "sql-drop").expect("plan");
    assert_eq!(plan.safety, SafetyTier::Destructive);
    assert_eq!(plan.argv[0], "remote:wp");
    let argv = plan.effective_argv();
    let ddash = argv.iter().position(|a| a == "--").expect("--");
    assert!(
        argv.iter().position(|a| a == "--yes").expect("yes") < ddash,
        "terminus --yes must sit before --, got {argv:?}"
    );
    assert!(!argv[ddash + 1..].iter().any(|a| a == "--yes" || a == "-y"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn history_drops_machine_token_lines() {
    assert!(looks_like_machine_token("auth:login --machine-token=abc"));
    let mut list = vec![];
    push_history(&mut list, "auth:login --machine-token=abc");
    push_history(&mut list, "env:clear-cache");
    assert_eq!(list, vec!["env:clear-cache".to_string()]);
}
