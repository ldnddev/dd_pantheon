use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_pantheon::app::App;
use dd_pantheon::plan::{SafetyTier, StagedPlan};
use dd_pantheon::state::Modal;
use dd_pantheon::workflows::domains;
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
    let root = temp_path("dd_pantheon_pr12");
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
fn demo_inspector_shows_domains_without_lock_password() {
    let (app, root) = demo_app();
    let domains = app.state.selected_domains();
    assert!(domains.iter().any(|d| d.id.contains("example.com")));
    let lock = app.state.selected_lock();
    assert!(lock.is_some());
    assert!(lock.unwrap().username != Some("s3cret-pass".into()));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn add_domain_opens_modal_and_stages() {
    let (mut app, root) = demo_app();
    pick_action(&mut app, "domain-add");
    assert!(matches!(app.state.modal, Some(Modal::DomainAdd { .. })));
    for c in "new.example.com".chars() {
        app.handle_key(key(KeyCode::Char(c))).unwrap();
    }
    app.handle_key(key(KeyCode::Enter)).unwrap();
    let plan = match app.state.current.as_ref() {
        Some(StagedPlan::One(p)) => p,
        other => panic!("expected domain:add, got {other:?}"),
    };
    assert_eq!(plan.argv[0], "domain:add");
    assert_eq!(plan.argv[2], "new.example.com");
    assert_eq!(plan.safety, SafetyTier::Mutating);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn remove_domain_is_destructive() {
    let (mut app, root) = demo_app();
    pick_action(&mut app, "domain-remove");
    match &app.state.modal {
        Some(Modal::DomainRemove { domains, .. }) => {
            assert!(domains.iter().any(|d| d == "staging.example.com"));
        }
        other => panic!("expected DomainRemove, got {other:?}"),
    }
    app.state.demo = false;
    app.handle_key(key(KeyCode::Enter)).unwrap();
    assert!(matches!(
        app.state.modal,
        Some(Modal::ConfirmDestructive { .. })
    ));
    let plan = match app.state.current.as_ref() {
        Some(StagedPlan::One(p)) => p,
        other => panic!("{other:?}"),
    };
    assert_eq!(plan.argv[0], "domain:remove");
    assert_eq!(plan.safety, SafetyTier::Destructive);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn lock_enable_redacts_password_in_preview() {
    let (mut app, root) = demo_app();
    pick_action(&mut app, "lock-enable");
    for c in "ops".chars() {
        app.handle_key(key(KeyCode::Char(c))).unwrap();
    }
    app.handle_key(key(KeyCode::Tab)).unwrap();
    for c in "s3cret-pass".chars() {
        app.handle_key(key(KeyCode::Char(c))).unwrap();
    }
    app.handle_key(key(KeyCode::Enter)).unwrap();
    let plan = match app.state.current.as_ref() {
        Some(StagedPlan::One(p)) => p,
        other => panic!("{other:?}"),
    };
    assert_eq!(plan.argv[0], "lock:enable");
    let line = plan.redacted_shell_line();
    assert!(!line.contains("s3cret-pass"), "{line}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn lock_info_parser_never_keeps_password() {
    let info =
        domains::parse_lock_info(r#"{"locked":true,"username":"ops","password":"s3cret-pass"}"#)
            .unwrap();
    assert!(info.locked);
    assert_eq!(info.username.as_deref(), Some("ops"));
}
