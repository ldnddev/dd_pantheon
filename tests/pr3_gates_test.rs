use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_pantheon::app::App;
use dd_pantheon::state::{Modal, TreeSel};
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

fn app_not_demo() -> (App, PathBuf) {
    let root = temp_path("dd_pantheon_pr3");
    fs::create_dir_all(&root).expect("root");
    let mut app = App::new_demo_in(&root, &root).expect("app");
    app.state.demo = false;
    (app, root)
}

#[test]
fn deploy_test_opens_destructive_modal() {
    let (mut app, root) = app_not_demo();
    app.handle_key(key(KeyCode::Char('e'))).unwrap();
    app.handle_key(key(KeyCode::Enter)).unwrap();
    assert!(
        matches!(app.state.modal, Some(Modal::ConfirmDestructive { .. })),
        "{:?}",
        app.state.modal
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn deploy_live_opens_livegate_modal() {
    let (mut app, root) = app_not_demo();
    app.state.selected = TreeSel::Env {
        site: "acme-wp".into(),
        env: "live".into(),
    };
    app.handle_key(key(KeyCode::Char('e'))).unwrap();
    app.handle_key(key(KeyCode::Enter)).unwrap();
    match app.state.modal {
        Some(Modal::LiveGate { ref expected, .. }) => assert_eq!(expected, "live"),
        other => panic!("expected LiveGate, got {other:?}"),
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn demo_still_does_not_open_spawn_modals() {
    let root = temp_path("dd_pantheon_pr3_demo");
    fs::create_dir_all(&root).expect("root");
    let mut app = App::new_demo_in(&root, &root).expect("app");
    app.handle_key(key(KeyCode::Char('e'))).unwrap();
    assert!(matches!(app.state.modal, Some(Modal::DeployNote { .. })));
    app.handle_key(key(KeyCode::Enter)).unwrap();
    assert!(
        !matches!(
            app.state.modal,
            Some(Modal::ConfirmDestructive { .. }) | Some(Modal::LiveGate { .. })
        ),
        "{:?}",
        app.state.modal
    );
    let toast = app.state.toast.as_ref().expect("toast");
    assert!(toast.message.contains("demo: no spawn"));
    let _ = fs::remove_dir_all(root);
}
