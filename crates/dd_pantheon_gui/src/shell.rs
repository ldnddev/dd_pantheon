//! One shell owns the site tree, the theme, and the Settings sheet.

#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use dd_pantheon::app::{App as PantheonApp, LaunchOpts};
use dd_pantheon::state::{FocusPane, Modal};
use dd_pantheon::theme::{self, THEME_FILE_NAME};
use gpui::{
    App, ClickEvent, Context, FocusHandle, Focusable, FontWeight, Hsla, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Render, Rgba, ScrollHandle,
    StatefulInteractiveElement, Styled, Window, div, prelude::*, px, relative,
};
use ldnddev_theme::{
    EditorKey, EditorOutcome, PickerOutcome, PickerRow, ThemeEditor, ThemePicker, ThemeSaveTarget,
};

use crate::RunOpts;
use crate::keys::{self, vim_motion};
use crate::prefs::{self, Density, GuiSettings};
use crate::present;
use crate::shortcuts;

const BARS: [&str; 4] = ["▃", "▅", "█", "▅"];
const FRAME_MS: u128 = 150;
const SETTINGS_NAV: [&str; 3] = ["Themes", "Window", "Help"];

/// Four-frame busy mark. Label is `{bar} d_d {bar}`.
pub fn busy_label(elapsed: Duration) -> String {
    let bar = BARS[(elapsed.as_millis() / FRAME_MS) as usize % BARS.len()];
    format!("{bar} d_d {bar}")
}

#[derive(Clone, Debug)]
pub struct Boot {
    pub demo: bool,
    pub root: Option<PathBuf>,
    pub project_root: PathBuf,
    pub config_dir: PathBuf,
    pub gui_path: PathBuf,
    /// Tests set this so boot and F3 never call Terminus.
    pub skip_detect: bool,
    pub quit_runtime: bool,
    /// Use `App::new_demo_in` (fixtures, skip_detect). The default test boot.
    pub fixtures: bool,
}

impl Boot {
    pub fn from_run(opts: RunOpts) -> Self {
        let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let config_dir = dd_pantheon::config::default_config_dir();
        Self {
            demo: opts.demo,
            root: opts.root,
            project_root,
            config_dir: config_dir.clone(),
            gui_path: prefs::settings_path(),
            // Match the terminal: --demo still looks up tool paths, and it does
            // not start doctor jobs. User plans stay gated on `state.demo`.
            skip_detect: false,
            quit_runtime: true,
            fixtures: false,
        }
    }

    /// Headless boot. Fixtures only. Does not call Terminus.
    #[cfg(test)]
    pub fn demo(dir: &Path) -> Self {
        Self {
            demo: true,
            root: None,
            project_root: dir.to_path_buf(),
            config_dir: dir.to_path_buf(),
            gui_path: dir.join("gui.json"),
            skip_detect: true,
            quit_runtime: false,
            fixtures: true,
        }
    }
}

struct Appearance {
    picker: ThemePicker,
    editor: Option<ThemeEditor>,
    direct_editor: bool,
    hex_error: Option<String>,
}

pub struct Shell {
    focus: FocusHandle,
    pub app: PantheonApp,
    palette: ldnddev_theme::Palette,
    gui: GuiSettings,
    gui_path: PathBuf,
    project_root: PathBuf,
    appearance: Option<Appearance>,
    settings_nav: u8,
    settings_row: usize,
    settings_query: String,
    settings_query_on: bool,
    quit: bool,
    quit_runtime: bool,
    note: String,
    settings_scroll: ScrollHandle,
    tree_scroll: ScrollHandle,
    booted_at: Instant,
}

struct Paint {
    base: Hsla,
    body: Hsla,
    raised: Hsla,
    text: Hsla,
    secondary: Hsla,
    border: Hsla,
    selected: Hsla,
    accent: Hsla,
}

impl Shell {
    pub fn open(boot: Boot, cx: &mut Context<Self>) -> Self {
        let app = if boot.fixtures {
            PantheonApp::new_demo_in(&boot.project_root, &boot.config_dir)
                .expect("demo fixtures boot")
        } else {
            PantheonApp::new(LaunchOpts {
                demo: boot.demo,
                root: boot.root,
                project_root: boot.project_root.clone(),
                config_dir: boot.config_dir,
                skip_detect: boot.skip_detect,
            })
            .expect("boot dd_pantheon")
        };
        let palette = theme::palette_from_theme(&app.state.theme);
        let gui = prefs::load_from(&boot.gui_path);
        let quit_runtime = boot.quit_runtime;
        let mut shell = Self {
            focus: cx.focus_handle(),
            app,
            palette,
            gui,
            gui_path: boot.gui_path,
            project_root: boot.project_root,
            appearance: None,
            settings_nav: 0,
            settings_row: 0,
            settings_query: String::new(),
            settings_query_on: false,
            quit: false,
            quit_runtime,
            note: String::new(),
            settings_scroll: ScrollHandle::new(),
            tree_scroll: ScrollHandle::new(),
            booted_at: Instant::now(),
        };
        shell.arm_areas();
        if quit_runtime {
            shell.spawn_tick(cx);
        }
        shell
    }

    /// Page keys use the terminal's pane height. The window has no cell grid.
    fn arm_areas(&mut self) {
        self.app.state.tree_area.width = 40;
        self.app.state.tree_area.height = 24;
        self.app.state.inspector_area.width = 64;
        self.app.state.inspector_area.height = 18;
        self.app.state.preview_area.width = 64;
        self.app.state.preview_area.height = 10;
        self.app.state.log_area.width = 64;
        self.app.state.log_area.height = 12;
        let mut modal = self.app.state.tree_area;
        modal.width = 80;
        modal.height = 24;
        self.app.state.modal_area = Some(modal);
    }

