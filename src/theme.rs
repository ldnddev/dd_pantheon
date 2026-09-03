use anyhow::{Context, Result, anyhow};
use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

pub const THEME_FILE_NAME: &str = "dd_pantheon_theme.yml";
pub const SUPPORTED_THEME_VERSION: u64 = 1;

pub const DEFAULT_HEADER_QUOTES: [&str; 6] = [
    "Dev, then Test, then Live. In that order.",
    "The command is the product.",
    "Never --yes in the dark.",
    "A cockpit that teaches Terminus.",
    "Lando locally. Terminus remotely. Git in between.",
    "LiveGate: type the word, then we talk.",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeSource {
    Local,
    Global,
    Default,
}

impl ThemeSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Global => "global",
            Self::Default => "default",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeStatusLevel {
    Healthy,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeStatus {
    pub level: ThemeStatusLevel,
    pub message: String,
}

impl ThemeStatus {
    pub fn healthy(source: ThemeSource, version: u64) -> Self {
        Self {
            level: ThemeStatusLevel::Healthy,
            message: format!("Theme OK: {} schema v{}", source.label(), version),
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            level: ThemeStatusLevel::Warning,
            message: message.into(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ThemeFile {
    version: Option<u64>,
    #[serde(default)]
    header_quotes: Option<Vec<String>>,
    colors: Option<ThemeFileColors>,
}

#[derive(Debug, Deserialize)]
struct ThemeFileColors {
    base_background: String,
    body_background: String,
    modal_background: String,
    text_primary: String,
    text_secondary: String,
    text_labels: String,
    text_active_focus: String,
    modal_labels: String,
    modal_text: String,
    modal_header: String,
    selected_background: String,
    border_default: String,
    border_active: String,
    scrollbar: String,
    scrollbar_hover: String,
    input_border_default: String,
    input_border_focus: String,
    input_text_default: String,
    input_text_focus: String,
    cursor: String,
    success: String,
    warning: String,
    error: String,
    info: String,
    folders: String,
    files: String,
    links: String,
    #[serde(default)]
    text_disabled: Option<String>,
    #[serde(default)]
    text_inverse: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub struct ThemeColors {
    pub base_background: Color,
    pub body_background: Color,
    pub modal_background: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_labels: Color,
    pub text_active_focus: Color,
    pub modal_labels: Color,
    pub modal_text: Color,
    pub modal_header: Color,
    pub selected_background: Color,
    pub border_default: Color,
    pub border_active: Color,
    pub scrollbar: Color,
    pub scrollbar_hover: Color,
    pub input_border_default: Color,
    pub input_border_focus: Color,
    pub input_text_default: Color,
    pub input_text_focus: Color,
    pub cursor: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,
    pub folders: Color,
    pub files: Color,
    pub links: Color,
    pub text_disabled: Option<Color>,
    pub text_inverse: Option<Color>,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            base_background: Color::Rgb(0x0f, 0x11, 0x14),
            body_background: Color::Rgb(0x2a, 0x2d, 0x31),
            modal_background: Color::Rgb(0x1c, 0x1e, 0x21),
            text_primary: Color::Rgb(0xf5, 0xf6, 0xf7),
            text_secondary: Color::Rgb(0x9e, 0xa3, 0xaa),
            text_labels: Color::Rgb(0xff, 0xaf, 0x46),
            text_active_focus: Color::Rgb(0x64, 0xb4, 0xf5),
            modal_labels: Color::Rgb(0x64, 0xb4, 0xf5),
            modal_text: Color::Rgb(0xf5, 0xf6, 0xf7),
            modal_header: Color::Rgb(0x64, 0xb4, 0xf5),
            selected_background: Color::Rgb(0x0f, 0x11, 0x14),
            border_default: Color::Rgb(0xf5, 0xf6, 0xf7),
            border_active: Color::Rgb(0x64, 0xb4, 0xf5),
            scrollbar: Color::Rgb(0xff, 0xa0, 0x87),
            scrollbar_hover: Color::Rgb(0x64, 0xb4, 0xf5),
            input_border_default: Color::Rgb(0xf5, 0xf6, 0xf7),
            input_border_focus: Color::Rgb(0x64, 0xb4, 0xf5),
            input_text_default: Color::Rgb(0xf5, 0xf6, 0xf7),
            input_text_focus: Color::Rgb(0x64, 0xb4, 0xf5),
            cursor: Color::Rgb(0x64, 0xb4, 0xf5),
            success: Color::Rgb(0x82, 0xe0, 0xaa),
            warning: Color::Rgb(0xf5, 0xc4, 0x69),
            error: Color::Rgb(0xe5, 0x73, 0x73),
            info: Color::Rgb(0x5d, 0xad, 0xe2),
            folders: Color::Rgb(0x64, 0xb4, 0xf5),
            files: Color::Rgb(0xff, 0xaf, 0x46),
            links: Color::Rgb(0xff, 0xa0, 0x87),
            text_disabled: None,
            text_inverse: None,
        }
    }
}

#[derive(Clone)]
pub struct Theme {
    pub source: ThemeSource,
    pub version: u64,
    pub colors: ThemeColors,
    pub header_quotes: Vec<String>,
    pub warning: Option<String>,
    pub app_shell: Style,
    pub body: Style,
    pub modal: Style,
    pub secondary: Style,
    pub label: Style,
    pub active_label: Style,
    pub modal_label: Style,
    pub modal_text: Style,
    pub modal_header: Style,
    pub selected: Style,
    pub success: Style,
    pub warning_style: Style,
    pub error: Style,
    pub info: Style,
    pub border: Style,
    pub active_border: Style,
    pub input_border: Style,
    pub input_border_focus: Style,
    pub input_text: Style,
    pub input_text_focus: Style,
    pub cursor: Style,
    pub scrollbar: Style,
    pub scrollbar_hover: Style,
    pub folder: Style,
    pub file: Style,
}

impl Theme {
    pub fn from_colors(colors: ThemeColors, source: ThemeSource, version: u64) -> Self {
        Self {
            source,
            version,
            colors,
            header_quotes: vec![],
            warning: None,
            app_shell: Style::default()
                .fg(colors.text_primary)
                .bg(colors.base_background),
            body: Style::default()
                .fg(colors.text_primary)
                .bg(colors.body_background),
            modal: Style::default()
                .fg(colors.modal_text)
                .bg(colors.modal_background),
            secondary: Style::default()
                .fg(colors.text_secondary)
                .bg(colors.body_background),
            label: Style::default()
                .fg(colors.text_labels)
                .bg(colors.body_background),
            active_label: Style::default()
                .fg(colors.text_active_focus)
                .bg(colors.body_background)
                .add_modifier(Modifier::BOLD),
            modal_label: Style::default()
                .fg(colors.modal_labels)
                .bg(colors.modal_background)
                .add_modifier(Modifier::BOLD),
            modal_text: Style::default()
                .fg(colors.modal_text)
                .bg(colors.modal_background),
            modal_header: Style::default()
                .fg(colors.modal_header)
                .bg(colors.modal_background)
                .add_modifier(Modifier::BOLD),
            selected: Style::default()
                .fg(colors.text_active_focus)
                .bg(colors.selected_background)
                .add_modifier(Modifier::BOLD),
            success: Style::default()
                .fg(colors.success)
                .bg(colors.body_background),
            warning_style: Style::default()
                .fg(colors.warning)
                .bg(colors.body_background),
            error: Style::default().fg(colors.error).bg(colors.body_background),
            info: Style::default().fg(colors.info).bg(colors.body_background),
            border: Style::default().fg(colors.border_default),
            active_border: Style::default().fg(colors.border_active),
            input_border: Style::default().fg(colors.input_border_default),
            input_border_focus: Style::default().fg(colors.input_border_focus),
            input_text: Style::default()
                .fg(colors.input_text_default)
                .bg(colors.modal_background),
            input_text_focus: Style::default()
                .fg(colors.input_text_focus)
                .bg(colors.modal_background),
            cursor: Style::default()
                .fg(colors.cursor)
                .bg(colors.modal_background)
                .add_modifier(Modifier::REVERSED),
            scrollbar: Style::default().fg(colors.scrollbar),
            scrollbar_hover: Style::default().fg(colors.scrollbar_hover),
            folder: Style::default()
                .fg(colors.folders)
                .bg(colors.modal_background),
            file: Style::default()
                .fg(colors.files)
                .bg(colors.modal_background),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        let mut theme = Self::from_colors(
            ThemeColors::default(),
            ThemeSource::Default,
            SUPPORTED_THEME_VERSION,
        );
        theme.header_quotes = DEFAULT_HEADER_QUOTES
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        theme
    }
}

pub fn load_theme(project_root: &Path) -> Theme {
    let mut warnings: Vec<String> = Vec::new();
    let local = project_root.join(THEME_FILE_NAME);
    if let Some(theme) = try_candidate(&local, ThemeSource::Local, &mut warnings) {
        return with_warnings(theme, warnings);
    }
    if let Some(home) = dirs::home_dir() {
        let global = home.join(".config/ldnddev").join(THEME_FILE_NAME);
        if let Some(theme) = try_candidate(&global, ThemeSource::Global, &mut warnings) {
            return with_warnings(theme, warnings);
        }
    }
    let mut theme = Theme::default();
    if !warnings.is_empty() {
        theme.warning = Some(warnings.join("; "));
    }
    theme
}

fn with_warnings(mut theme: Theme, warnings: Vec<String>) -> Theme {
    if !warnings.is_empty() {
        let extra = warnings.join("; ");
        theme.warning = Some(match theme.warning.take() {
            Some(existing) => format!("{existing}; {extra}"),
            None => extra,
        });
    }
    theme
}

fn try_candidate(path: &Path, source: ThemeSource, warnings: &mut Vec<String>) -> Option<Theme> {
    if !path.exists() {
        return None;
    }
    match load_theme_file(path, source) {
        Ok(theme) => Some(theme),
        Err(err) => {
            warnings.push(format!("{}: {err:#}; skipped", path.display()));
            None
        }
    }
}

fn load_theme_file(path: &Path, source: ThemeSource) -> Result<Theme> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read theme file: {}", path.display()))?;
    let parsed: ThemeFile = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse theme file: {}", path.display()))?;
    match parsed.version {
        Some(v) if v == SUPPORTED_THEME_VERSION => {}
        Some(v) => {
            return Err(anyhow!(
                "Unsupported theme schema version `{v}` (expected `{SUPPORTED_THEME_VERSION}`)"
            ));
        }
        None => {
            return Err(anyhow!(
                "Missing required theme key `version`; expected `{SUPPORTED_THEME_VERSION}`"
            ));
        }
    }
    let colors_file = parsed
        .colors
        .ok_or_else(|| anyhow!("Missing required theme key `colors`"))?;
    let colors = map_colors(colors_file)?;
    let mut quotes = parsed.header_quotes.unwrap_or_default();
    if quotes.is_empty() {
        quotes = DEFAULT_HEADER_QUOTES
            .iter()
            .map(|s| (*s).to_string())
            .collect();
    }
    let mut theme = Theme::from_colors(colors, source, SUPPORTED_THEME_VERSION);
    theme.header_quotes = quotes;
    Ok(theme)
}

fn map_colors(c: ThemeFileColors) -> Result<ThemeColors> {
    Ok(ThemeColors {
        base_background: parse_hex("base_background", &c.base_background)?,
        body_background: parse_hex("body_background", &c.body_background)?,
        modal_background: parse_hex("modal_background", &c.modal_background)?,
        text_primary: parse_hex("text_primary", &c.text_primary)?,
        text_secondary: parse_hex("text_secondary", &c.text_secondary)?,
        text_labels: parse_hex("text_labels", &c.text_labels)?,
        text_active_focus: parse_hex("text_active_focus", &c.text_active_focus)?,
        modal_labels: parse_hex("modal_labels", &c.modal_labels)?,
        modal_text: parse_hex("modal_text", &c.modal_text)?,
        modal_header: parse_hex("modal_header", &c.modal_header)?,
        selected_background: parse_hex("selected_background", &c.selected_background)?,
        border_default: parse_hex("border_default", &c.border_default)?,
        border_active: parse_hex("border_active", &c.border_active)?,
        scrollbar: parse_hex("scrollbar", &c.scrollbar)?,
        scrollbar_hover: parse_hex("scrollbar_hover", &c.scrollbar_hover)?,
        input_border_default: parse_hex("input_border_default", &c.input_border_default)?,
        input_border_focus: parse_hex("input_border_focus", &c.input_border_focus)?,
        input_text_default: parse_hex("input_text_default", &c.input_text_default)?,
        input_text_focus: parse_hex("input_text_focus", &c.input_text_focus)?,
        cursor: parse_hex("cursor", &c.cursor)?,
        success: parse_hex("success", &c.success)?,
        warning: parse_hex("warning", &c.warning)?,
        error: parse_hex("error", &c.error)?,
        info: parse_hex("info", &c.info)?,
        folders: parse_hex("folders", &c.folders)?,
        files: parse_hex("files", &c.files)?,
        links: parse_hex("links", &c.links)?,
        text_disabled: c
            .text_disabled
            .as_deref()
            .map(|v| parse_hex("text_disabled", v))
            .transpose()?,
        text_inverse: c
            .text_inverse
            .as_deref()
            .map(|v| parse_hex("text_inverse", v))
            .transpose()?,
    })
}

pub fn parse_hex(key: &str, value: &str) -> Result<Color> {
    let raw = value.trim().trim_matches('"').trim_start_matches('#');
    if raw.len() != 6 {
        return Err(anyhow!(
            "Theme color `{key}` must be a 6-digit hex value, got `{value}`"
        ));
    }
    let r = u8::from_str_radix(&raw[0..2], 16)
        .with_context(|| format!("Theme color `{key}` is not valid hex"))?;
    let g = u8::from_str_radix(&raw[2..4], 16)
        .with_context(|| format!("Theme color `{key}` is not valid hex"))?;
    let b = u8::from_str_radix(&raw[4..6], 16)
        .with_context(|| format!("Theme color `{key}` is not valid hex"))?;
    Ok(Color::Rgb(r, g, b))
}

pub fn color_hex(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("#{r:02X}{g:02X}{b:02X}"),
        other => format!("{other:?}"),
    }
}

pub fn global_theme_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/ldnddev")
        .join(THEME_FILE_NAME)
}
