use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_pantheon::app::App;
use dd_pantheon::plan::StagedPlan;
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

#[test]
fn test_deploy_note_modal_custom_note() {
    let root = temp_path("dd_pantheon_pr18");
    fs::create_dir_all(&root).expect("root");
    let mut app = App::new_demo_in(&root, &root).expect("app");
    app.state.selected = TreeSel::Env {
        site: "acme-wp".into(),
        env: "test".into(),
    };
    app.handle_key(key(KeyCode::Char('e'))).unwrap();
    match &app.state.modal {
        Some(Modal::DeployNote {
            note,
            sync_content,
            env,
            ..
        }) => {
            assert_eq!(env, "test");
            assert!(*sync_content);
            assert_eq!(note, "Deploy from dd_pantheon");
        }
        other => panic!("expected DeployNote, got {other:?}"),
    }
    for _ in 0..24 {
        app.handle_key(key(KeyCode::Backspace)).unwrap();
    }
    for c in "hotfix".chars() {
        app.handle_key(key(KeyCode::Char(c))).unwrap();
    }
    app.handle_key(key(KeyCode::Enter)).unwrap();
    match app.state.current.as_ref() {
        Some(StagedPlan::Workflow { plan, .. }) => {
            let deploy = plan
                .steps
                .iter()
                .find(|s| s.argv.first().map(|a| a.as_str()) == Some("env:deploy"))
                .expect("deploy");
            assert!(deploy.argv.iter().any(|a| a == "--note=hotfix"));
            assert!(deploy.argv.iter().any(|a| a == "--sync-content"));
        }
        other => panic!("expected workflow, got {other:?}"),
    }
    let _ = fs::remove_dir_all(root);
}
