use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_pantheon::app::App;
use dd_pantheon::models::{CACHE_OK, CACHE_WARN, CacheLevel, cache_ratio_level};
use dd_pantheon::theme::{
    SUPPORTED_THEME_VERSION, ThemeSource, color_hex, load_theme, load_theme_resolved,
};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Theme lookup reads `~/.config/ldnddev`. These tests need a home with no theme file.
fn with_empty_home<T>(body: impl FnOnce() -> T) -> T {
    static LOCK: Mutex<()> = Mutex::new(());
    let _guard = LOCK.lock().expect("theme home lock");
    let prev = std::env::var_os("HOME");
    let home = temp_path("dd_pantheon_empty_home");
    fs::create_dir_all(&home).expect("empty home");
    unsafe {
        std::env::set_var("HOME", &home);
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(body));
    match &prev {
        Some(value) => unsafe { std::env::set_var("HOME", value) },
        None => unsafe { std::env::remove_var("HOME") },
    }
    let _ = fs::remove_dir_all(&home);
    match result {
        Ok(value) => value,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn buffer_text(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| app.draw(frame)).expect("draw");
    let buffer = terminal.backend().buffer();
    let mut out = String::new();
    for y in 0..height {
        for x in 0..width {
            out.push_str(buffer[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

fn temp_path(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("{}_{}_{}", prefix, std::process::id(), nanos))
}

fn valid_theme_yaml() -> &'static str {
    r##"
version: 1
header_quotes:
  - "Custom quote 1"
  - "Custom quote 2"
colors:
  base_background: "#010203"
  body_background: "#111213"
  modal_background: "#212223"
  text_primary: "#313233"
  text_secondary: "#414243"
  text_labels: "#515253"
  text_active_focus: "#616263"
  modal_labels: "#717273"
  modal_text: "#818283"
  modal_header: "#919293"
  selected_background: "#A1A2A3"
  border_default: "#B1B2B3"
  border_active: "#C1C2C3"
  scrollbar: "#D1D2D3"
  scrollbar_hover: "#E1E2E3"
  input_border_default: "#F1F2F3"
  input_border_focus: "#0A0B0C"
  input_text_default: "#1A1B1C"
  input_text_focus: "#2A2B2C"
  cursor: "#3A3B3C"
  success: "#4A4B4C"
  warning: "#5A5B5C"
  error: "#6A6B6C"
  info: "#7A7B7C"
  folders: "#8A8B8C"
  files: "#9A9B9C"
  links: "#AAABAC"
"##
}

#[test]
fn local_standard_theme_file_is_loaded() {
    let root = temp_path("dd_pantheon_theme_local");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join("dd_pantheon_theme.yml"), valid_theme_yaml()).expect("write theme");

    let theme = load_theme(&root);
    assert_eq!(theme.source, ThemeSource::Local);
    assert_eq!(theme.version, SUPPORTED_THEME_VERSION);
    assert_eq!(theme.colors.base_background, Color::Rgb(1, 2, 3));
    assert_eq!(theme.colors.border_active, Color::Rgb(0xc1, 0xc2, 0xc3));
    assert_eq!(theme.colors.modal_header, Color::Rgb(0x91, 0x92, 0x93));
    assert_eq!(
        theme.header_quotes,
        vec!["Custom quote 1".to_string(), "Custom quote 2".to_string()]
    );
    assert!(theme.warning.is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn missing_version_falls_back_to_default_with_warning() {
    with_empty_home(|| {
        let root = temp_path("dd_pantheon_theme_missing_version");
        fs::create_dir_all(&root).expect("create root");
        fs::write(
            root.join("dd_pantheon_theme.yml"),
            "colors:\n  base_background: \"#010203\"\n",
        )
        .expect("write theme");

        let theme = load_theme(&root);
        assert_eq!(theme.source, ThemeSource::Default);
        assert!(theme.warning.as_ref().unwrap().contains("version"));
        let _ = fs::remove_dir_all(root);
    });
}

#[test]
fn unsupported_version_falls_back_to_default_with_warning() {
    with_empty_home(|| {
        let root = temp_path("dd_pantheon_theme_bad_version");
        fs::create_dir_all(&root).expect("create root");
        fs::write(
            root.join("dd_pantheon_theme.yml"),
            "version: 999\ncolors:\n  base_background: \"#010203\"\n",
        )
        .expect("write theme");

        let app = App::new_demo_in(&root, &root).expect("app init");
        assert_eq!(app.state.theme.source, ThemeSource::Default);
        assert!(
            app.state
                .theme_status
                .message
                .to_ascii_lowercase()
                .contains("version")
        );
        let _ = fs::remove_dir_all(root);
    });
}

#[test]
fn omarchy_link_maps_colors_and_keeps_override() {
    let project = temp_path("dd_pantheon_omarchy_link");
    let root = temp_path("dd_pantheon_omarchy_root");
    fs::create_dir_all(root.join("stock/tokyo-night")).expect("mkdir");
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("crates/ldnddev_theme/tests/fixtures/omarchy/tokyo-night/colors.toml");
    fs::copy(&src, root.join("stock/tokyo-night/colors.toml")).expect("copy");
    fs::create_dir_all(&project).expect("project");
    fs::write(
        project.join("dd_pantheon_theme.yml"),
        "version: 1\nomarchy: tokyo-night\ncolors:\n  text_labels: \"#ABCDEF\"\n",
    )
    .expect("yaml");
    let theme = load_theme_resolved(&project, &ldnddev_theme::OmarchyPaths::from_root(&root));
    assert!(theme.warning.is_none(), "{:?}", theme.warning);
    assert_eq!(theme.source, ThemeSource::Local);
    assert_eq!(color_hex(theme.colors.body_background), "#1A1B26");
    assert_eq!(color_hex(theme.colors.text_labels), "#ABCDEF");
    assert_eq!(
        theme.omarchy,
        Some(ldnddev_theme::OmarchyChoice::Theme("tokyo-night".into()))
    );
    assert!(theme.overrides.contains("text_labels"));
    let _ = fs::remove_dir_all(project);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn missing_omarchy_theme_uses_builtin_with_warning() {
    let project = temp_path("dd_pantheon_omarchy_missing");
    let root = temp_path("dd_pantheon_omarchy_empty");
    fs::create_dir_all(&project).expect("project");
    fs::create_dir_all(&root).expect("root");
    fs::write(
        project.join("dd_pantheon_theme.yml"),
        "version: 1\nomarchy: missing-theme\n",
    )
    .expect("yaml");
    let theme = load_theme_resolved(&project, &ldnddev_theme::OmarchyPaths::from_root(&root));
    let warning = theme.warning.unwrap_or_default();
    assert!(warning.contains("omarchy theme failed"), "{warning}");
    assert_eq!(theme.source, ThemeSource::Default);
    assert_eq!(color_hex(theme.colors.base_background), "#0F1114");
    let _ = fs::remove_dir_all(project);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn f2_lists_omarchy_themes_and_esc_reverts() {
    let root = temp_path("dd_pantheon_omarchy_f2");
    let omarchy = root.join("omarchy");
    fs::create_dir_all(omarchy.join("stock/tokyo-night")).expect("mkdir");
    fs::create_dir_all(omarchy.join("state/theme")).expect("state");
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("crates/ldnddev_theme/tests/fixtures/omarchy/tokyo-night/colors.toml");
    fs::copy(&src, omarchy.join("stock/tokyo-night/colors.toml")).expect("copy");
    fs::copy(&src, omarchy.join("state/theme/colors.toml")).expect("current");
    fs::write(omarchy.join("state/theme.name"), "tokyo-night\n").expect("name");

    let mut app = App::new_demo_in(&root, &root).expect("app");
    app.state.omarchy_paths = ldnddev_theme::OmarchyPaths::from_root(&omarchy);
    let before = color_hex(app.state.theme.colors.body_background);
    app.handle_key(key(KeyCode::F(2))).expect("f2");
    assert!(app.state.show_settings, "F2 should open Settings");
    assert!(app.state.modal.is_none());
    assert!(app.state.theme_picker.is_some());
    assert!(app.state.theme_editor.is_none());
    assert_eq!(app.state.settings_nav, 0);
    let listed = buffer_text(&mut app, 120, 40);
    assert!(listed.contains("Settings"), "{listed}");
    assert!(listed.contains("Themes"), "{listed}");
    assert!(listed.contains("acme-wp"), "body stays visible\n{listed}");
    assert!(!listed.contains("F2 Themes"), "{listed}");
    assert!(listed.contains("Enter apply"), "{listed}");
    assert!(listed.contains("e colors"), "{listed}");
    assert!(listed.contains("y save"), "{listed}");
    assert!(listed.contains("t target"), "{listed}");

    app.handle_key(key(KeyCode::Char('t'))).expect("t");
    assert_eq!(
        app.state.theme_picker.as_ref().unwrap().save_target,
        ldnddev_theme::ThemeSaveTarget::Local
    );

    app.handle_key(key(KeyCode::Char('g'))).expect("g");
    assert_eq!(
        color_hex(app.state.theme.colors.body_background),
        "#1A1B26",
        "j/k/g preview the highlighted row"
    );
    app.handle_key(key(KeyCode::Enter)).expect("enter");
    assert_eq!(color_hex(app.state.theme.colors.body_background), "#1A1B26");

    app.handle_key(key(KeyCode::Char('e'))).expect("e");
    assert!(app.state.theme_editor.is_some());
    assert!(app.state.theme_picker.is_some());
    assert!(app.state.show_settings);
    let editing = buffer_text(&mut app, 120, 40);
    assert!(editing.contains("Esc back"), "{editing}");

    app.handle_key(key(KeyCode::Esc)).expect("esc editor");
    assert!(app.state.theme_editor.is_none());
    assert!(app.state.theme_picker.is_some());
    assert!(app.state.show_settings);

    app.handle_key(key(KeyCode::Tab)).expect("tab help");
    assert_eq!(app.state.settings_nav, 1);
    let help = buffer_text(&mut app, 120, 40);
    assert!(help.contains("F1 / F2"), "{help}");
    assert!(help.contains("Ctrl+Q"), "{help}");

    app.handle_key(key(KeyCode::Esc)).expect("esc help");
    assert!(!app.state.show_settings);
    assert!(app.state.modal.is_none());
    assert!(app.state.theme_picker.is_none());
    assert_eq!(color_hex(app.state.theme.colors.body_background), before);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn f2_without_omarchy_opens_hex_editor() {
    let root = temp_path("dd_pantheon_no_omarchy");
    fs::create_dir_all(&root).expect("root");
    let mut app = App::new_demo_in(&root, &root).expect("app");
    app.state.omarchy_paths = ldnddev_theme::OmarchyPaths::from_root(std::path::Path::new(
        "/nonexistent/dd_pantheon_omarchy",
    ));
    let before = color_hex(app.state.theme.colors.body_background);
    let base_before = color_hex(app.state.theme.colors.base_background);
    app.handle_key(key(KeyCode::F(2))).expect("f2");
    assert!(app.state.show_settings);
    assert!(app.state.theme_editor.is_some());
    assert!(app.state.theme_picker.is_none());
    assert!(app.state.modal.is_none());
    let editing = buffer_text(&mut app, 120, 40);
    assert!(editing.contains("Esc revert"), "{editing}");
    assert!(!editing.contains("Esc back"), "{editing}");
    app.handle_key(key(KeyCode::Char('l'))).expect("nudge");
    assert_ne!(color_hex(app.state.theme.colors.base_background), base_before);
    app.handle_key(key(KeyCode::F(2))).expect("f2 again");
    assert!(!app.state.show_settings);
    assert!(app.state.theme_editor.is_none());
    assert_eq!(color_hex(app.state.theme.colors.body_background), before);
    assert_eq!(color_hex(app.state.theme.colors.base_background), base_before);

    app.handle_key(key(KeyCode::F(1))).expect("f1");
    assert!(app.state.show_settings);
    app.handle_key(key(KeyCode::Char('l'))).expect("nudge");
    let ctrl_q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
    app.handle_key(key(KeyCode::Char('q'))).expect("bare q");
    assert!(!app.state.should_quit);
    assert!(app.state.show_settings);
    let quit = app.handle_key(ctrl_q).expect("ctrl q");
    assert!(quit);
    assert!(app.state.should_quit);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cache_thresholds() {
    assert_eq!(CACHE_OK, 0.80);
    assert_eq!(CACHE_WARN, 0.50);
    assert_eq!(cache_ratio_level(0.91), CacheLevel::Ok);
    assert_eq!(cache_ratio_level(0.60), CacheLevel::Warn);
    assert_eq!(cache_ratio_level(0.42), CacheLevel::Bad);
}
