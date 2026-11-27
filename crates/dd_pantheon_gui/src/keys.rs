//! Window chord to the terminal `KeyEvent`. One command table.

use dd_pantheon::{KeyCode, KeyEvent, KeyModifiers};

/// `ctrl-q`, `shift-g`, `f3`, `shift-/` (which is `?`).
pub fn to_key_event(chord: &str) -> Option<KeyEvent> {
    let chord = chord.to_ascii_lowercase();
    if chord == "backtab" || chord == "shift-tab" {
        return Some(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    }
    let (mods, key) = split_chord(&chord)?;
    let mut flags = KeyModifiers::empty();
    if mods.control {
        flags |= KeyModifiers::CONTROL;
    }
    if mods.alt {
        flags |= KeyModifiers::ALT;
    }
    let code = match key {
        "escape" | "esc" => KeyCode::Esc,
        "enter" | "return" => KeyCode::Enter,
        "tab" => {
            if mods.shift {
                KeyCode::BackTab
            } else {
                KeyCode::Tab
            }
        }
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "space" | " " => KeyCode::Char(' '),
        "?" | "shift-/" => KeyCode::Char('?'),
        other if other.starts_with('f') && other.len() > 1 => {
            let n: u8 = other[1..].parse().ok()?;
            if (1..=12).contains(&n) {
                KeyCode::F(n)
            } else {
                return None;
            }
        }
        "/" if mods.shift => KeyCode::Char('?'),
        other => {
            let mut chars = other.chars();
            let ch = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            let ch = if mods.shift {
                ch.to_ascii_uppercase()
            } else {
                ch
            };
            KeyCode::Char(ch)
        }
    };
    if mods.shift && !matches!(code, KeyCode::Char(_) | KeyCode::BackTab) {
        flags |= KeyModifiers::SHIFT;
    }
    Some(KeyEvent::new(code, flags))
}

/// Letters that move the caret in the terminal. The window suppresses them
/// when vim keys are off and no modal is collecting input.
pub fn vim_motion(chord: &str) -> bool {
    matches!(chord, "j" | "k" | "h" | "l" | "g" | "shift-g")
}

struct Mods {
    control: bool,
    shift: bool,
    alt: bool,
}

fn split_chord(chord: &str) -> Option<(Mods, &str)> {
    if chord == "?" {
        return Some((
            Mods {
                control: false,
                shift: false,
                alt: false,
            },
            "?",
        ));
    }
    let mut parts: Vec<&str> = chord.split('-').collect();
    let key = parts.pop()?;
    if key.is_empty() {
        return None;
    }
    let mut mods = Mods {
        control: false,
        shift: false,
        alt: false,
    };
    for part in parts {
        match part {
            "ctrl" => mods.control = true,
            "shift" => mods.shift = true,
            "alt" => mods.alt = true,
            _ => return None,
        }
    }
    Some((mods, key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords_match_the_terminal_keys() {
        let quit = to_key_event("ctrl-q").unwrap();
        assert_eq!(quit.code, KeyCode::Char('q'));
        assert!(quit.modifiers.contains(KeyModifiers::CONTROL));

        assert_eq!(to_key_event("shift-g").unwrap().code, KeyCode::Char('G'));
        assert_eq!(to_key_event("shift-m").unwrap().code, KeyCode::Char('M'));
        assert_eq!(to_key_event("shift-t").unwrap().code, KeyCode::Char('T'));
        assert_eq!(to_key_event("shift-/").unwrap().code, KeyCode::Char('?'));
        assert_eq!(to_key_event("f3").unwrap().code, KeyCode::F(3));
        assert_eq!(to_key_event("pageup").unwrap().code, KeyCode::PageUp);
        assert_eq!(to_key_event("shift-tab").unwrap().code, KeyCode::BackTab);
        assert!(vim_motion("j"));
        assert!(!vim_motion("m"));
        assert!(!vim_motion("ctrl-k"));
    }
}
