use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_pantheon::app::App;
use dd_pantheon::models::LayoutId;
use dd_pantheon::state::{FocusPane, TreeSel};
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
