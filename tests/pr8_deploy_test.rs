use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_pantheon::app::App;
use dd_pantheon::plan::{SafetyTier, StagedPlan};
use dd_pantheon::state::{Modal, TreeSel};
use dd_pantheon::workflows::deploy;
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
    let root = temp_path("dd_pantheon_pr8");
    fs::create_dir_all(&root).expect("root");
    let app = App::new_demo_in(&root, &root).expect("app");
    (app, root)
}

#[test]
fn demo_dev_stages_git_push_and_wait() {
    let (mut app, root) = demo_app();
    app.state.selected = TreeSel::Env {
        site: "acme-wp".into(),
        env: "dev".into(),
    };
    app.handle_key(key(KeyCode::Char('e'))).unwrap();
    match app.state.current.as_ref() {
        Some(StagedPlan::Workflow { plan, .. }) => {
            assert_eq!(plan.steps[0].argv[0], "push");
            assert_eq!(plan.steps[0].argv[1], "origin");
            assert_eq!(plan.steps[0].argv[2], "master");
            assert!(plan.steps[0].cwd.is_some());
            assert!(!plan.steps[0].dry_run);
            assert_eq!(plan.steps[1].argv[0], "workflow:wait");
            assert!(plan.steps[1].argv.iter().any(|a| a == "--max=600"));
            assert!(!plan.steps.iter().any(|s| s.argv[0] == "connection:set"));
        }
        other => panic!("expected git-mode workflow, got {other:?}"),
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn demo_sftp_env_stages_connection_set() {
    let (mut app, root) = demo_app();
    app.state.selected = TreeSel::Env {
        site: "acme-wp".into(),
        env: "feat-x".into(),
    };
    app.handle_key(key(KeyCode::Char('e'))).unwrap();
    match app.state.current.as_ref() {
        Some(StagedPlan::Workflow { plan, .. }) => {
            assert_eq!(plan.steps[0].argv[0], "connection:set");
            assert_eq!(plan.steps[0].argv[2], "git");
            assert_eq!(plan.steps[1].argv[0], "push");
            assert_eq!(plan.steps[2].argv[0], "workflow:wait");
        }
        other => panic!("expected connection:set workflow, got {other:?}"),
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn dirty_diffstat_opens_modal_and_does_not_stage_connection_set() {
    let (mut app, root) = demo_app();
    app.state.selected = TreeSel::Env {
        site: "acme-wp".into(),
        env: "feat-x".into(),
    };
    let before = app
        .state
        .current
        .as_ref()
        .and_then(|p| p.current().map(|c| c.argv.clone()));
    deploy::apply_diffstat(
        &mut app.state,
        "acme-wp",
        "feat-x",
        r#"[{"file":"index.php","status":"M"}]"#,
    );
    match &app.state.modal {
        Some(Modal::DiffstatDirty { files, env, .. }) => {
            assert_eq!(env, "feat-x");
            assert_eq!(files, &vec!["index.php".to_string()]);
        }
        other => panic!("expected DiffstatDirty, got {other:?}"),
    }
    let after = app
        .state
        .current
        .as_ref()
        .and_then(|p| p.current().map(|c| c.argv.clone()));
    assert_eq!(before, after);
    assert!(
        after
            .as_ref()
            .is_none_or(|argv| argv.first().is_none_or(|a| a != "connection:set"))
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn live_deploy_has_note_and_no_sync_content() {
    let (mut app, root) = demo_app();
    app.state.selected = TreeSel::Env {
        site: "acme-wp".into(),
        env: "live".into(),
    };
    app.handle_key(key(KeyCode::Char('e'))).unwrap();
    app.handle_key(key(KeyCode::Enter)).unwrap();
    let plan = match app.state.current.as_ref() {
        Some(StagedPlan::One(p)) => p,
        other => panic!("expected live deploy plan, got {other:?}"),
    };
    assert_eq!(plan.argv[0], "env:deploy");
    assert_eq!(plan.safety, SafetyTier::LiveGate);
    assert!(!plan.argv.iter().any(|a| a == "--sync-content"));
    assert!(
        plan.argv
            .iter()
            .any(|a| a == "--note=Deploy from dd_pantheon")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn drupal_test_deploy_includes_updatedb() {
    let (mut app, root) = demo_app();
    app.state.selected = TreeSel::Env {
        site: "acme-d10".into(),
        env: "test".into(),
    };
    app.handle_key(key(KeyCode::Char('e'))).unwrap();
    app.handle_key(key(KeyCode::Enter)).unwrap();
    match app.state.current.as_ref() {
        Some(StagedPlan::Workflow { plan, .. }) => {
            let deploy = plan
                .steps
                .iter()
                .find(|s| s.argv.first().map(|a| a.as_str()) == Some("env:deploy"))
                .expect("deploy step");
            assert!(deploy.argv.iter().any(|a| a == "--updatedb"));
            assert!(deploy.argv.iter().any(|a| a == "--sync-content"));
        }
        other => panic!("expected workflow, got {other:?}"),
    }
    let _ = fs::remove_dir_all(root);
}
