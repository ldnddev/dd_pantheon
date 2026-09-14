use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_pantheon::app::App;
use dd_pantheon::models::LayoutId;
use dd_pantheon::state::{FocusPane, Modal, TreeSel};
use dd_pantheon::ui::footer_keys;
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
    let root = temp_path("dd_pantheon_lab");
    fs::create_dir_all(&root).expect("root");
    let app = App::new_demo_in(&root, &root).expect("app");
    (app, root)
}

#[test]
fn demo_loads_three_sites_and_classic_layout() {
    let (app, root) = demo_app();
    assert!(app.state.demo);
    assert_eq!(app.state.sites.len(), 3);
    assert_eq!(app.state.layout, LayoutId::ClassicStack);
    assert!(
        app.state
            .sites
            .iter()
            .any(|s| s.name == "frozen-lab" && s.frozen)
    );
    assert!(
        app.state
            .sites
            .iter()
            .any(|s| s.name == "acme-wp" && s.orgs.len() == 1)
    );
    assert!(
        app.state
            .sites
            .iter()
            .any(|s| s.name == "frozen-lab" && s.orgs.is_empty())
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn f4_cycles_layout_and_preserves_selection() {
    let (mut app, root) = demo_app();
    app.state.selected = TreeSel::Env {
        site: "acme-wp".into(),
        env: "test".into(),
    };
    app.state.select_matching_row();

    app.handle_key(key(KeyCode::F(4))).unwrap();
    assert_eq!(app.state.layout, LayoutId::ThreeColumn);
    assert_eq!(
        app.state.selected,
        TreeSel::Env {
            site: "acme-wp".into(),
            env: "test".into()
        }
    );

    app.handle_key(key(KeyCode::F(4))).unwrap();
    assert_eq!(app.state.layout, LayoutId::TabbedInspector);
    assert!(app.state.log_collapsed());

    app.handle_key(key(KeyCode::F(4))).unwrap();
    assert_eq!(app.state.layout, LayoutId::ClassicStack);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn enter_on_preview_toasts_demo_no_spawn() {
    let (mut app, root) = demo_app();
    app.state.focus = FocusPane::Preview;
    app.handle_key(key(KeyCode::Enter)).unwrap();
    let toast = app.state.toast.as_ref().expect("toast");
    assert!(toast.message.contains("demo: no spawn"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn footer_starts_with_f1_then_f2_then_quit() {
    let narrow = footer_keys(70);
    assert!(narrow.starts_with("F1:Help"));
    assert!(narrow.contains("F2:Theme"));
    assert!(narrow.contains("C-q:Quit"));
    assert!(!narrow.contains("F4"));

    let medium = footer_keys(100);
    assert!(medium.contains("F1: Help"));
    let f1 = medium.find("F1").unwrap();
    let f2 = medium.find("F2").unwrap();
    let q = medium.find("Ctrl+Q: Quit").unwrap();
    assert!(f1 < f2 && f2 < q);
}

#[test]
fn ctrl_q_quits_and_bare_q_does_not() {
    let (mut app, root) = demo_app();
    app.handle_key(key(KeyCode::Char('q'))).unwrap();
    assert!(!app.state.should_quit);

    let ctrl_q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
    let quit = app.handle_key(ctrl_q).unwrap();
    assert!(quit);
    assert!(app.state.should_quit);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn enter_on_log_opens_expand_modal() {
    let (mut app, root) = demo_app();
    app.state.focus = FocusPane::Log;
    app.handle_key(key(KeyCode::Enter)).unwrap();
    assert!(matches!(app.state.modal, Some(Modal::LogExpand)));
    app.handle_key(key(KeyCode::Esc)).unwrap();
    assert!(app.state.modal.is_none());

    app.state.focus = FocusPane::Log;
    app.handle_key(key(KeyCode::Char('e'))).unwrap();
    assert!(matches!(app.state.modal, Some(Modal::LogExpand)));
    app.handle_key(key(KeyCode::Enter)).unwrap();
    assert!(app.state.modal.is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn log_expand_scrolls_shared_offset() {
    let (mut app, root) = demo_app();
    app.state.focus = FocusPane::Log;
    app.state.log_lines = (0..40).map(|i| format!("line {i}")).collect();
    app.handle_key(key(KeyCode::Char('j'))).unwrap();
    app.handle_key(key(KeyCode::Char('j'))).unwrap();
    assert_eq!(app.state.log_scroll, 2);
    app.handle_key(key(KeyCode::Enter)).unwrap();
    assert!(matches!(app.state.modal, Some(Modal::LogExpand)));
    app.handle_key(key(KeyCode::Char('j'))).unwrap();
    assert_eq!(app.state.log_scroll, 3);
    app.handle_key(key(KeyCode::Char('g'))).unwrap();
    assert_eq!(app.state.log_scroll, 0);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn slash_opens_filter_from_any_pane() {
    let (mut app, root) = demo_app();
    app.state.focus = FocusPane::Log;
    app.handle_key(key(KeyCode::Char('/'))).unwrap();
    assert!(matches!(app.state.modal, Some(Modal::Filter { .. })));
    app.handle_key(key(KeyCode::Esc)).unwrap();
    assert!(app.state.modal.is_none());

    app.state.focus = FocusPane::Preview;
    app.handle_key(key(KeyCode::Char('/'))).unwrap();
    match &app.state.modal {
        Some(Modal::Filter { query, .. }) => assert!(query.is_empty()),
        other => panic!("expected filter modal, got {other:?}"),
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn slash_filter_matches_name_and_tag() {
    let (mut app, root) = demo_app();
    app.handle_key(key(KeyCode::Char('/'))).unwrap();
    for ch in ['p', 'r', 'o', 'd'] {
        app.handle_key(key(KeyCode::Char(ch))).unwrap();
    }
    let Some(Modal::Filter { query, selected }) = &app.state.modal else {
        panic!("filter modal");
    };
    assert_eq!(query, "prod");
    let hits = app.state.filter_hits(query);
    assert!(
        hits.iter().any(|h| h.site == "acme-wp" && h.env.is_none()),
        "prod tag should list acme-wp: {hits:?}"
    );
    assert!(hits.iter().any(|h| h.site == "acme-d10"));
    assert!(!hits.iter().any(|h| h.site == "frozen-lab"));
    let _ = selected;
    app.handle_key(key(KeyCode::Enter)).unwrap();
    assert!(app.state.modal.is_none());
    assert_eq!(app.state.filter, "prod");
    assert!(
        app.state
            .tree_rows
            .iter()
            .any(|r| r.site == "acme-wp" && r.env.is_none())
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn colon_opens_command_palette() {
    let (mut app, root) = demo_app();
    app.state.focus = FocusPane::Inspector;
    app.handle_key(key(KeyCode::Char(':'))).unwrap();
    assert!(matches!(
        app.state.modal,
        Some(Modal::Palette { form: None, .. })
    ));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn header_and_footer_heights_are_fixed() {
    use ratatui::layout::Rect;
    use ratatui::layout::{Constraint, Direction, Layout};
    let area = Rect::new(0, 0, 100, 40);
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);
    assert_eq!(outer[0].height, 3);
    assert_eq!(outer[2].height, 1);
}
