//! Window prefs. They live in gui.json, never in the theme YAML or site data.

use std::fs;
use std::path::{Path, PathBuf};

use ldnddev_theme::default_config_home;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}

impl Density {
    pub fn label(self) -> &'static str {
        match self {
            Self::Comfortable => "Comfortable",
            Self::Compact => "Compact",
        }
    }

    pub fn row_px(self) -> f32 {
        match self {
            Self::Comfortable => 28.0,
            Self::Compact => 24.0,
        }
    }

    pub fn toggled(self) -> Self {
        match self {
            Self::Comfortable => Self::Compact,
            Self::Compact => Self::Comfortable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuiSettings {
    #[serde(default)]
    pub density: Density,
    /// On by default, matching the terminal.
    #[serde(default = "default_true")]
    pub vim_keys: bool,
    #[serde(default)]
    pub reduced_motion: bool,
}

fn default_true() -> bool {
    true
}

impl Default for GuiSettings {
    fn default() -> Self {
        Self {
            density: Density::Comfortable,
            vim_keys: true,
            reduced_motion: false,
        }
    }
}

/// `$XDG_CONFIG_HOME/ldnddev/dd_pantheon/gui.json`.
pub fn settings_path() -> PathBuf {
    default_config_home()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ldnddev")
        .join("dd_pantheon")
        .join("gui.json")
}

pub fn load_from(path: &Path) -> GuiSettings {
    let Ok(bytes) = fs::read(path) else {
        return GuiSettings::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

pub fn save_to(path: &Path, settings: &GuiSettings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_vec_pretty(settings).unwrap_or_else(|_| b"{}".to_vec());
    fs::write(path, json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gui_json_round_trip_in_a_temp_dir() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gui.json");
        let document = dir.path().join("sites.toml");
        let theme = dir.path().join("dd_pantheon_theme.yml");
        fs::write(&document, "sites = {}\n").unwrap();
        fs::write(&theme, "version: 1\n").unwrap();
        let settings = GuiSettings {
            density: Density::Compact,
            vim_keys: false,
            reduced_motion: true,
        };
        save_to(&path, &settings).unwrap();
        let loaded = load_from(&path);
        assert_eq!(loaded, settings);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("compact"), "{text}");
        assert!(text.contains("\"vim_keys\": false"), "{text}");
        assert!(!text.contains("colors"), "{text}");
        assert_eq!(fs::read_to_string(&document).unwrap(), "sites = {}\n");
        assert_eq!(fs::read_to_string(&theme).unwrap(), "version: 1\n");
    }

    #[test]
    fn missing_file_defaults_vim_on() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = load_from(&dir.path().join("gui.json"));
        assert!(loaded.vim_keys);
        assert_eq!(loaded.density, Density::Comfortable);
        assert!(!loaded.reduced_motion);
    }
}
