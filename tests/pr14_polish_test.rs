use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_pantheon::app::App;
use dd_pantheon::doctor::AuthState;
use dd_pantheon::models::SiteOverlay;
use dd_pantheon::plan::SafetyTier;
use dd_pantheon::state::{FocusPane, TreeSel};
use dd_pantheon::workflows::auth;
use dd_pantheon::workflows::inventory;
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
    let root = temp_path("dd_pantheon_pr14");
    fs::create_dir_all(&root).expect("root");
    let app = App::new_demo_in(&root, &root).expect("app");
    (app, root)
}

#[test]
fn overlay_from_sites_toml_applies_on_inventory() {
    let (mut app, root) = demo_app();
    app.state.config.sites.insert(
        "acme-wp".into(),
        SiteOverlay {
            cms: Some(dd_pantheon::models::Framework::WordPress),
            multidev_ok: false,
            composer_managed: true,
            git_branch: Some("master".into()),
            local_path: None,
        },
    );
    inventory::apply_site_list(
        &mut app.state,
        r#"{
          "acme-wp": {
            "name": "acme-wp",
            "id": "aaa",
            "framework": "wordpress",
            "frozen": false
          }
        }"#,
    );
    let site = app.state.site("acme-wp").expect("site");
    let overlay = site.overlay.as_ref().expect("overlay");
    assert!(!overlay.multidev_ok);
    assert!(overlay.composer_managed);
    assert_eq!(overlay.git_branch.as_deref(), Some("master"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn persist_path_writes_overlay_not_locals_when_entry_exists() {
    let (mut app, root) = demo_app();
    app.state.config.sites.insert(
        "acme-wp".into(),
        SiteOverlay {
            cms: Some(dd_pantheon::models::Framework::WordPress),
            ..SiteOverlay::default()
        },
    );
    app.state
        .config
        .persist_local_path("acme-wp", &root.join("site"));
    assert!(
        app.state.config.sites["acme-wp"]
            .local_path
            .as_ref()
            .is_some_and(|p| p.ends_with("site"))
    );
    assert!(!app.state.config.config.locals.contains_key("acme-wp"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn logout_is_mutating_and_replaces_login_when_logged_in() {
    let (mut app, root) = demo_app();
    let plan = auth::plan_logout(std::path::PathBuf::from("terminus"));
    assert_eq!(plan.argv[0], "auth:logout");
    assert_eq!(plan.safety, SafetyTier::Mutating);

    app.state.auth = AuthState::LoggedIn {
        email: "a@b.com".into(),
        id: None,
    };
    dd_pantheon::workflows::local::refresh_actions(&mut app.state);
    assert!(app.state.actions.iter().any(|a| a.id == "logout"));
    assert!(!app.state.actions.iter().any(|a| a.id == "login"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn preview_y_copies_redacted_or_toasts_missing_helper() {
    let (mut app, root) = demo_app();
    app.state.focus = FocusPane::Preview;
    app.handle_key(key(KeyCode::Char('y'))).unwrap();
    let toast = app.state.toast.as_ref().expect("toast");
    let msg = toast.message.to_ascii_lowercase();
    assert!(
        msg.contains("copied") || msg.contains("wl-copy") || msg.contains("xclip"),
        "unexpected clipboard toast: {}",
        toast.message
    );
    if msg.contains("copied") {
        assert!(
            app.state.log_lines.iter().any(|l| l.starts_with("copied:")),
            "success should also append an info log line"
        );
        assert!(
            !app.state
                .log_lines
                .iter()
                .any(|l| l.contains("--machine-token=")),
            "copy log must stay redacted"
        );
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn debug_log_is_a_config_scalar() {
    let (app, root) = demo_app();
    assert!(!app.state.config.config.debug_log);
    let _ = app.state.selected;
    let _ = TreeSel::None;
    let _ = fs::remove_dir_all(root);
}
