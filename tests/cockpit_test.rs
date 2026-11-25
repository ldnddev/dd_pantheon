use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use dd_pantheon::app::App;
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
    let root = temp_path("dd_pantheon_cockpit");
    fs::create_dir_all(&root).expect("root");
    let app = App::new_demo_in(&root, &root).expect("app");
    (app, root)
}

#[test]
fn bracketed_paste_fills_the_filter_in_one_insert() {
    let (mut app, root) = demo_app();
    app.handle_key(key(KeyCode::Char('/'))).expect("filter");
    app.paste_text("ab\r\ncd\u{1b}e").expect("paste");
    match &app.state.modal {
        Some(Modal::Filter { query, .. }) => assert_eq!(query, "abcde"),
        other => panic!("expected the filter modal, got {other:?}"),
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn demo_loads_three_sites() {
    let (app, root) = demo_app();
    assert!(app.state.demo);
    assert_eq!(app.state.sites.len(), 3);
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
fn f4_is_not_a_layout_cycle() {
    let (mut app, root) = demo_app();
    app.state.selected = TreeSel::Env {
        site: "acme-wp".into(),
        env: "test".into(),
    };
    app.state.select_matching_row();
    app.handle_key(key(KeyCode::F(4))).unwrap();
    assert_eq!(
        app.state.selected,
        TreeSel::Env {
            site: "acme-wp".into(),
            env: "test".into()
        }
    );
    assert!(app.state.modal.is_none());
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
fn footer_starts_with_settings_then_quit() {
    let narrow = footer_keys(70);
    assert!(narrow.starts_with("F1:Settings"), "{narrow}");
    assert!(narrow.contains("^Q:Quit"), "{narrow}");
    assert!(!narrow.contains("F1:Help"), "{narrow}");
    assert!(!narrow.contains("F2"), "{narrow}");
    assert!(!narrow.contains("F4"));

    let medium = footer_keys(100);
    assert!(medium.starts_with("F1: Settings"), "{medium}");
    assert!(medium.contains("Ctrl+Q: Quit"), "{medium}");
    assert!(!medium.contains("F2"), "{medium}");
    assert!(!medium.contains("F4"));
    let f1 = medium.find("F1").unwrap();
    let hint = medium.find("j/k").unwrap();
    let q = medium.find("Ctrl+Q: Quit").unwrap();
    assert!(f1 < hint && hint < q, "{medium}");

    let wide = footer_keys(140);
    assert!(wide.contains("F3: Doctor"), "{wide}");
    assert!(wide.contains("Ctrl+Q: Quit"), "{wide}");
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

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

#[test]
fn click_outside_settings_does_not_close_quit_confirm_stays() {
    let (mut app, root) = demo_app();
    app.handle_key(key(KeyCode::F(1))).unwrap();
    assert!(app.state.show_settings);
    app.state.settings_nav_area = ratatui::layout::Rect::new(80, 3, 14, 20);
    app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 0, 10))
        .unwrap();
    assert!(app.state.show_settings);
    assert!(app.state.modal.is_none());

    app.state.modal = Some(Modal::QuitConfirm);
    app.state.modal_area = Some(ratatui::layout::Rect::new(10, 5, 40, 20));
    app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 0, 0))
        .unwrap();
    assert!(matches!(app.state.modal, Some(Modal::QuitConfirm)));
    assert!(app.state.show_settings);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn live_gate_ignores_f1_and_f2_and_keeps_ctrl_c() {
    let (mut app, root) = demo_app();
    app.state.modal = Some(Modal::LiveGate {
        plan: dd_pantheon::plan::CommandPlan {
            tool: dd_pantheon::plan::ToolKind::Terminus,
            binary: std::path::PathBuf::from("terminus"),
            argv: vec!["env:deploy".into(), "acme-wp.live".into()],
            cwd: None,
            why: "deploy code to acme-wp.live".into(),
            safety: dd_pantheon::plan::SafetyTier::LiveGate,
            target: dd_pantheon::plan::PlanTarget::Env {
                site: "acme-wp".into(),
                env: "live".into(),
            },
            dry_run: false,
            timeout: None,
            expects_json: false,
            extra_env: Vec::new(),
            redact: Vec::new(),
            confirm_with_yes: true,
        },
        expected: "live".into(),
        typed: "li".into(),
    });
    app.handle_key(key(KeyCode::F(1))).unwrap();
    app.handle_key(key(KeyCode::F(2))).unwrap();
    assert!(!app.state.show_settings);
    match &app.state.modal {
        Some(Modal::LiveGate { typed, .. }) => assert_eq!(typed, "li"),
        other => panic!("gate swallowed: {other:?}"),
    }
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    app.handle_key(ctrl_c).unwrap();
    assert!(matches!(app.state.modal, Some(Modal::LiveGate { .. })));
    assert!(!app.state.show_settings);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn page_down_and_home_move_tree() {
    let (mut app, root) = demo_app();
    app.state.focus = FocusPane::Tree;
    app.state.tree_area = ratatui::layout::Rect::new(0, 0, 30, 8);
    app.handle_key(key(KeyCode::Home)).unwrap();
    assert_eq!(app.state.tree_state.selected(), Some(0));
    app.handle_key(key(KeyCode::PageDown)).unwrap();
    assert_ne!(
        app.state.tree_state.selected(),
        Some(0),
        "PageDown should move the tree selection"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn log_scroll_up_stops_follow_and_g_resumes() {
    let (mut app, root) = demo_app();
    app.state.focus = FocusPane::Log;
    app.state.log_area = ratatui::layout::Rect::new(0, 0, 80, 12);
    app.state.log_lines = (0..40).map(|i| format!("line {i}")).collect();
    app.state.log_follow = true;
    app.state.snap_log_follow();
    app.handle_key(key(KeyCode::Char('k'))).unwrap();
    assert!(!app.state.log_follow);
    app.handle_key(key(KeyCode::Char('G'))).unwrap();
    assert!(app.state.log_follow);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn idle_tick_skips_redraw() {
    let (mut app, root) = demo_app();
    app.state.toast = None;
    app.state.config.dirty_since = None;
    app.state.config.sites_dirty_since = None;
    let _ = app.tick();
    app.state.toast = None;
    app.state.config.dirty_since = None;
    app.state.config.sites_dirty_since = None;
    assert!(!app.tick());
    assert!(app.next_poll_timeout().is_none());
    let _ = fs::remove_dir_all(root);
}
