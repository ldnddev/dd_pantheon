use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_pantheon::app::{App, LaunchOpts};
use dd_pantheon::plan::{SafetyTier, StagedPlan};
use dd_pantheon::state::Modal;
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
    let root = temp_path("dd_pantheon_pr9");
    fs::create_dir_all(&root).expect("root");
    let app = App::new_demo_in(&root, &root).expect("app");
    (app, root)
}

#[test]
fn demo_s_stages_lando_start() {
    let (mut app, root) = demo_app();
    app.handle_key(key(KeyCode::Char('s'))).unwrap();
    let plan = match app.state.current.as_ref() {
        Some(StagedPlan::One(p)) => p,
        other => panic!("expected start plan, got {other:?}"),
    };
    assert_eq!(plan.argv[0], "start");
    assert_eq!(plan.safety, SafetyTier::Mutating);
    assert!(plan.cwd.is_some());
    assert!(plan.timeout.is_none());
    let toast = app.state.toast.as_ref().expect("demo toast");
    assert!(toast.message.contains("demo: no spawn"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn demo_push_is_code_only() {
    let (mut app, root) = demo_app();
    assert!(
        app.state.actions.iter().any(|a| a.id == "lando-push"),
        "pantheon recipe should expose push"
    );
    dd_pantheon::workflows::stage_action(&mut app.state, "lando-push");
    dd_pantheon::workflows::request_run(&mut app.state);
    let plan = match app.state.current.as_ref() {
        Some(StagedPlan::One(p)) => p,
        other => panic!("expected push plan, got {other:?}"),
    };
    assert_eq!(plan.argv[0], "push");
    assert!(plan.argv.iter().any(|a| a == "--database=none"));
    assert_eq!(plan.safety, SafetyTier::Mutating);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn push_db_opens_livegate_database() {
    let (mut app, root) = demo_app();
    app.state.demo = false;
    assert!(app.state.actions.iter().any(|a| a.id == "lando-push-db"));
    dd_pantheon::workflows::stage_action(&mut app.state, "lando-push-db");
    dd_pantheon::workflows::request_run(&mut app.state);
    match app.state.modal {
        Some(Modal::LiveGate { ref expected, .. }) => assert_eq!(expected, "database"),
        other => panic!("expected LiveGate database, got {other:?}"),
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebuild_opens_destructive_modal() {
    let (mut app, root) = demo_app();
    app.state.demo = false;
    assert!(app.state.actions.iter().any(|a| a.id == "lando-rebuild"));
    dd_pantheon::workflows::stage_action(&mut app.state, "lando-rebuild");
    dd_pantheon::workflows::request_run(&mut app.state);
    assert!(
        matches!(app.state.modal, Some(Modal::ConfirmDestructive { .. })),
        "{:?}",
        app.state.modal
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn root_flag_peeks_lando_yml() {
    let root = temp_path("dd_pantheon_pr9_root");
    fs::create_dir_all(&root).expect("root");
    fs::write(
        root.join(".lando.yml"),
        "name: boundapp\nrecipe: pantheon\nconfig:\n  framework: wordpress\n  site: acme-wp\n",
    )
    .unwrap();
    let mut app = App::new(LaunchOpts {
        demo: true,
        root: Some(root.clone()),
        project_root: root.clone(),
        config_dir: root.clone(),
        skip_detect: true,
    })
    .expect("app");
    let local = app
        .state
        .sites
        .iter()
        .find(|s| s.name == "acme-wp")
        .and_then(|s| s.local.as_ref())
        .expect("bound local");
    assert_eq!(local.path, root);
    assert_eq!(local.lando_name.as_deref(), Some("boundapp"));
    assert_eq!(local.recipe.as_deref(), Some("pantheon"));
    app.handle_key(key(KeyCode::Char('s'))).unwrap();
    let plan = match app.state.current.as_ref() {
        Some(StagedPlan::One(p)) => p,
        other => panic!("{other:?}"),
    };
    assert_eq!(plan.cwd.as_ref(), Some(&root));
    let _ = fs::remove_dir_all(root);
}