    fn spawn_tick(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                let wait = this
                    .update(cx, |shell, _| {
                        shell
                            .app
                            .next_poll_timeout()
                            .unwrap_or(Duration::from_secs(60))
                    })
                    .unwrap_or(Duration::from_secs(60));
                cx.background_executor().timer(wait).await;
                let stop = this
                    .update(cx, |shell, cx| {
                        if shell.app.tick() {
                            cx.notify();
                        }
                        shell.quit
                    })
                    .unwrap_or(true);
                if stop {
                    break;
                }
            }
        })
        .detach();
    }

    #[cfg(test)]
    pub fn palette_hex(&self, key: &str) -> String {
        self.palette
            .get(key)
            .map(|rgb| rgb.to_hex())
            .unwrap_or_else(|| "#000000".to_string())
    }

    #[cfg(test)]
    pub fn selected_label(&self) -> String {
        let idx = self.app.state.tree_state.selected().unwrap_or(0);
        self.app
            .state
            .tree_rows
            .get(idx)
            .map(|row| match &row.env {
                Some(env) => format!("{} {env}", row.site),
                None => row.site.clone(),
            })
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub fn visible_text(&self) -> String {
        let mut lines = present::cockpit(&self.app.state, &self.footer_line());
        if self.app.state.job_running {
            lines.insert(1, busy_label(self.booted_at.elapsed()));
        }
        if !self.note.is_empty() {
            lines.insert(1, self.note.clone());
        }
        if self.appearance.is_some() {
            lines.extend(self.settings_lines());
        }
        lines.join("\n")
    }

    fn footer_line(&self) -> String {
        if self.appearance.is_some() {
            "F1: Settings  Esc: Close  Ctrl+Q: Quit".to_string()
        } else {
            dd_pantheon::ui::footer_text(&self.app.state, 160)
        }
    }

    fn tree_lines(&self) -> Vec<String> {
        present::tree_lines(&self.app.state)
    }

    #[cfg(test)]
    fn settings_lines(&self) -> Vec<String> {
        let mut lines = vec![
            "settings-panel:right:70".to_string(),
            "settings-nav:Themes,Window,Help".to_string(),
        ];
        let page = if !self.settings_query.trim().is_empty() {
            "Results"
        } else {
            SETTINGS_NAV
                .get(self.settings_nav as usize)
                .copied()
                .unwrap_or("Themes")
        };
        lines.push(format!("settings-page:{page}"));
        lines.push("Settings".to_string());
        for (index, label) in SETTINGS_NAV.iter().enumerate() {
            let mark =
                if self.settings_query.trim().is_empty() && self.settings_nav as usize == index {
                    ">"
                } else {
                    " "
                };
            lines.push(format!("{mark} {label}"));
        }
        if !self.settings_query.trim().is_empty() {
            lines.push("Results".to_string());
            lines.extend(self.settings_results());
            return lines;
        }
        match self.settings_nav {
            1 => {
                lines.push(format!("Density {}", self.gui.density.label()));
                lines.push(format!(
                    "Vim keys {}",
                    if self.gui.vim_keys { "on" } else { "off" }
                ));
            }
            2 => lines.extend(shortcuts::lines()),
            _ => lines.extend(self.theme_lines()),
        }
        lines
    }

    #[cfg(test)]
    fn theme_lines(&self) -> Vec<String> {
        let Some(appearance) = &self.appearance else {
            return Vec::new();
        };
        let mut lines = Vec::new();
        lines.push(format!("Source: {}", self.app.state.theme.source.label()));
        let status = self
            .app
            .state
            .theme
            .warning
            .clone()
            .unwrap_or_else(|| "OK (loaded cleanly)".to_string());
        lines.push(format!("Status: {status}"));
        lines.push(format!("Save to {}", appearance.picker.save_target.label()));
        if let Some(editor) = &appearance.editor {
            lines.push("Theme color editor".to_string());
            for (index, field) in editor.fields.iter().enumerate() {
                let hex = editor
                    .palette
                    .get(field.key)
                    .map(|color| color.to_hex())
                    .unwrap_or_else(|| "#000000".to_string());
                let mark = if index == editor.selected { ">" } else { " " };
                lines.push(format!("{mark} {} {hex}", field.key));
            }
            if let Some(err) = &appearance.hex_error {
                lines.push(err.clone());
            }
        } else {
            lines.push("theme-list:flow".to_string());
            for (index, row) in appearance.picker.rows.iter().enumerate() {
                let mark = if index == appearance.picker.selected {
                    ">"
                } else {
                    " "
                };
                lines.push(format!("{mark} {}", theme_row_label(row)));
            }
            lines.push("Edit colors…".to_string());
        }
        lines.push("Save".to_string());
        lines
    }

    pub fn handle_key(&mut self, key: &str) {
        let key = key.to_ascii_lowercase();
        if key == "ctrl-q" {
            self.forward(&key);
            self.sync_quit();
            return;
        }
        // A gate, confirm, or any other terminal modal owns the key. Settings
        // must not swallow LiveGate, a destructive confirm, or the quit prompt.
        if self.app.state.modal.is_some() {
            self.forward(&key);
            self.sync_quit();
            return;
        }
        if self.appearance.is_some() {
            self.settings_key(&key);
            return;
        }
        if matches!(key.as_str(), "f1" | "f2" | "ctrl-/" | "ctrl-,") {
            self.open_settings();
            return;
        }
        if key == "?" || key == "shift-/" {
            self.open_settings();
            self.settings_nav = 2;
            return;
        }
        if !self.gui.vim_keys && vim_motion(&key) {
            return;
        }
        self.forward(&key);
        self.sync_quit();
    }

    pub fn paste(&mut self, raw: &str) {
        if self.appearance.is_some() {
            if self.editing_hex() {
                for ch in raw.chars().filter(|ch| ch.is_ascii_hexdigit()) {
                    self.editor_key(&ch.to_ascii_lowercase().to_string());
                }
            } else if self.settings_query_on {
                for ch in raw.chars().filter(|ch| !ch.is_control()) {
                    self.settings_query.push(ch);
                }
                self.settings_row = 0;
            }
            return;
        }
        if let Err(err) = self.app.paste_text(raw) {
            self.note = err.to_string();
        }
    }

    fn forward(&mut self, key: &str) {
        let Some(event) = keys::to_key_event(key) else {
            return;
        };
        match self.app.handle_key(event) {
            Ok(quit) => {
                if quit {
                    self.app.state.should_quit = true;
                    self.quit = true;
                }
            }
            Err(err) => self.note = err.to_string(),
        }
    }

    fn sync_quit(&mut self) {
        if self.app.state.should_quit {
            self.quit = true;
        }
    }

    /// Same exclusion list as the terminal's shared letter keys.
    pub fn activate_action(&mut self, id: &str) {
        let staged = dd_pantheon::workflows::stage_action(&mut self.app.state, id);
        if staged && !matches!(id, "r" | "n" | "m" | "a" | "cms" | "create") {
            dd_pantheon::workflows::request_run(&mut self.app.state);
        }
        self.sync_quit();
    }

    pub fn preview_click(&mut self, button: &str) {
        self.app.state.focus = FocusPane::Preview;
        match button {
            "process" => dd_pantheon::workflows::request_run(&mut self.app.state),
            "copy" => self.forward("y"),
            "cancel" => {
                dd_pantheon::workflows::cancel_preview_or_job(&mut self.app.state);
            }
            _ => {}
        }
    }

    /// Palette field rows and the CMS command take a click the way the terminal does.
    pub fn click_modal_line(&mut self, index: usize) {
        match &mut self.app.state.modal {
            Some(Modal::Palette {
                form: Some(form), ..
            }) => {
                let field = index.saturating_sub(1);
                if field < form.field_count() {
                    form.focus = field;
                }
            }
            Some(Modal::Cms { form }) => {
                form.focus = dd_pantheon::state::CmsFocus::Command;
            }
            _ => {}
        }
    }

    /// Body pins the tag filter. The × on the selected chip stages removal.
    pub fn click_tag(&mut self, name: &str, close: bool) {
        self.focus_pane(FocusPane::Inspector);
        if close {
            dd_pantheon::workflows::tags::stage_remove(&mut self.app.state, name);
        } else {
            self.app.state.selected_chip = Some(name.to_string());
            dd_pantheon::workflows::tags::pin_filter(&mut self.app.state, name.to_string());
        }
    }

    fn window_inspector(&mut self) -> Vec<String> {
        let lines: Vec<String> = present::inspector_lines(&self.app.state)
            .into_iter()
            .filter(|line| !line.starts_with("tags ["))
            .collect();
        let rows = self.app.state.inspector_area.height.max(1);
        let clamped = present::clamp_scroll(self.app.state.inspector_scroll, lines.len(), rows);
        self.app.state.inspector_scroll = clamped;
        present::page_lines(&lines, clamped, rows)
    }

    fn window_preview(&mut self) -> Vec<String> {
        let lines = present::preview_lines(&self.app.state);
        let rows = self.app.state.preview_area.height.max(1);
        let clamped = present::clamp_scroll(self.app.state.preview_scroll, lines.len(), rows);
        self.app.state.preview_scroll = clamped;
        present::page_lines(&lines, clamped, rows)
    }

    fn page_modal(&mut self, lines: Vec<String>) -> Vec<String> {
        let Some(scroll) = self.app.state.modal.as_ref().and_then(|modal| match modal {
            Modal::Doctor { scroll } => Some(*scroll),
            _ => None,
        }) else {
            return lines;
        };
        let rows = self
            .app
            .state
            .modal_area
            .map(|area| area.height)
            .unwrap_or(24)
            .max(1);
        let body_rows = rows.saturating_sub(1).max(1);
        let body_len = lines.len().saturating_sub(1);
        let clamped = present::clamp_scroll(scroll, body_len, body_rows);
        if let Some(Modal::Doctor { scroll }) = &mut self.app.state.modal {
            *scroll = clamped;
        }
        if clamped == 0 || lines.is_empty() {
            return lines;
        }
        let mut lines = lines;
        let title = lines.remove(0);
        let mut out = vec![title];
        out.extend(present::page_lines(&lines, clamped, body_rows));
        out
    }

    pub fn dismiss_outside(&mut self) {
        if self
            .app
            .state
            .modal
            .as_ref()
            .is_some_and(Modal::dismiss_on_click_outside)
        {
            self.app.state.modal = None;
        }
    }

    fn focus_pane(&mut self, pane: FocusPane) {
        self.app.state.focus = pane;
    }

    fn wheel(&mut self, pane: FocusPane, down: bool) {
        let state = &mut self.app.state;
        match pane {
            FocusPane::Tree => state.move_tree(if down { 1 } else { -1 }),
            FocusPane::Inspector => {
                if down {
                    state.inspector_scroll = state.inspector_scroll.saturating_add(1);
                } else {
                    state.inspector_scroll = state.inspector_scroll.saturating_sub(1);
                }
            }
            FocusPane::Preview => {
                if down {
                    state.preview_scroll = state.preview_scroll.saturating_add(1);
                } else {
                    state.preview_scroll = state.preview_scroll.saturating_sub(1);
                }
            }
            FocusPane::Log => {
                state.focus = FocusPane::Log;
                let vis = state.log_visible_rows();
                let max = (state.log_lines.len() as u16).saturating_sub(vis);
                if down {
                    state.log_scroll = state.log_scroll.saturating_add(1).min(max);
                } else {
                    state.log_scroll = state.log_scroll.saturating_sub(1);
                }
                state.log_follow = state.log_scroll >= max;
            }
        }
    }

    #[cfg(test)]
    pub fn quit_requested(&self) -> bool {
        self.quit
    }

    #[cfg(test)]
    pub fn vim_keys(&self) -> bool {
        self.gui.vim_keys
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = chord(event);
        if key == "ctrl-v" {
            if let Some(item) = cx.read_from_clipboard()
                && let Some(text) = item.text()
            {
                self.paste(&text);
            }
        } else {
            self.handle_key(&key);
        }
        if self.quit && self.quit_runtime {
            cx.quit();
        }
    }

    fn open_settings(&mut self) {
        if self.appearance.is_some() {
            return;
        }
        let palette = self.palette.clone();
        let picker = ThemePicker::new(
            palette,
            theme::extra_theme_fields(),
            self.app.state.omarchy_paths.clone(),
        );
        let direct = !picker.lists_omarchy();
        let mut appearance = Appearance {
            picker,
            editor: None,
            direct_editor: direct,
            hex_error: None,
        };
        if direct {
            appearance.editor = Some(editor_from(&appearance));
        }
        self.settings_nav = 0;
        self.settings_row = 0;
        self.settings_query.clear();
        self.settings_query_on = false;
        self.appearance = Some(appearance);
        self.reveal_theme();
    }

    fn revert_settings(&mut self) {
        if let Some(appearance) = self.appearance.take() {
            self.install_palette(appearance.picker.revert_palette());
        }
        self.settings_nav = 0;
        self.settings_query.clear();
        self.settings_query_on = false;
    }

    fn install_palette(&mut self, palette: ldnddev_theme::Palette) {
        self.app.state.theme = theme::theme_from_palette(palette.clone());
        self.palette = palette;
    }

    fn sync_picker_palette(&mut self) {
        let Some(palette) = self
            .appearance
            .as_ref()
            .map(|appearance| appearance.picker.palette.clone())
        else {
            return;
        };
        self.install_palette(palette);
    }

    fn settings_key(&mut self, key: &str) {
        if matches!(key, "f1" | "f2" | "ctrl-/" | "ctrl-,") {
            self.revert_settings();
            return;
        }
        if self.settings_query_on {
            self.query_key(key);
            return;
        }
        let editing_hex = self.editing_hex();
        if key == "/" && !editing_hex {
            self.settings_query_on = true;
            self.settings_query.clear();
            self.settings_row = 0;
            return;
        }
        if let Some(dir) = section_dir(key, editing_hex) {
            self.jump_section(dir);
            return;
        }
        match self.settings_nav {
            0 => self.themes_key(key),
            1 => self.window_key(key),
            _ => {
                if key == "escape" {
                    self.escape_settings();
                }
            }
        }
    }

    fn themes_key(&mut self, key: &str) {
        let editing = self.editing_hex();
        if key == "t" && !editing {
            self.toggle_save_target();
            return;
        }
        if self.editor_open() {
            self.editor_key(key);
            return;
        }
        let vim = self.gui.vim_keys;
        match key {
            "escape" => self.revert_settings(),
            "enter" => self.apply_theme(),
            "e" => self.open_editor(),
            "s" | "y" => self.save_theme(),
            "up" => self.move_theme(-1),
            "down" => self.move_theme(1),
            "pageup" => self.move_theme(-8),
            "pagedown" => self.move_theme(8),
            "k" if vim => self.move_theme(-1),
            "j" if vim => self.move_theme(1),
            "g" if vim => self.move_theme_edge(true),
            "shift-g" if vim => self.move_theme_edge(false),
            _ => {}
        }
    }

    fn window_key(&mut self, key: &str) {
        let vim = self.gui.vim_keys;
        let last = 1usize;
        match key {
            "escape" => self.revert_settings(),
            "up" => self.settings_row = self.settings_row.saturating_sub(1),
            "down" => self.settings_row = (self.settings_row + 1).min(last),
            "k" if vim => self.settings_row = self.settings_row.saturating_sub(1),
            "j" if vim => self.settings_row = (self.settings_row + 1).min(last),
            "g" if vim => self.settings_row = 0,
            "shift-g" if vim => self.settings_row = last,
            "enter" => self.activate_window_row(),
            _ => {}
        }
    }

    fn escape_settings(&mut self) {
        if self.settings_query_on {
            self.settings_query_on = false;
            self.settings_query.clear();
            self.settings_row = 0;
            return;
        }
        if self.editor_open() {
            self.editor_key("escape");
            return;
        }
        self.revert_settings();
    }

    fn query_key(&mut self, key: &str) {
        match key {
            "escape" => {
                self.settings_query_on = false;
                self.settings_query.clear();
                self.settings_row = 0;
            }
            "backspace" => {
                self.settings_query.pop();
                self.settings_row = 0;
            }
            "enter" => self.activate_settings_result(),
            "up" | "k" => self.settings_row = self.settings_row.saturating_sub(1),
            "down" | "j" => {
                let count = self.settings_results().len();
                if count > 0 {
                    self.settings_row = (self.settings_row + 1).min(count - 1);
                }
            }
            other if is_text(other) => {
                self.settings_query.push_str(other);
                self.settings_row = 0;
            }
            _ => {}
        }
    }

    fn settings_results(&self) -> Vec<String> {
        let query = self.settings_query.trim().to_ascii_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        let mut hits = Vec::new();
        if let Some(appearance) = &self.appearance {
            for row in &appearance.picker.rows {
                let label = theme_row_label(row);
                if label.to_ascii_lowercase().contains(&query) {
                    hits.push(label);
                }
            }
        }
        for label in ["Density", "Vim keys"] {
            if label.to_ascii_lowercase().contains(&query) {
                hits.push(label.to_string());
            }
        }
        for row in shortcuts::rows() {
            if row.action.to_ascii_lowercase().contains(&query) {
                hits.push(row.action.to_string());
            }
        }
        hits
    }

    fn activate_settings_result(&mut self) {
        let Some(label) = self.settings_results().get(self.settings_row).cloned() else {
            return;
        };
        self.settings_query_on = false;
        self.settings_query.clear();
        if label == "Density" || label == "Vim keys" {
            self.settings_nav = 1;
            self.settings_row = usize::from(label == "Vim keys");
            self.activate_window_row();
            return;
        }
        if let Some(appearance) = &self.appearance
            && let Some(index) = appearance
                .picker
                .rows
                .iter()
                .position(|row| theme_row_label(row) == label)
        {
            self.settings_nav = 0;
            self.choose_theme(index);
            return;
        }
        self.settings_nav = 2;
        self.settings_row = 0;
    }

    fn activate_window_row(&mut self) {
        if self.settings_row == 0 {
            self.set_density(self.gui.density.toggled());
        } else {
            self.set_vim_keys(!self.gui.vim_keys);
        }
    }

    pub fn set_density(&mut self, density: Density) {
        self.gui.density = density;
        let _ = prefs::save_to(&self.gui_path, &self.gui);
    }

    pub fn set_vim_keys(&mut self, on: bool) {
        self.gui.vim_keys = on;
        let _ = prefs::save_to(&self.gui_path, &self.gui);
    }

    fn jump_section(&mut self, dir: i32) {
        let last = (SETTINGS_NAV.len() - 1) as i32;
        let next = (self.settings_nav as i32 + dir).clamp(0, last) as u8;
        if next != self.settings_nav {
            self.settings_nav = next;
            self.settings_row = 0;
            self.reveal_theme();
        }
    }

    fn editing_hex(&self) -> bool {
        self.appearance
            .as_ref()
            .and_then(|appearance| appearance.editor.as_ref())
            .is_some_and(|editor| editor.editing_hex)
    }

    fn editor_open(&self) -> bool {
        self.appearance
            .as_ref()
            .is_some_and(|appearance| appearance.editor.is_some())
    }

    fn toggle_save_target(&mut self) {
        let Some(appearance) = &mut self.appearance else {
            return;
        };
        appearance.picker.save_target = appearance.picker.save_target.toggled();
        let target = appearance.picker.save_target;
        if let Some(editor) = &mut appearance.editor {
            editor.save_target = target;
        }
    }

    fn choose_theme(&mut self, index: usize) {
        if self.editor_open() {
            return;
        }
        if let Some(appearance) = &mut self.appearance {
            appearance.picker.select(index);
        }
        self.apply_theme();
        self.reveal_theme();
    }

    fn move_theme(&mut self, delta: i32) {
        let moved = {
            let Some(appearance) = &mut self.appearance else {
                return;
            };
            if appearance.editor.is_some() {
                return;
            }
            let len = appearance.picker.rows.len() as i32;
            if len == 0 {
                return;
            }
            let next = (appearance.picker.selected as i32 + delta).clamp(0, len - 1) as usize;
            appearance.picker.select(next);
            appearance.picker.preview_selected()
        };
        self.finish_picker(moved);
        self.reveal_theme();
    }

    fn move_theme_edge(&mut self, first: bool) {
        let outcome = {
            let Some(appearance) = &mut self.appearance else {
                return;
            };
            let last = appearance.picker.rows.len().saturating_sub(1);
            appearance.picker.select(if first { 0 } else { last });
            appearance.picker.preview_selected()
        };
        self.finish_picker(outcome);
        self.reveal_theme();
    }

    fn apply_theme(&mut self) {
        let outcome = {
            let Some(appearance) = &mut self.appearance else {
                return;
            };
            if appearance.editor.is_some() {
                return;
            }
            appearance.picker.handle(EditorKey::Enter)
        };
        self.finish_picker(outcome);
    }

    fn finish_picker(&mut self, outcome: PickerOutcome) {
        match outcome {
            PickerOutcome::Applied => self.sync_picker_palette(),
            PickerOutcome::Error(message) => self.note = message,
            PickerOutcome::EditColors => self.open_editor(),
            PickerOutcome::RequestSave => self.save_theme(),
            PickerOutcome::Closed { .. } => self.revert_settings(),
            PickerOutcome::None => {}
        }
    }

    fn reveal_theme(&self) {
        if self.settings_nav != 0 || self.settings_query_on {
            return;
        }
        let Some(index) = self.appearance.as_ref().and_then(|appearance| {
            if appearance.editor.is_some() {
                None
            } else {
                Some(appearance.picker.selected)
            }
        }) else {
            return;
        };
        self.settings_scroll.scroll_to_item(index);
    }

    fn open_editor(&mut self) {
        let Some(appearance) = &mut self.appearance else {
            return;
        };
        if appearance.editor.is_some() {
            return;
        }
        appearance.editor = Some(editor_from(appearance));
        appearance.hex_error = None;
    }

    fn editor_key(&mut self, key: &str) {
        let shift = key.starts_with("shift-");
        let bare = key.strip_prefix("shift-").unwrap_or(key);
        let vim = self.gui.vim_keys;
        let editor_key = match bare {
            "escape" => Some(EditorKey::Esc),
            "enter" => Some(EditorKey::Enter),
            "backspace" => Some(EditorKey::Backspace),
            "up" => Some(EditorKey::Up),
            "down" => Some(EditorKey::Down),
            "left" => Some(EditorKey::Left),
            "right" => Some(EditorKey::Right),
            "j" if vim => Some(EditorKey::Char('j')),
            "k" if vim => Some(EditorKey::Char('k')),
            "s" | "y" => Some(EditorKey::Char('s')),
            "r" if shift => Some(EditorKey::Char('R')),
            "g" if shift => Some(EditorKey::Char('G')),
            "+" | "=" => Some(EditorKey::Char('+')),
            "-" | "_" => Some(EditorKey::Char('-')),
            "[" => Some(EditorKey::Char('[')),
            "]" => Some(EditorKey::Char(']')),
            other if is_text(other) => other.chars().next().map(EditorKey::Char),
            _ => None,
        };
        let Some(editor_key) = editor_key else {
            return;
        };
        let outcome = {
            let Some(editor) = self
                .appearance
                .as_mut()
                .and_then(|appearance| appearance.editor.as_mut())
            else {
                return;
            };
            editor.handle(editor_key, shift)
        };
        match outcome {
            EditorOutcome::RequestSave => self.save_theme(),
            EditorOutcome::Closed { .. } => self.close_editor(),
            EditorOutcome::HexError(message) => {
                if let Some(appearance) = &mut self.appearance {
                    appearance.hex_error = Some(message);
                }
            }
            EditorOutcome::PaletteChanged => {
                if let Some(appearance) = &mut self.appearance {
                    if let Some(editor) = &appearance.editor {
                        appearance.picker.palette = editor.palette.clone();
                        appearance.picker.save_target = editor.save_target;
                    }
                    appearance.hex_error = None;
                }
                self.sync_picker_palette();
            }
            EditorOutcome::None => {
                if let Some(appearance) = &mut self.appearance
                    && appearance
                        .editor
                        .as_ref()
                        .is_some_and(|editor| editor.editing_hex)
                {
                    appearance.hex_error = None;
                }
            }
        }
    }

    fn close_editor(&mut self) {
        let direct = self
            .appearance
            .as_ref()
            .is_some_and(|appearance| appearance.direct_editor);
        if let Some(appearance) = &mut self.appearance {
            if let Some(editor) = appearance.editor.take() {
                appearance.picker.palette = editor.palette;
                appearance.picker.save_target = editor.save_target;
            }
            appearance.hex_error = None;
        }
        self.sync_picker_palette();
        if direct {
            self.revert_settings();
        }
    }

    fn save_theme(&mut self) {
        let Some(appearance) = &self.appearance else {
            return;
        };
        let palette = appearance
            .editor
            .as_ref()
            .map(|editor| editor.palette.clone())
            .unwrap_or_else(|| appearance.picker.palette.clone());
        let target = appearance
            .editor
            .as_ref()
            .map(|editor| editor.save_target)
            .unwrap_or(appearance.picker.save_target);
        let fields = theme_fields();
        match ldnddev_theme::save_theme(
            &palette,
            &self.project_root,
            THEME_FILE_NAME,
            target,
            ldnddev_theme::default_config_home().as_deref(),
            &fields,
        ) {
            Ok(path) => {
                self.note = format!("Saved {} theme to {}", target.label(), path.display());
                let mut palette = palette;
                palette.source = match target {
                    ThemeSaveTarget::Local => ldnddev_theme::ThemeSource::Local,
                    ThemeSaveTarget::Global => ldnddev_theme::ThemeSource::Global,
                };
                self.install_palette(palette);
                self.appearance = None;
            }
            Err(err) => self.note = format!("Save failed: {err}"),
        }
    }

    fn paint(&self) -> Paint {
        Paint {
            base: color(&self.palette, "base_background"),
            body: color(&self.palette, "body_background"),
            raised: color(&self.palette, "modal_background"),
            text: color(&self.palette, "text_primary"),
            secondary: color(&self.palette, "text_secondary"),
            border: color(&self.palette, "border_default"),
            selected: color(&self.palette, "selected_background"),
            accent: color(&self.palette, "text_active_focus"),
        }
    }

    fn row_h(&self) -> f32 {
        self.gui.density.row_px()
    }
}

