use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_pantheon::app::App;
use dd_pantheon::plan::{SafetyTier, StagedPlan};
use dd_pantheon::state::{FocusPane, TreeSel};
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
    let root = temp_path("dd_pantheon_pr7");
    fs::create_dir_all(&root).expect("root");
    let app = App::new_demo_in(&root, &root).expect("app");
    (app, root)
}

#[test]
fn demo_c_stages_clear_cache_preview_c_is_noop() {
    let (mut app, root) = demo_app();
    app.handle_key(key(KeyCode::Char('c'))).unwrap();
    let plan = match app.state.current.as_ref() {
        Some(StagedPlan::One(p)) => p,
        other => panic!("expected clear-cache plan, got {other:?}"),
    };
    assert_eq!(plan.argv[0], "env:clear-cache");
    assert_eq!(plan.argv[1], "acme-wp.test");
    assert_eq!(plan.safety, SafetyTier::Mutating);
    assert!(plan.effective_argv().contains(&"--yes".to_string()));
    let toast = app.state.toast.as_ref().expect("demo toast");
    assert!(toast.message.contains("demo: no spawn"));

    app.state.toast = None;
    app.state.focus = FocusPane::Preview;
    app.handle_key(key(KeyCode::Char('c'))).unwrap();
    let plan = match app.state.current.as_ref() {
        Some(StagedPlan::One(p)) => p,
        other => panic!("preview c must not restage, got {other:?}"),
    };
    assert_eq!(plan.argv[0], "env:clear-cache");
    assert!(app.state.toast.is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn demo_wake_from_actions() {
    let (mut app, root) = demo_app();
    let idx = app
        .state
        .actions
        .iter()
        .position(|a| a.id == "wake")
        .expect("wake action");
    app.state.action_state.select(Some(idx));
    app.state.focus = FocusPane::Inspector;
    app.handle_key(key(KeyCode::Enter)).unwrap();
    let plan = match app.state.current.as_ref() {
        Some(StagedPlan::One(p)) => p,
        other => panic!("expected wake plan, got {other:?}"),
    };
    assert_eq!(plan.argv[0], "env:wake");
    assert_eq!(plan.safety, SafetyTier::ReadOnly);
    assert!(!plan.confirm_with_yes);
    let toast = app.state.toast.as_ref().expect("demo toast");
    assert!(toast.message.contains("demo: no spawn"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn demo_b_requires_env_row() {
    let (mut app, root) = demo_app();
    app.state.selected = TreeSel::Site("acme-wp".into());
    app.handle_key(key(KeyCode::Char('b'))).unwrap();
    let toast = app.state.toast.as_ref().expect("toast");
    assert!(
        toast.message.contains("select an environment"),
        "{}",
        toast.message
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn demo_inspector_has_backup_rows() {
    let (app, root) = demo_app();
    let rows = app.state.selected_backups();
    assert!(!rows.is_empty());
    assert!(rows.iter().any(|b| b.file.contains("acme-wp_test")));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn second_mutating_job_on_same_slot_is_refused() {
    let (mut app, root) = demo_app();
    app.state.demo = false;
    app.state.mutating_slots.insert("acme-wp.test".into());
    app.handle_key(key(KeyCode::Char('b'))).unwrap();
    let toast = app.state.toast.as_ref().expect("slot toast");
    assert!(
        toast.message.contains("mutating job is already running"),
        "{}",
        toast.message
    );
    assert!(app.state.jobs.is_empty());
    let _ = fs::remove_dir_all(root);
}
