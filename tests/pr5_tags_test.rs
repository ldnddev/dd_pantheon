use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_pantheon::app::App;
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

#[test]
fn demo_t_opens_tag_picker_and_again_clears() {
    let root = temp_path("dd_pantheon_pr5");
    fs::create_dir_all(&root).expect("root");
    let mut app = App::new_demo_in(&root, &root).expect("app");
    app.handle_key(key(KeyCode::Char('T'))).unwrap();
    match &app.state.modal {
        Some(Modal::TagPicker { tags, .. }) => {
            assert!(tags.iter().any(|t| t == "prod"));
        }
        other => panic!("expected TagPicker, got {other:?}"),
    }
    let first = match &app.state.modal {
        Some(Modal::TagPicker { tags, selected, .. }) => tags[*selected].clone(),
        _ => unreachable!(),
    };
    app.handle_key(key(KeyCode::Enter)).unwrap();
    assert_eq!(app.state.tag_filter.as_deref(), Some(first.as_str()));
    app.handle_key(key(KeyCode::Char('T'))).unwrap();
    assert!(app.state.tag_filter.is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn demo_a_opens_add_modal() {
    let root = temp_path("dd_pantheon_pr5_add");
    fs::create_dir_all(&root).expect("root");
    let mut app = App::new_demo_in(&root, &root).expect("app");
    app.handle_key(key(KeyCode::Char('a'))).unwrap();
    assert!(matches!(app.state.modal, Some(Modal::TagAdd { .. })));
    let _ = fs::remove_dir_all(root);
}