impl Focusable for Shell {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focus.is_focused(window) {
            self.focus.focus(window, cx);
        }
        let paint = self.paint();
        let band = self.row_h();
        let footer = self.footer_line();
        let tree = self.tree_lines();
        let selected = self.app.state.tree_state.selected();
        if let Some(index) = selected {
            self.tree_scroll.scroll_to_item(index);
        }
        let busy = self
            .app
            .state
            .job_running
            .then(|| busy_label(self.booted_at.elapsed()));
        let status = present::header_status(&self.app.state);
        let demo = self.app.state.demo;
        let focus = self.app.state.focus;
        let tree_title = present::tree_title(&self.app.state);
        let chips = present::tag_chips(&self.app.state);
        let inspector = self.window_inspector();
        let preview = self.window_preview();
        let log_lines = present::log_lines(&self.app.state);
        let modal = present::modal_lines(&self.app.state).map(|lines| self.page_modal(lines));
        let toast = self
            .app
            .state
            .toast
            .as_ref()
            .map(|toast| toast.message.clone());
        let actions: Vec<(String, String)> = self
            .app
            .state
            .actions
            .iter()
            .map(|action| (action.id.to_string(), action.label.to_string()))
            .collect();
        let period = self.app.state.metrics_period;
        let settings = self.appearance.is_some();
        let nav = self.settings_nav;
        let query = self.settings_query.clone();
        let query_on = self.settings_query_on;
        let page = settings_page_name(nav, &query);
        let hint = settings_hint(self);
        let theme_rows = self.theme_row_views();
        let color_rows = self.color_row_views();
        let help = shortcuts::lines();
        let results = self.settings_results();
        let density = self.gui.density.label().to_string();
        let vim_on = self.gui.vim_keys;
        let window_row = self.settings_row;
        let scroll = self.settings_scroll.clone();
        let tree_scroll = self.tree_scroll.clone();

