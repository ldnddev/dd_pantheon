use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_pantheon::app::App;
use dd_pantheon::plan::{SafetyTier, StagedPlan};
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

#[test]
fn demo_skips_detect_and_b_stages_backup_plan() {
    let root = temp_path("dd_pantheon_pr2");
    fs::create_dir_all(&root).expect("root");
    let mut app = App::new_demo_in(&root, &root).expect("app");
    assert!(app.state.catalog.is_empty());
    assert!(!app.state.tools_enabled);

    app.handle_key(key(KeyCode::Char('b'))).unwrap();
    let plan = match app.state.current.as_ref() {
        Some(StagedPlan::One(p)) => p,
        other => panic!("expected one plan, got {other:?}"),
    };
    assert_eq!(plan.argv[0], "backup:create");
    assert_eq!(plan.safety, SafetyTier::Mutating);
    assert!(!plan.dry_run);
    assert!(plan.effective_argv().contains(&"--yes".to_string()));
    assert!(
        plan.effective_argv()
            .contains(&"--no-interaction".to_string())
    );
    let toast = app.state.toast.as_ref().expect("toast");
    assert!(toast.message.contains("demo: no spawn"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn deploy_test_is_backup_first_workflow() {
    let root = temp_path("dd_pantheon_pr2_deploy");
    fs::create_dir_all(&root).expect("root");
    let mut app = App::new_demo_in(&root, &root).expect("app");
    app.handle_key(key(KeyCode::Char('e'))).unwrap();
    match app.state.current.as_ref() {
        Some(StagedPlan::Workflow { plan, .. }) => {
            assert!(plan.steps.len() >= 3);
            assert_eq!(plan.steps[0].argv[0], "backup:create");
            assert_eq!(plan.steps[1].argv[0], "backup:list");
            assert_eq!(plan.steps[2].argv[0], "env:deploy");
            assert!(plan.steps[2].argv.iter().any(|a| a == "--sync-content"));
            assert_eq!(plan.safety, SafetyTier::Destructive);
        }
        other => panic!("expected workflow, got {other:?}"),
    }
    let _ = fs::remove_dir_all(root);
}
