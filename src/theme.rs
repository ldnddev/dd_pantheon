use anyhow::{Context, Result, anyhow};
use ratatui::style::{Color, Modifier, Style};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const THEME_FILE_NAME: &str = "dd_pantheon_theme.yml";
pub const SUPPORTED_THEME_VERSION: u64 = 1;

pub const DEFAULT_HEADER_QUOTES: [&str; 7] = [
    "Dev, then Test, then Live. In that order.",
    "The command is the product.",
    "Never --yes in the dark.",
    "A cockpit that teaches Terminus.",
    "Lando locally. Terminus remotely. Git in between.",
    "LiveGate: type the word, then we talk.",
    "Wait, is it Terminus or Treminus? Let's go with both.",
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
    pub omarchy: Option<ldnddev_theme::OmarchyChoice>,
    pub overrides: BTreeSet<String>,
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
            omarchy: None,
            overrides: BTreeSet::new(),
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
    load_theme_resolved(project_root, &ldnddev_theme::OmarchyPaths::system())
}

pub fn load_theme_resolved(project_root: &Path, omarchy: &ldnddev_theme::OmarchyPaths) -> Theme {
    let mut warnings: Vec<String> = Vec::new();
    let local = project_root.join(THEME_FILE_NAME);
    if let Some(theme) = try_candidate(&local, ThemeSource::Local, omarchy, &mut warnings) {
        return with_warnings(theme, warnings);
    }
    if let Some(home) = dirs::home_dir() {
        let global = home.join(".config/ldnddev").join(THEME_FILE_NAME);
        if let Some(theme) = try_candidate(&global, ThemeSource::Global, omarchy, &mut warnings) {
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

fn try_candidate(
    path: &Path,
    source: ThemeSource,
    omarchy: &ldnddev_theme::OmarchyPaths,
    warnings: &mut Vec<String>,
) -> Option<Theme> {
    if !path.exists() {
        return None;
    }
    match ldnddev_theme::load_from_file(path, ldnddev_theme::ParseMode::Strict, ldnd_source(source))
    {
        Ok(palette) => Some(finish_palette(palette, omarchy)),
        Err(err) => {
            warnings.push(format!("{}: {err:#}; skipped", path.display()));
            None
        }
    }
}

fn ldnd_source(source: ThemeSource) -> ldnddev_theme::ThemeSource {
    match source {
        ThemeSource::Local => ldnddev_theme::ThemeSource::Local,
        ThemeSource::Global => ldnddev_theme::ThemeSource::Global,
        ThemeSource::Default => ldnddev_theme::ThemeSource::Default,
    }
}

fn finish_palette(palette: ldnddev_theme::Palette, omarchy: &ldnddev_theme::OmarchyPaths) -> Theme {
    match ldnddev_theme::apply_omarchy(&palette, omarchy) {
        Ok(resolved) => theme_from_palette(resolved),
        Err(err) => {
            let mut theme = Theme::default();
            theme.warning = Some(format!(
                "omarchy theme failed ({err}); using built-in defaults"
            ));
            theme
        }
    }
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

pub fn extra_theme_fields() -> &'static [ldnddev_theme::ColorField] {
    &[
        ldnddev_theme::EXTRA_MODAL_HEADER,
        ldnddev_theme::EXTRA_TEXT_DISABLED,
        ldnddev_theme::EXTRA_TEXT_INVERSE,
    ]
}

pub fn color_from_rgb(rgb: ldnddev_theme::Rgb) -> Color {
    Color::Rgb(rgb.r, rgb.g, rgb.b)
}

pub fn color_to_rgb(color: Color) -> ldnddev_theme::Rgb {
    match color {
        Color::Rgb(r, g, b) => ldnddev_theme::Rgb { r, g, b },
        _ => ldnddev_theme::Rgb { r: 0, g: 0, b: 0 },
    }
}

pub fn palette_from_theme(theme: &Theme) -> ldnddev_theme::Palette {
    let mut palette = ldnddev_theme::Palette::builtin();
    palette.header_quotes = theme.header_quotes.clone();
    palette.source = match theme.source {
        ThemeSource::Local => ldnddev_theme::ThemeSource::Local,
        ThemeSource::Global => ldnddev_theme::ThemeSource::Global,
        ThemeSource::Default => ldnddev_theme::ThemeSource::Default,
    };
    palette.version = theme.version;
    palette.omarchy = theme.omarchy.clone();
    palette.overrides = theme.overrides.clone();
    let c = theme.colors;
    let pairs = [
        ("base_background", c.base_background),
        ("body_background", c.body_background),
        ("modal_background", c.modal_background),
        ("text_primary", c.text_primary),
        ("text_secondary", c.text_secondary),
        ("text_labels", c.text_labels),
        ("text_active_focus", c.text_active_focus),
        ("modal_labels", c.modal_labels),
        ("modal_text", c.modal_text),
        ("modal_header", c.modal_header),
        ("selected_background", c.selected_background),
        ("border_default", c.border_default),
        ("border_active", c.border_active),
        ("scrollbar", c.scrollbar),
        ("scrollbar_hover", c.scrollbar_hover),
        ("input_border_default", c.input_border_default),
        ("input_border_focus", c.input_border_focus),
        ("input_text_default", c.input_text_default),
        ("input_text_focus", c.input_text_focus),
        ("cursor", c.cursor),
        ("success", c.success),
        ("warning", c.warning),
        ("error", c.error),
        ("info", c.info),
        ("folders", c.folders),
        ("files", c.files),
        ("links", c.links),
    ];
    for (key, color) in pairs {
        palette.set(key, color_to_rgb(color));
    }
    if let Some(color) = c.text_disabled {
        palette.set("text_disabled", color_to_rgb(color));
    }
    if let Some(color) = c.text_inverse {
        palette.set("text_inverse", color_to_rgb(color));
    }
    palette
}

pub fn theme_from_palette(palette: ldnddev_theme::Palette) -> Theme {
    let get = |k: &str| palette.get(k).map(color_from_rgb).unwrap_or(Color::Reset);
    let colors = ThemeColors {
        base_background: get("base_background"),
        body_background: get("body_background"),
        modal_background: get("modal_background"),
        text_primary: get("text_primary"),
        text_secondary: get("text_secondary"),
        text_labels: get("text_labels"),
        text_active_focus: get("text_active_focus"),
        modal_labels: get("modal_labels"),
        modal_text: get("modal_text"),
        modal_header: get("modal_header"),
        selected_background: get("selected_background"),
        border_default: get("border_default"),
        border_active: get("border_active"),
        scrollbar: get("scrollbar"),
        scrollbar_hover: get("scrollbar_hover"),
        input_border_default: get("input_border_default"),
        input_border_focus: get("input_border_focus"),
        input_text_default: get("input_text_default"),
        input_text_focus: get("input_text_focus"),
        cursor: get("cursor"),
        success: get("success"),
        warning: get("warning"),
        error: get("error"),
        info: get("info"),
        folders: get("folders"),
        files: get("files"),
        links: get("links"),
        text_disabled: palette.get("text_disabled").map(color_from_rgb),
        text_inverse: palette.get("text_inverse").map(color_from_rgb),
    };
    let source = match palette.source {
        ldnddev_theme::ThemeSource::Local => ThemeSource::Local,
        ldnddev_theme::ThemeSource::Global => ThemeSource::Global,
        ldnddev_theme::ThemeSource::Default => ThemeSource::Default,
    };
    let quotes = palette.header_quotes.clone();
    let mut theme = Theme::from_colors(colors, source, palette.version);
    theme.header_quotes = if quotes.is_empty() {
        DEFAULT_HEADER_QUOTES
            .iter()
            .map(|s| (*s).to_string())
            .collect()
    } else {
        quotes
    };
    theme.omarchy = palette.omarchy;
    theme.overrides = palette.overrides;
    theme
}