        div()
            .id("shell")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|shell, event: &KeyDownEvent, _, cx| {
                shell.on_key_down(event, cx);
                cx.notify();
            }))
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(paint.base)
            .text_color(paint.text)
            .text_size(px(13.0))
            .child(header_bar(
                band,
                busy,
                status,
                demo,
                paint.raised,
                paint.text,
                paint.border,
                paint.accent,
            ))
            .child(cockpit_body(
                cx,
                tree_title,
                tree,
                selected,
                inspector,
                chips,
                preview,
                log_lines,
                actions,
                period,
                focus,
                band,
                &paint,
                &tree_scroll,
            ))
            .children(toast.map(|message| toast_bar(message, band, paint.accent, paint.base)))
            .child(footer_bar(
                &footer,
                band,
                paint.raised,
                paint.secondary,
                paint.border,
            ))
            .children(modal.map(|lines| {
                modal_layer(
                    cx,
                    lines,
                    band,
                    paint.raised,
                    paint.text,
                    paint.border,
                    paint.accent,
                )
            }))
            .when(settings, |root| {
                root.child(settings_panel(
                    cx,
                    page,
                    hint,
                    query,
                    query_on,
                    nav,
                    theme_rows,
                    color_rows,
                    help,
                    results,
                    density,
                    vim_on,
                    window_row,
                    band,
                    paint.raised,
                    paint.text,
                    paint.secondary,
                    paint.border,
                    paint.selected,
                    paint.accent,
                    scroll,
                ))
            })
    }
}

