use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_pantheon::app::App;
use dd_pantheon::models::SiteOverlay;
use dd_pantheon::plan::{SafetyTier, StagedPlan};
use dd_pantheon::state::{FocusPane, Modal, TreeSel};
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
    let root = temp_path("dd_pantheon_pr11");
    fs::create_dir_all(&root).expect("root");
    let app = App::new_demo_in(&root, &root).expect("app");
    (app, root)
}

fn pick_action(app: &mut App, id: &str) {
    let idx = app
        .state
        .actions
        .iter()
        .position(|a| a.id == id)
        .unwrap_or_else(|| panic!("missing action {id}"));
    app.state.action_state.select(Some(idx));
    app.state.focus = FocusPane::Inspector;
}

#[test]
fn no_w_key_for_wipe() {
    let (mut app, root) = demo_app();
    app.handle_key(key(KeyCode::Char('W'))).unwrap();
    let plan = app.state.current.as_ref().and_then(|p| p.current());
    assert!(
        plan.is_none_or(|p| p.argv.first().map(|a| a.as_str()) != Some("env:wipe")),
        "W must not stage wipe"
    );
    assert!(app.state.actions.iter().any(|a| a.id == "wipe"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn wipe_test_is_backup_first_destructive() {
    let (mut app, root) = demo_app();
    app.state.demo = false;
    pick_action(&mut app, "wipe");
    app.handle_key(key(KeyCode::Enter)).unwrap();
    match app.state.current.as_ref() {
        Some(StagedPlan::Workflow { plan, .. }) => {
            assert_eq!(plan.steps[0].argv[0], "backup:create");
            assert_eq!(plan.steps.last().unwrap().argv[0], "env:wipe");
            assert_eq!(plan.safety, SafetyTier::Destructive);
        }
        other => panic!("expected wipe workflow, got {other:?}"),
    }
    assert!(matches!(
        app.state.modal,
        Some(Modal::ConfirmDestructive { .. })
    ));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn wipe_live_opens_livegate() {
    let (mut app, root) = demo_app();
    app.state.demo = false;
    app.state.selected = TreeSel::Env {
        site: "acme-wp".into(),
        env: "live".into(),
    };
    force_actions(&mut app);
    pick_action(&mut app, "wipe");
    app.handle_key(key(KeyCode::Enter)).unwrap();
    match app.state.modal {
        Some(Modal::LiveGate { ref expected, .. }) => assert_eq!(expected, "live"),
        other => panic!("expected LiveGate, got {other:?}"),
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn clone_opens_form_default_origin_live() {
    let (mut app, root) = demo_app();
    pick_action(&mut app, "clone-content");
    app.handle_key(key(KeyCode::Enter)).unwrap();
    match &app.state.modal {
        Some(Modal::CloneContent {
            target,
            origins,
            origin_idx,
            cc,
            ..
        }) => {
            assert_eq!(target, "test");
            assert!(cc);
            assert_eq!(origins.get(*origin_idx).map(|s| s.as_str()), Some("live"));
        }
        other => panic!("expected CloneContent, got {other:?}"),
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn multidev_create_hidden_when_overlay_false() {
    let (mut app, root) = demo_app();
    let site = app
        .state
        .sites
        .iter_mut()
        .find(|s| s.name == "acme-wp")
        .unwrap();
    if let Some(overlay) = site.overlay.as_mut() {
        overlay.multidev_ok = false;
    } else {
        site.overlay = Some(SiteOverlay {
            cms: None,
            multidev_ok: false,
            composer_managed: false,
            git_branch: None,
            local_path: None,
        });
    }
    force_actions(&mut app);
    assert!(
        !app.state.actions.iter().any(|a| a.id == "multidev-create"),
        "create should be hidden when multidev_ok is false"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn multidev_create_modal_and_merge_on_feat_x() {
    let (mut app, root) = demo_app();
    assert!(app.state.actions.iter().any(|a| a.id == "multidev-create"));
    pick_action(&mut app, "multidev-create");
    app.handle_key(key(KeyCode::Enter)).unwrap();
    assert!(matches!(
        app.state.modal,
        Some(Modal::MultidevCreate { .. })
    ));
    app.handle_key(key(KeyCode::Esc)).unwrap();

    app.state.selected = TreeSel::Env {
        site: "acme-wp".into(),
        env: "feat-x".into(),
    };
    force_actions(&mut app);
    pick_action(&mut app, "multidev-merge");
    app.handle_key(key(KeyCode::Enter)).unwrap();
    let plan = match app.state.current.as_ref() {
        Some(StagedPlan::One(p)) => p,
        other => panic!("expected merge plan, got {other:?}"),
    };
    assert_eq!(plan.argv[0], "multidev:merge-to-dev");
    assert_eq!(plan.argv[1], "acme-wp.feat-x");
    let _ = fs::remove_dir_all(root);
}

/// Test helper: refresh Actions after mutating overlay/selection without a tree move.
pub fn force_actions(app: &mut App) {
    dd_pantheon::workflows::local::refresh_actions(&mut app.state);
}
