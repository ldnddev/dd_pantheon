use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_pantheon::app::App;
use dd_pantheon::plan::{SafetyTier, StagedPlan};
use dd_pantheon::state::{CreateField, Modal};
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
    let root = temp_path("dd_pantheon_pr16");
    fs::create_dir_all(&root).expect("root");
    let app = App::new_demo_in(&root, &root).expect("app");
    (app, root)
}

#[test]
fn bind_local_stages_create_then_local_clone() {
    let (mut app, root) = demo_app();
    app.handle_key(key(KeyCode::Char('n'))).unwrap();
    for c in ['a', 'c', 'm', 'e', '-', 'n', 'e', 'w'] {
        app.handle_key(key(KeyCode::Char(c))).unwrap();
    }
    for _ in 0..4 {
        app.handle_key(key(KeyCode::Tab)).unwrap();
    }
    match &app.state.modal {
        Some(Modal::SiteCreate { form }) => assert_eq!(form.focus, CreateField::Bind),
        other => panic!("expected bind field, got {other:?}"),
    }
    app.handle_key(key(KeyCode::Char(' '))).unwrap();
    app.handle_key(key(KeyCode::Tab)).unwrap();
    let dest = root.join("acme-new");
    for c in dest.display().to_string().chars() {
        app.handle_key(key(KeyCode::Char(c))).unwrap();
    }
    app.handle_key(key(KeyCode::Enter)).unwrap();
    match app.state.current.as_ref() {
        Some(StagedPlan::Workflow { plan, .. }) => {
            assert_eq!(plan.steps[0].argv[0], "site:create");
            assert_eq!(plan.steps[1].argv[0], "local:clone");
            assert!(
                plan.steps[1]
                    .argv
                    .iter()
                    .any(|a| a.starts_with("--site_dir="))
            );
            assert_eq!(plan.safety, SafetyTier::Mutating);
            assert!(plan.steps[1].cwd.is_none());
        }
        other => panic!("expected create+clone workflow, got {other:?}"),
    }
    let toast = app.state.toast.as_ref().expect("demo toast");
    assert!(toast.message.contains("demo: no spawn"));
    let _ = fs::remove_dir_all(root);
}