fn theme_row_label(row: &PickerRow) -> String {
    match row {
        PickerRow::Follow { name } => format!("Follow desktop {name}"),
        PickerRow::Theme(theme) => theme.name.clone(),
        PickerRow::Custom => "Custom".to_string(),
    }
}

fn editor_from(appearance: &Appearance) -> ThemeEditor {
    let mut editor = ThemeEditor::new(
        appearance.picker.palette.clone(),
        theme::extra_theme_fields(),
    );
    editor.save_target = appearance.picker.save_target;
    editor
}

fn theme_fields() -> Vec<ldnddev_theme::ColorField> {
    let mut fields = ldnddev_theme::COLOR_FIELDS.to_vec();
    for extra in theme::extra_theme_fields() {
        if !fields.iter().any(|field| field.key == extra.key) {
            fields.push(*extra);
        }
    }
    fields
}

fn section_dir(key: &str, editing_hex: bool) -> Option<i32> {
    match key {
        "tab" => Some(1),
        "shift-tab" | "backtab" => Some(-1),
        "]" if !editing_hex => Some(1),
        "[" if !editing_hex => Some(-1),
        _ => None,
    }
}

fn settings_page_name(nav: u8, query: &str) -> String {
    if !query.trim().is_empty() {
        return "Results".to_string();
    }
    SETTINGS_NAV
        .get(nav as usize)
        .copied()
        .unwrap_or("Themes")
        .to_string()
}

fn settings_hint(shell: &Shell) -> String {
    if shell.settings_query_on {
        return "type to filter    up/down move    Enter    Esc clear".to_string();
    }
    if shell.settings_nav == 0 && shell.editor_open() {
        let target = shell
            .appearance
            .as_ref()
            .map(|appearance| appearance.picker.save_target.label())
            .unwrap_or("global");
        return format!("j/k move    Tab section    y save {target}    t target    Esc back");
    }
    if shell.settings_nav == 2 {
        return "Tab section    Esc close".to_string();
    }
    if shell.settings_nav == 0 {
        return "j/k move    Enter    / search    Tab section    y save    t target    Esc revert"
            .to_string();
    }
    "j/k move    Enter    / search    Tab section    Esc close".to_string()
}

struct ThemeRowView {
    index: usize,
    label: String,
    selected: bool,
}

struct ColorRowView {
    index: usize,
    label: String,
    selected: bool,
}

impl Shell {
    fn theme_row_views(&self) -> Vec<ThemeRowView> {
        let Some(appearance) = &self.appearance else {
            return Vec::new();
        };
        if appearance.editor.is_some()
            || self.settings_nav != 0
            || !self.settings_query.trim().is_empty()
        {
            return Vec::new();
        }
        appearance
            .picker
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| ThemeRowView {
                index,
                label: theme_row_label(row),
                selected: index == appearance.picker.selected,
            })
            .collect()
    }

    fn color_row_views(&self) -> Vec<ColorRowView> {
        let Some(appearance) = &self.appearance else {
            return Vec::new();
        };
        if self.settings_nav != 0 || !self.settings_query.trim().is_empty() {
            return Vec::new();
        }
        let Some(editor) = &appearance.editor else {
            return Vec::new();
        };
        editor
            .fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let hex = editor
                    .palette
                    .get(field.key)
                    .map(|color| color.to_hex())
                    .unwrap_or_else(|| "#000000".to_string());
                ColorRowView {
                    index,
                    label: format!("{} {hex}", field.key),
                    selected: index == editor.selected,
                }
            })
            .collect()
    }
}

fn color(palette: &ldnddev_theme::Palette, key: &str) -> Hsla {
    let rgb = palette
        .get(key)
        .or_else(|| ldnddev_theme::Palette::builtin().get(key))
        .unwrap_or(ldnddev_theme::Rgb::new(255, 255, 255));
    hsla(rgb)
}

fn hsla(color: ldnddev_theme::Rgb) -> Hsla {
    Rgba {
        r: f32::from(color.r) / 255.0,
        g: f32::from(color.g) / 255.0,
        b: f32::from(color.b) / 255.0,
        a: 1.0,
    }
    .into()
}

fn chord(event: &KeyDownEvent) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mods = event.keystroke.modifiers;
    if mods.control || mods.platform {
        parts.push("ctrl".to_string());
    }
    if mods.shift {
        parts.push("shift".to_string());
    }
    if mods.alt {
        parts.push("alt".to_string());
    }
    parts.push(event.keystroke.key.to_ascii_lowercase());
    parts.join("-")
}

fn is_text(key: &str) -> bool {
    key.chars().count() == 1 && !key.chars().any(|ch| ch.is_control())
}

fn header_bar(
    band: f32,
    busy: Option<String>,
    status: String,
    demo: bool,
    raised: Hsla,
    text: Hsla,
    border: Hsla,
    accent: Hsla,
) -> gpui::Div {
    div()
        .h(px(band))
        .flex()
        .flex_shrink_0()
        .items_center()
        .px(px(8.0))
        .gap(px(12.0))
        .bg(raised)
        .text_color(text)
        .border_b_1()
        .border_color(border)
        .font_weight(FontWeight::BOLD)
        .child("dd_pantheon")
        .when(demo, |bar| bar.child("DEMO"))
        .when(!status.is_empty(), |bar| bar.child(status))
        .child(div().flex_1())
        .children(busy.map(|label| div().text_color(accent).child(label)))
}

