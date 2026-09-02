use dd_pantheon::app::App;
use dd_pantheon::models::{CACHE_OK, CACHE_WARN, CacheLevel, cache_ratio_level};
use dd_pantheon::theme::{SUPPORTED_THEME_VERSION, ThemeSource, load_theme};
use ratatui::style::Color;
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
}

#[test]
fn unsupported_version_falls_back_to_default_with_warning() {
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
}

#[test]
fn cache_thresholds() {
    assert_eq!(CACHE_OK, 0.80);
    assert_eq!(CACHE_WARN, 0.50);
    assert_eq!(cache_ratio_level(0.91), CacheLevel::Ok);
    assert_eq!(cache_ratio_level(0.60), CacheLevel::Warn);
    assert_eq!(cache_ratio_level(0.42), CacheLevel::Bad);
}
