use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_pantheon::app::App;
use dd_pantheon::plan::{SafetyTier, StagedPlan};
use dd_pantheon::state::{BackupPickKind, Modal};
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
    let root = temp_path("dd_pantheon_pr15");
    fs::create_dir_all(&root).expect("root");
    let app = App::new_demo_in(&root, &root).expect("app");
    (app, root)
}

fn pick_action(app: &mut App, id: &str) {
    dd_pantheon::workflows::local::refresh_actions(&mut app.state);
    let found = app.state.actions.iter().any(|a| a.id == id);
    assert!(found, "missing action {id}");
    dd_pantheon::workflows::stage_action(&mut app.state, id);
}

#[test]
fn restore_and_get_actions_on_env_with_backups() {
    let (app, root) = demo_app();
    assert!(app.state.actions.iter().any(|a| a.id == "backup-restore"));
    assert!(app.state.actions.iter().any(|a| a.id == "backup-get"));
    assert!(!app.state.selected_backups().is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn restore_test_is_backup_first_destructive() {
    let (mut app, root) = demo_app();
    app.state.demo = false;
    pick_action(&mut app, "backup-restore");
    assert!(matches!(
        app.state.modal,
        Some(Modal::BackupPick {
            kind: BackupPickKind::Restore,
            ..
        })
    ));
    app.handle_key(key(KeyCode::Enter)).unwrap();
    match app.state.current.as_ref() {
        Some(StagedPlan::Workflow { plan, .. }) => {
            assert_eq!(plan.steps[0].argv[0], "backup:create");
            assert_eq!(plan.steps.last().unwrap().argv[0], "backup:restore");
            assert!(
                plan.steps
                    .last()
                    .unwrap()
                    .argv
                    .iter()
                    .any(|a| a.starts_with("--file="))
            );
            assert_eq!(plan.safety, SafetyTier::Destructive);
        }
        other => panic!("expected restore workflow, got {other:?}"),
    }
    assert!(matches!(
        app.state.modal,
        Some(Modal::ConfirmDestructive { .. })
    ));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn get_does_not_write_to_cwd_and_is_readonly() {
    let (mut app, root) = demo_app();
    pick_action(&mut app, "backup-get");
    assert!(matches!(
        app.state.modal,
        Some(Modal::BackupPick {
            kind: BackupPickKind::Get,
            ..
        })
    ));
    app.handle_key(key(KeyCode::Enter)).unwrap();
    let plan = match app.state.current.as_ref() {
        Some(StagedPlan::One(p)) => p,
        other => panic!("expected get plan, got {other:?}"),
    };
    assert_eq!(plan.argv[0], "backup:get");
    assert_eq!(plan.safety, SafetyTier::ReadOnly);
    assert!(!plan.argv.iter().any(|a| a.starts_with("--to")));
    assert!(plan.cwd.is_none());
    let toast = app.state.toast.as_ref().expect("demo toast");
    assert!(toast.message.contains("demo: no spawn"));
    let _ = fs::remove_dir_all(root);
}
