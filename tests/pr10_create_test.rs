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
    let root = temp_path("dd_pantheon_pr10");
    fs::create_dir_all(&root).expect("root");
    let app = App::new_demo_in(&root, &root).expect("app");
    (app, root)
}

#[test]
fn demo_n_opens_create_wizard() {
    let (mut app, root) = demo_app();
    app.handle_key(key(KeyCode::Char('n'))).unwrap();
    match &app.state.modal {
        Some(Modal::SiteCreate { form }) => {
            assert!(!form.orgs.is_empty());
            assert!(!form.upstreams.is_empty());
            assert_eq!(form.focus, CreateField::Name);
        }
        other => panic!("expected SiteCreate, got {other:?}"),
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn demo_create_stages_site_create_with_org() {
    let (mut app, root) = demo_app();
    app.handle_key(key(KeyCode::Char('n'))).unwrap();
    for c in ['a', 'c', 'm', 'e', '-', 'n', 'e', 'w'] {
        app.handle_key(key(KeyCode::Char(c))).unwrap();
    }
    app.handle_key(key(KeyCode::Enter)).unwrap();
    let plan = match app.state.current.as_ref() {
        Some(StagedPlan::One(p)) => p,
        other => panic!("expected site:create plan, got {other:?}"),
    };
    assert_eq!(plan.argv[0], "site:create");
    assert_eq!(plan.argv[1], "acme-new");
    assert!(plan.argv.iter().any(|a| a.starts_with("--org=")));
    assert_eq!(plan.safety, SafetyTier::Mutating);
    let toast = app.state.toast.as_ref().expect("demo toast");
    assert!(toast.message.contains("demo: no spawn"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn demo_create_rejects_invalid_name() {
    let (mut app, root) = demo_app();
    app.handle_key(key(KeyCode::Char('n'))).unwrap();
    app.handle_key(key(KeyCode::Char('1'))).unwrap();
    app.handle_key(key(KeyCode::Enter)).unwrap();
    assert!(matches!(app.state.modal, Some(Modal::SiteCreate { .. })));
    let toast = app.state.toast.as_ref().expect("toast");
    assert!(toast.message.contains("site name"), "{}", toast.message);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn actions_include_create_site() {
    let (app, root) = demo_app();
    assert!(app.state.actions.iter().any(|a| a.id == "create"));
    let _ = fs::remove_dir_all(root);
}