fn footer_bar(line: &str, band: f32, raised: Hsla, secondary: Hsla, border: Hsla) -> gpui::Div {
    div()
        .h(px(band))
        .flex()
        .flex_shrink_0()
        .items_center()
        .px(px(8.0))
        .bg(raised)
        .text_color(secondary)
        .border_t_1()
        .border_color(border)
        .child(line.to_string())
}

fn site_tree(
    cx: &mut Context<Shell>,
    rows: Vec<String>,
    selected: Option<usize>,
    band: f32,
    paint: &Paint,
    scroll: &ScrollHandle,
) -> gpui::Stateful<gpui::Div> {
    let mut list = div()
        .id("site-tree")
        .flex_1()
        .min_h(px(0.0))
        .flex_basis(px(0.0))
        .overflow_y_scroll()
        .track_scroll(scroll)
        .flex()
        .flex_col()
        .bg(paint.body)
        .text_color(paint.text);
    if rows.is_empty() {
        list = list.child(
            div()
                .px(px(8.0))
                .h(px(band))
                .flex()
                .items_center()
                .text_color(paint.secondary)
                .child("no sites"),
        );
    }
    for (index, row) in rows.into_iter().enumerate() {
        let on = selected == Some(index);
        list = list.child(
            div()
                .id(("site-row", index))
                .h(px(band))
                .px(px(8.0))
                .flex()
                .items_center()
                .overflow_hidden()
                .whitespace_nowrap()
                .cursor_pointer()
                .when(on, |row| row.bg(paint.selected).text_color(paint.accent))
                .on_click(cx.listener(move |shell, _: &ClickEvent, _, cx| {
                    shell.focus_pane(FocusPane::Tree);
                    shell.app.state.tree_state.select(Some(index));
                    shell.app.state.apply_row_selection(index);
                    cx.stop_propagation();
                    cx.notify();
                }))
                .child(row),
        );
    }
    list
}

fn cockpit_body(
    cx: &mut Context<Shell>,
    tree_title: String,
    tree: Vec<String>,
    selected: Option<usize>,
    inspector: Vec<String>,
    chips: Vec<present::TagChip>,
    preview: Vec<String>,
    log_lines: Vec<String>,
    actions: Vec<(String, String)>,
    period: dd_pantheon::models::MetricsPeriod,
    focus: FocusPane,
    band: f32,
    paint: &Paint,
    tree_scroll: &ScrollHandle,
) -> gpui::Div {
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_row()
        .overflow_hidden()
        .child(
            div()
                .w(relative(0.34))
                .h_full()
                .flex()
                .flex_col()
                .border_r_1()
                .border_color(paint.border)
                .child(pane_title(
                    &tree_title,
                    focus == FocusPane::Tree,
                    band,
                    paint,
                ))
                .child(site_tree(cx, tree, selected, band, paint, tree_scroll)),
        )
        .child(
            div()
                .flex_1()
                .h_full()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .overflow_hidden()
                .child(text_pane(
                    cx,
                    "inspector",
                    "Inspector",
                    focus == FocusPane::Inspector,
                    inspector,
                    chips,
                    FocusPane::Inspector,
                    band,
                    paint,
                ))
                .child(action_row(cx, actions, period, band, paint))
                .child(text_pane(
                    cx,
                    "preview",
                    "Command preview",
                    focus == FocusPane::Preview,
                    preview,
                    Vec::new(),
                    FocusPane::Preview,
                    band,
                    paint,
                ))
                .child(preview_buttons(cx, band, paint))
                .child(text_pane(
                    cx,
                    "log",
                    "Log",
                    focus == FocusPane::Log,
                    log_lines,
                    Vec::new(),
                    FocusPane::Log,
                    band,
                    paint,
                )),
        )
}

fn pane_title(title: &str, focused: bool, band: f32, paint: &Paint) -> gpui::Div {
    div()
        .h(px(band * 0.85))
        .px(px(8.0))
        .flex()
        .flex_shrink_0()
        .items_center()
        .font_weight(FontWeight::BOLD)
        .bg(paint.raised)
        .text_color(if focused {
            paint.accent
        } else {
            paint.secondary
        })
        .border_b_1()
        .border_color(paint.border)
        .child(title.to_string())
}

fn text_pane(
    cx: &mut Context<Shell>,
    id: &'static str,
    title: &str,
    focused: bool,
    lines: Vec<String>,
    chips: Vec<present::TagChip>,
    pane: FocusPane,
    band: f32,
    paint: &Paint,
) -> gpui::Div {
    let mut body = div()
        .id(id)
        .flex_1()
        .min_h(px(0.0))
        .overflow_hidden()
        .flex()
        .flex_col()
        .bg(paint.body)
        .text_color(paint.text)
        .on_click(cx.listener(move |shell, _: &ClickEvent, _, cx| {
            let expand = pane == FocusPane::Log && shell.app.state.focus == FocusPane::Log;
            shell.focus_pane(pane);
            if expand {
                shell.forward("enter");
            }
            cx.stop_propagation();
            cx.notify();
        }))
        .on_scroll(cx.listener(move |shell, event: &gpui::ScrollEvent, _, cx| {
            let down = scroll_down(&event.delta);
            shell.wheel(pane, down);
            cx.stop_propagation();
            cx.notify();
        }));
    if !chips.is_empty() {
        body = body.child(tag_row(cx, chips, band, paint));
    }
    if lines.is_empty() {
        body = body.child(
            div()
                .px(px(8.0))
                .text_color(paint.secondary)
                .child("(empty)"),
        );
    }
    for (index, line) in lines.into_iter().enumerate() {
        body = body.child(div().id((id, index)).px(px(8.0)).py(px(1.0)).child(line));
    }
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .overflow_hidden()
        .border_b_1()
        .border_color(paint.border)
        .child(pane_title(title, focused, band, paint))
        .child(body)
}

