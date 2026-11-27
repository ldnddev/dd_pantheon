//! Shortcut sheet. The first Global row is the settings chord.

pub struct Shortcut {
    pub section: &'static str,
    pub action: &'static str,
    pub gui: &'static str,
    pub tui: &'static str,
}

pub fn rows() -> &'static [Shortcut] {
    &[
        Shortcut {
            section: "Global",
            action: "Settings",
            gui: "F1",
            tui: "F1",
        },
        Shortcut {
            section: "Global",
            action: "Quit",
            gui: "Ctrl+Q",
            tui: "Ctrl+Q",
        },
        Shortcut {
            section: "Global",
            action: "F3 Doctor",
            gui: "F3",
            tui: "F3",
        },
        Shortcut {
            section: "Global",
            action: "Close",
            gui: "Esc",
            tui: "Esc",
        },
        Shortcut {
            section: "Global",
            action: "Palette",
            gui: "Ctrl+K",
            tui: "Ctrl+K",
        },
        Shortcut {
            section: "Global",
            action: "Filter",
            gui: "/",
            tui: "/",
        },
        Shortcut {
            section: "Global",
            action: "Login",
            gui: "Ctrl+L",
            tui: "Ctrl+L",
        },
        Shortcut {
            section: "Global",
            action: "Cancel job",
            gui: "Ctrl+C",
            tui: "Ctrl+C",
        },
        Shortcut {
            section: "Global",
            action: "Help",
            gui: "?",
            tui: "?",
        },
        Shortcut {
            section: "Tree",
            action: "Move",
            gui: "Up/Down",
            tui: "j/k",
        },
        Shortcut {
            section: "Tree",
            action: "Expand site",
            gui: "Enter",
            tui: "Enter",
        },
        Shortcut {
            section: "Themes",
            action: "Save theme",
            gui: "y",
            tui: "y",
        },
        Shortcut {
            section: "Themes",
            action: "Save target",
            gui: "t",
            tui: "t",
        },
    ]
}

pub fn lines() -> Vec<String> {
    let mut out = Vec::new();
    let mut section = "";
    for row in rows() {
        if row.section != section {
            section = row.section;
            out.push(section.to_string());
        }
        out.push(format!("{} · {} · {}", row.action, row.gui, row.tui));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_starts_with_settings_and_names_doctor() {
        let lines = lines();
        assert_eq!(lines[0], "Global");
        assert_eq!(lines[1], "Settings · F1 · F1");
        let text = lines.join("\n");
        assert!(text.contains("F3 Doctor"));
        assert!(text.contains("F3 · F3"));
    }
}
