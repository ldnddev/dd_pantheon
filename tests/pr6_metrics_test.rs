use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_pantheon::app::App;
use dd_pantheon::models::MetricsPeriod;
use dd_pantheon::state::{FocusPane, TreeSel};
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
    let root = temp_path("dd_pantheon_pr6");
    fs::create_dir_all(&root).expect("root");
    let app = App::new_demo_in(&root, &root).expect("app");
    (app, root)
}

#[test]
fn demo_period_keys_seed_series_and_m_stays_cms() {
    let (mut app, root) = demo_app();
    assert_eq!(
        app.state.selected,
        TreeSel::Env {
            site: "acme-wp".into(),
            env: "test".into(),
        }
    );
    assert_eq!(app.state.metrics_period, MetricsPeriod::Day);
    let day = app.state.selected_metrics().expect("day fixture");
    assert_eq!(day.period, MetricsPeriod::Day);
    assert!(!day.points.is_empty());

    app.handle_key(key(KeyCode::Char('w'))).unwrap();
    assert_eq!(app.state.metrics_period, MetricsPeriod::Week);
    let week = app.state.selected_metrics().expect("week series");
    assert_eq!(week.period, MetricsPeriod::Week);
    assert!(!week.points.is_empty());
    assert!(app.state.jobs.is_empty());

    app.handle_key(key(KeyCode::Char('m'))).unwrap();
    assert_eq!(app.state.metrics_period, MetricsPeriod::Week);
    let toast = app.state.toast.as_ref().expect("cms toast");
    assert!(
        toast.message.to_ascii_lowercase().contains("cms"),
        "lowercase m must stay CMS, got {}",
        toast.message
    );

    app.handle_key(KeyEvent::new(KeyCode::Char('M'), KeyModifiers::SHIFT))
        .unwrap();
    assert_eq!(app.state.metrics_period, MetricsPeriod::Month);
    assert_eq!(
        app.state.selected_metrics().expect("month").period,
        MetricsPeriod::Month
    );

    app.handle_key(key(KeyCode::Char('d'))).unwrap();
    assert_eq!(app.state.metrics_period, MetricsPeriod::Day);

    app.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::SHIFT))
        .unwrap();
    assert_eq!(app.state.metrics_period, MetricsPeriod::Month);

    app.state.focus = FocusPane::Preview;
    app.handle_key(key(KeyCode::Char('w'))).unwrap();
    assert_eq!(app.state.metrics_period, MetricsPeriod::Week);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn demo_site_row_has_no_env_series() {
    let (mut app, root) = demo_app();
    app.state.selected = TreeSel::Site("acme-wp".into());
    assert!(app.state.selected_metrics().is_none());
    let _ = fs::remove_dir_all(root);
}