fn tag_row(
    cx: &mut Context<Shell>,
    chips: Vec<present::TagChip>,
    band: f32,
    paint: &Paint,
) -> gpui::Div {
    let mut row = div()
        .h(px(band))
        .flex_shrink_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .px(px(8.0))
        .bg(paint.body);
    for (index, chip) in chips.into_iter().enumerate() {
        let name = chip.name.clone();
        row = row.child(
            div()
                .id(("tag", index))
                .px(px(6.0))
                .cursor_pointer()
                .border_1()
                .border_color(paint.border)
                .text_color(if chip.close { paint.accent } else { paint.text })
                .on_click(cx.listener(move |shell, _: &ClickEvent, _, cx| {
                    shell.click_tag(&name, false);
                    cx.stop_propagation();
                    cx.notify();
                }))
                .child(format!("[{}]", chip.name)),
        );
        if chip.close {
            let name = chip.name;
            row = row.child(
                div()
                    .id(("tag-x", index))
                    .px(px(4.0))
                    .cursor_pointer()
                    .text_color(paint.accent)
                    .on_click(cx.listener(move |shell, _: &ClickEvent, _, cx| {
                        shell.click_tag(&name, true);
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .child("×"),
            );
        }
    }
    row
}

fn action_row(
    cx: &mut Context<Shell>,
    actions: Vec<(String, String)>,
    period: dd_pantheon::models::MetricsPeriod,
    band: f32,
    paint: &Paint,
) -> gpui::Div {
    let mut row = div()
        .h(px(band))
        .flex_shrink_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .px(px(8.0))
        .bg(paint.raised)
        .border_b_1()
        .border_color(paint.border)
        .overflow_hidden();
    for (index, (key, label)) in [
        (dd_pantheon::models::MetricsPeriod::Day, "d"),
        (dd_pantheon::models::MetricsPeriod::Week, "w"),
        (dd_pantheon::models::MetricsPeriod::Month, "M"),
    ]
    .into_iter()
    .enumerate()
    {
        let on = period == key;
        row = row.child(
            div()
                .id(("period", index))
                .px(px(6.0))
                .cursor_pointer()
                .text_color(if on { paint.accent } else { paint.secondary })
                .on_click(cx.listener(move |shell, _: &ClickEvent, _, cx| {
                    dd_pantheon::workflows::metrics::set_period(&mut shell.app.state, key);
                    cx.stop_propagation();
                    cx.notify();
                }))
                .child(if on {
                    format!("[{label}]")
                } else {
                    label.to_string()
                }),
        );
    }
    for (index, (id, label)) in actions.into_iter().enumerate() {
        row = row.child(
            div()
                .id(("action", index))
                .px(px(6.0))
                .cursor_pointer()
                .text_color(paint.secondary)
                .on_click(cx.listener(move |shell, _: &ClickEvent, _, cx| {
                    shell.activate_action(&id);
                    cx.stop_propagation();
                    cx.notify();
                }))
                .child(label),
        );
    }
    row
}

fn preview_buttons(cx: &mut Context<Shell>, band: f32, paint: &Paint) -> gpui::Div {
    div()
        .h(px(band))
        .flex_shrink_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .px(px(8.0))
        .bg(paint.raised)
        .border_b_1()
        .border_color(paint.border)
        .child(preview_button(
            cx,
            "preview-process",
            "process",
            "PROCESS COMMAND",
            paint.accent,
        ))
        .child(preview_button(
            cx,
            "preview-copy",
            "copy",
            "COPY TO CLIPBOARD",
            paint.secondary,
        ))
        .child(preview_button(
            cx,
            "preview-cancel",
            "cancel",
            "CANCEL",
            paint.secondary,
        ))
}

fn preview_button(
    cx: &mut Context<Shell>,
    element_id: &'static str,
    action: &'static str,
    label: &'static str,
    color: Hsla,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(element_id)
        .px(px(8.0))
        .cursor_pointer()
        .border_1()
        .border_color(color)
        .text_color(color)
        .on_click(cx.listener(move |shell, _: &ClickEvent, _, cx| {
            shell.preview_click(action);
            cx.stop_propagation();
            cx.notify();
        }))
        .child(label)
}

fn toast_bar(message: String, band: f32, accent: Hsla, base: Hsla) -> gpui::Div {
    div()
        .h(px(band * 0.85))
        .flex()
        .flex_shrink_0()
        .items_center()
        .px(px(8.0))
        .bg(base)
        .text_color(accent)
        .child(message)
}

fn scroll_down(delta: &gpui::ScrollDelta) -> bool {
    match delta {
        gpui::ScrollDelta::Pixels(point) => point.y > px(0.0),
        gpui::ScrollDelta::Lines(point) => point.y > 0.0,
    }
}

fn modal_layer(
    cx: &mut Context<Shell>,
    lines: Vec<String>,
    band: f32,
    raised: Hsla,
    color: Hsla,
    border: Hsla,
    accent: Hsla,
) -> gpui::Stateful<gpui::Div> {
    let title = lines.first().cloned().unwrap_or_else(|| "Dialog".into());
    div()
        .id("modal-scrim")
        .absolute()
        .inset_0()
        .occlude()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::hsla(0.0, 0.0, 0.0, 0.35))
        .on_click(cx.listener(|shell, _: &ClickEvent, _, cx| {
            shell.dismiss_outside();
            cx.stop_propagation();
            cx.notify();
        }))
        .child(
            div()
                .id("cockpit-modal")
                .w(relative(0.62))
                .max_h(relative(0.8))
                .flex()
                .flex_col()
                .overflow_hidden()
                .bg(raised)
                .text_color(color)
                .border_1()
                .border_color(border)
                .on_click(cx.listener(|_, _: &ClickEvent, _, cx| {
                    cx.stop_propagation();
                }))
                .child(
                    div()
                        .h(px(band))
                        .px(px(8.0))
                        .flex()
                        .items_center()
                        .font_weight(FontWeight::BOLD)
                        .text_color(accent)
                        .child(title),
                )
                .child({
                    let mut body = div()
                        .id("modal-body")
                        .flex_1()
                        .min_h(px(0.0))
                        .overflow_hidden()
                        .p(px(8.0))
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .on_scroll(cx.listener(|shell, event: &gpui::ScrollEvent, _, cx| {
                            let down = scroll_down(&event.delta);
                            match &mut shell.app.state.modal {
                                Some(Modal::Doctor { scroll }) => {
                                    if down {
                                        *scroll = scroll.saturating_add(1);
                                    } else {
                                        *scroll = scroll.saturating_sub(1);
                                    }
                                }
                                Some(Modal::LogExpand) => shell.wheel(FocusPane::Log, down),
                                _ => {}
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }));
                    for (index, line) in lines.into_iter().enumerate() {
                        body = body.child(
                            div()
                                .id(("modal-line", index))
                                .on_click(cx.listener(move |shell, _: &ClickEvent, _, cx| {
                                    shell.click_modal_line(index);
                                    cx.stop_propagation();
                                    cx.notify();
                                }))
                                .child(line),
                        );
                    }
                    body
                }),
        )
}

#[allow(clippy::too_many_arguments)]
fn settings_panel(
    cx: &mut Context<Shell>,
    page: String,
    hint: String,
    query: String,
    query_on: bool,
    nav: u8,
    theme_rows: Vec<ThemeRowView>,
    color_rows: Vec<ColorRowView>,
    help: Vec<String>,
    results: Vec<String>,
    density: String,
    vim_on: bool,
    window_row: usize,
    band: f32,
    raised: Hsla,
    text: Hsla,
    secondary: Hsla,
    border: Hsla,
    selected: Hsla,
    accent: Hsla,
    scroll: ScrollHandle,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id("settings-panel")
        .occlude()
        .absolute()
        .top(px(band))
        .right(px(0.0))
        .bottom(px(band))
        .w(relative(0.7))
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(raised)
        .text_color(text)
        .border_l_1()
        .border_color(border)
        .on_click(cx.listener(|_, _: &ClickEvent, _, cx| {
            cx.stop_propagation();
        }))
        .child(settings_header(cx, secondary, border))
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .flex()
                .flex_row()
                .overflow_hidden()
                .child(settings_nav(
                    cx,
                    nav,
                    &query,
                    query_on,
                    &NavColors {
                        border,
                        secondary,
                        selected,
                        text,
                    },
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .h_full()
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .child(
                            div()
                                .h(px(28.0))
                                .px(px(8.0))
                                .flex()
                                .items_center()
                                .font_weight(FontWeight::BOLD)
                                .bg(raised)
                                .border_b_1()
                                .border_color(border)
                                .text_color(accent)
                                .child(page),
                        )
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(4.0))
                                .text_color(secondary)
                                .child(hint),
                        )
                        .child(settings_body(
                            cx, nav, theme_rows, color_rows, help, results, density, vim_on,
                            window_row, query, text, secondary, border, selected, accent, raised,
                            scroll,
                        )),
                ),
        )
}

fn settings_header(cx: &mut Context<Shell>, secondary: Hsla, border: Hsla) -> gpui::Div {
    div()
        .flex_shrink_0()
        .h(px(48.0))
        .px(px(16.0))
        .flex()
        .items_center()
        .gap(px(12.0))
        .border_b_1()
        .border_color(border)
        .child("Settings")
        .child(div().flex_1())
        .child(div().text_color(secondary).child("F1"))
        .child(
            div()
                .id("settings-close")
                .px(px(8.0))
                .cursor_pointer()
                .child("Close")
                .on_click(cx.listener(|shell, _: &ClickEvent, _, cx| {
                    shell.revert_settings();
                    cx.stop_propagation();
                    cx.notify();
                })),
        )
}

struct NavColors {
    border: Hsla,
    secondary: Hsla,
    selected: Hsla,
    text: Hsla,
}

fn settings_nav(
    cx: &mut Context<Shell>,
    nav: u8,
    query: &str,
    query_on: bool,
    colors: &NavColors,
) -> gpui::Div {
    let label = if query.is_empty() {
        "Search".to_string()
    } else {
        query.to_string()
    };
    let mut column = div()
        .w(px(200.0))
        .h_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        .border_r_1()
        .border_color(colors.border)
        .child(
            div()
                .id("settings-search")
                .h(px(36.0))
                .px(px(8.0))
                .flex()
                .items_center()
                .cursor_pointer()
                .text_color(if query_on {
                    colors.text
                } else {
                    colors.secondary
                })
                .on_click(cx.listener(|shell, _: &ClickEvent, _, cx| {
                    shell.settings_query_on = true;
                    cx.stop_propagation();
                    cx.notify();
                }))
                .child(label),
        );
    for (index, name) in SETTINGS_NAV.iter().enumerate() {
        let on = query.trim().is_empty() && nav as usize == index;
        let page = index as u8;
        column = column.child(
            div()
                .id(("settings-nav", index))
                .h(px(32.0))
                .px(px(8.0))
                .flex()
                .items_center()
                .overflow_hidden()
                .whitespace_nowrap()
                .cursor_pointer()
                .when(on, |row| row.bg(colors.selected).text_color(colors.text))
                .on_click(cx.listener(move |shell, _: &ClickEvent, _, cx| {
                    shell.settings_nav = page;
                    shell.settings_row = 0;
                    shell.settings_query.clear();
                    shell.settings_query_on = false;
                    cx.stop_propagation();
                    cx.notify();
                }))
                .child(format!("{} {name}", if on { ">" } else { " " })),
        );
    }
    column
}

#[allow(clippy::too_many_arguments)]
fn settings_body(
    cx: &mut Context<Shell>,
    nav: u8,
    theme_rows: Vec<ThemeRowView>,
    color_rows: Vec<ColorRowView>,
    help: Vec<String>,
    results: Vec<String>,
    density: String,
    vim_on: bool,
    window_row: usize,
    query: String,
    text: Hsla,
    secondary: Hsla,
    border: Hsla,
    selected: Hsla,
    accent: Hsla,
    raised: Hsla,
    scroll: ScrollHandle,
) -> gpui::Stateful<gpui::Div> {
    let mut body = div()
        .id("settings-scroll")
        .track_scroll(&scroll)
        .flex_1()
        .min_h(px(0.0))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .p(px(8.0))
        .text_color(text);
    if !query.trim().is_empty() {
        body = body.child(line("Results", secondary));
        if results.is_empty() {
            body = body.child(line("(no matching settings)", secondary));
        }
        for (index, hit) in results.into_iter().enumerate() {
            body = body.child(
                div()
                    .id(("settings-hit", index))
                    .h(px(28.0))
                    .px(px(8.0))
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(hit),
            );
        }
        return body;
    }
    if nav == 1 {
        body = body.child(outlined(
            "Density",
            vec![
                (density.clone(), window_row == 0),
                (
                    if density == "Compact" {
                        "Comfortable".to_string()
                    } else {
                        "Compact".to_string()
                    },
                    false,
                ),
            ],
            0,
            border,
            raised,
            text,
            secondary,
            selected,
            accent,
            cx,
            true,
        ));
        body = body.child(div().h(px(12.0)));
        body = body.child(outlined(
            "Vim keys",
            vec![(
                if vim_on {
                    "Vim keys on".to_string()
                } else {
                    "Vim keys off".to_string()
                },
                window_row == 1,
            )],
            1,
            border,
            raised,
            text,
            secondary,
            selected,
            accent,
            cx,
            false,
        ));
        return body;
    }
    if nav == 2 {
        body = body.child(outlined_lines(
            "Keyboard shortcuts",
            help,
            border,
            raised,
            text,
            secondary,
        ));
        return body;
    }
    if !color_rows.is_empty() {
        body = body.child(line("Theme color editor", accent));
        for row in color_rows {
            let on = row.selected;
            let index = row.index;
            body = body.child(
                div()
                    .id(("color-row", index))
                    .h(px(28.0))
                    .px(px(8.0))
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .when(on, |row| row.bg(selected).text_color(accent))
                    .on_click(cx.listener(move |shell, _: &ClickEvent, _, cx| {
                        if let Some(editor) = shell
                            .appearance
                            .as_mut()
                            .and_then(|appearance| appearance.editor.as_mut())
                        {
                            editor.select(index);
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .child(row.label),
            );
        }
        return body;
    }
    for row in theme_rows {
        let on = row.selected;
        let index = row.index;
        body = body.child(
            div()
                .id(("theme-row", index))
                .h(px(28.0))
                .px(px(8.0))
                .flex()
                .items_center()
                .overflow_hidden()
                .whitespace_nowrap()
                .cursor_pointer()
                .when(on, |row| row.bg(selected).text_color(accent))
                .on_click(cx.listener(move |shell, _: &ClickEvent, _, cx| {
                    shell.choose_theme(index);
                    cx.stop_propagation();
                    cx.notify();
                }))
                .child(row.label),
        );
    }
    body = body.child(line("Edit colors…", secondary));
    body = body.child(line("Save", secondary));
    body
}

fn line(text: impl Into<String>, color: Hsla) -> gpui::Div {
    div()
        .h(px(28.0))
        .px(px(8.0))
        .flex()
        .items_center()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_color(color)
        .child(text.into())
}

#[allow(clippy::too_many_arguments)]
fn outlined(
    title: &'static str,
    rows: Vec<(String, bool)>,
    which: usize,
    border: Hsla,
    raised: Hsla,
    text: Hsla,
    secondary: Hsla,
    selected: Hsla,
    accent: Hsla,
    cx: &mut Context<Shell>,
    density: bool,
) -> gpui::Div {
    let mut box_ = div()
        .flex()
        .flex_col()
        .border_1()
        .border_color(border)
        .overflow_hidden()
        .child(
            div()
                .h(px(28.0))
                .px(px(8.0))
                .flex()
                .items_center()
                .font_weight(FontWeight::BOLD)
                .bg(raised)
                .border_b_1()
                .border_color(border)
                .text_color(text)
                .child(title.to_string()),
        );
    for (index, (label, on)) in rows.into_iter().enumerate() {
        let picked = label.clone();
        box_ = box_.child(
            div()
                .id((title, index))
                .h(px(28.0))
                .px(px(8.0))
                .flex()
                .items_center()
                .cursor_pointer()
                .text_color(if on { accent } else { secondary })
                .when(on, |row| row.bg(selected))
                .on_click(cx.listener(move |shell, _: &ClickEvent, _, cx| {
                    shell.settings_nav = 1;
                    shell.settings_row = if density { 0 } else { which };
                    if index == 0 || !density {
                        shell.activate_window_row();
                    } else if density {
                        let next = if picked == "Compact" {
                            Density::Compact
                        } else {
                            Density::Comfortable
                        };
                        shell.set_density(next);
                    }
                    cx.stop_propagation();
                    cx.notify();
                }))
                .child(label),
        );
    }
    box_
}

fn outlined_lines(
    title: &str,
    rows: Vec<String>,
    border: Hsla,
    raised: Hsla,
    text: Hsla,
    secondary: Hsla,
) -> gpui::Div {
    let mut box_ = div()
        .flex()
        .flex_col()
        .border_1()
        .border_color(border)
        .overflow_hidden()
        .child(
            div()
                .h(px(28.0))
                .px(px(8.0))
                .flex()
                .items_center()
                .font_weight(FontWeight::BOLD)
                .bg(raised)
                .border_b_1()
                .border_color(border)
                .text_color(text)
                .child(title.to_string()),
        );
    for (index, row) in rows.into_iter().enumerate() {
        box_ = box_.child(
            div()
                .id(("help-row", index))
                .min_h(px(24.0))
                .px(px(8.0))
                .flex()
                .items_center()
                .text_color(if index == 0 { text } else { secondary })
                .child(row),
        );
    }
    box_
}

#[cfg(test)]
mod frame_tests {
    use super::*;

    #[test]
    fn busy_mark_bounces_low_mid_high_mid() {
        assert_eq!(busy_label(Duration::from_millis(0)), "▃ d_d ▃");
        assert_eq!(busy_label(Duration::from_millis(150)), "▅ d_d ▅");
        assert_eq!(busy_label(Duration::from_millis(300)), "█ d_d █");
        assert_eq!(busy_label(Duration::from_millis(450)), "▅ d_d ▅");
        assert_eq!(busy_label(Duration::from_millis(600)), "▃ d_d ▃");
    }

    #[test]
    fn idle_footer_names_settings_and_quit() {
        assert!("F1: Settings  Ctrl+Q: Quit".contains("F1: Settings"));
    }
}
