use crate::models::InspectorTab;
use crate::state::{AppState, FocusPane, Modal};
use crate::toast::ToastLevel;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

pub fn handle_key(state: &mut AppState, key: KeyEvent) -> Result<bool> {
    if key.kind != crossterm::event::KeyEventKind::Press
        && key.kind != crossterm::event::KeyEventKind::Repeat
    {
        return Ok(false);
    }

    if is_quit_key(key) {
        if state.job_running {
            state.modal = Some(Modal::QuitConfirm);
            return Ok(false);
        }
        state.should_quit = true;
        return Ok(true);
    }

    let text_field = matches!(
        state.modal,
        Some(
            Modal::Filter { .. }
                | Modal::Login { .. }
                | Modal::LiveGate { .. }
                | Modal::TagAdd { .. },
        )
    );

    if !text_field {
        match key.code {
            KeyCode::F(1) => {
                state.modal = Some(Modal::Help { scroll: 0 });
                return Ok(false);
            }
            KeyCode::F(2) => {
                state.modal = Some(Modal::Theme { scroll: 0 });
                return Ok(false);
            }
            KeyCode::F(3) => {
                crate::workflows::start_doctor_jobs(state);
                state.modal = Some(Modal::Doctor { scroll: 0 });
                return Ok(false);
            }
            KeyCode::F(4) => {
                state.cycle_layout();
                return Ok(false);
            }
            _ => {}
        }
    }

    if let Some(modal) = state.modal.clone() {
        return handle_modal(state, key, modal);
    }

    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            crate::workflows::cancel_jobs(state);
            return Ok(false);
        }
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            crate::workflows::open_login(state);
            return Ok(false);
        }
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.show_toast(ToastLevel::Info, "palette lands in PR 13");
            return Ok(false);
        }
        KeyCode::Char(':') => {
            state.show_toast(ToastLevel::Info, "palette lands in PR 13");
            return Ok(false);
        }
        KeyCode::Tab => {
            state.focus = if key.modifiers.contains(KeyModifiers::SHIFT) {
                state.focus.prev()
            } else {
                state.focus.next()
            };
            return Ok(false);
        }
        KeyCode::BackTab => {
            state.focus = state.focus.prev();
            return Ok(false);
        }
        KeyCode::Esc => {
            state.filter.clear();
            state.tag_filter = None;
            state.rebuild_tree();
            state.select_matching_row();
            return Ok(false);
        }
        _ => {}
    }

    match state.focus {
        FocusPane::Tree => handle_tree(state, key),
        FocusPane::Inspector => handle_inspector(state, key),
        FocusPane::Preview => handle_preview(state, key),
        FocusPane::Log => handle_log(state, key),
    }
}

fn handle_modal(state: &mut AppState, key: KeyEvent, modal: Modal) -> Result<bool> {
    match modal {
        Modal::Filter { mut query } => match key.code {
            KeyCode::Esc => {
                state.modal = None;
            }
            KeyCode::Enter => {
                state.filter = query;
                state.modal = None;
                state.rebuild_tree();
                state.select_matching_row();
            }
            KeyCode::Backspace => {
                query.pop();
                state.filter = query.clone();
                state.rebuild_tree();
                state.modal = Some(Modal::Filter { query });
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                query.push(c);
                state.filter = query.clone();
                state.rebuild_tree();
                state.modal = Some(Modal::Filter { query });
            }
            _ => {}
        },
        Modal::TagAdd { mut value } => match key.code {
            KeyCode::Esc => state.modal = None,
            KeyCode::Enter => {
                state.modal = None;
                crate::workflows::tags::submit_add(state, value);
            }
            KeyCode::Backspace => {
                value.pop();
                state.modal = Some(Modal::TagAdd { value });
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                value.push(c);
                state.modal = Some(Modal::TagAdd { value });
            }
            _ => {}
        },
        Modal::OrgPicker {
            site,
            orgs,
            mut selected,
        } => match key.code {
            KeyCode::Esc => state.modal = None,
            KeyCode::Char('j') | KeyCode::Down => {
                if !orgs.is_empty() {
                    selected = (selected + 1).min(orgs.len() - 1);
                }
                state.modal = Some(Modal::OrgPicker {
                    site,
                    orgs,
                    selected,
                });
            }
            KeyCode::Char('k') | KeyCode::Up => {
                selected = selected.saturating_sub(1);
                state.modal = Some(Modal::OrgPicker {
                    site,
                    orgs,
                    selected,
                });
            }
            KeyCode::Enter => {
                if let Some(org) = orgs.get(selected).cloned() {
                    state.modal = None;
                    crate::workflows::tags::pick_org(state, &site, org);
                }
            }
            _ => {
                state.modal = Some(Modal::OrgPicker {
                    site,
                    orgs,
                    selected,
                });
            }
        },
        Modal::TagPicker { tags, mut selected } => match key.code {
            KeyCode::Esc => state.modal = None,
            KeyCode::Char('j') | KeyCode::Down => {
                if !tags.is_empty() {
                    selected = (selected + 1).min(tags.len() - 1);
                }
                state.modal = Some(Modal::TagPicker { tags, selected });
            }
            KeyCode::Char('k') | KeyCode::Up => {
                selected = selected.saturating_sub(1);
                state.modal = Some(Modal::TagPicker { tags, selected });
            }
            KeyCode::Enter => {
                if let Some(tag) = tags.get(selected).cloned() {
                    state.modal = None;
                    crate::workflows::tags::pin_filter(state, tag);
                }
            }
            _ => {
                state.modal = Some(Modal::TagPicker { tags, selected });
            }
        },
        Modal::Help { scroll } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::F(1) => state.modal = None,
            KeyCode::Char('j') | KeyCode::Down => {
                state.modal = Some(Modal::Help {
                    scroll: scroll.saturating_add(1),
                });
            }
            KeyCode::Char('k') | KeyCode::Up => {
                state.modal = Some(Modal::Help {
                    scroll: scroll.saturating_sub(1),
                });
            }
            _ => {}
        },
        Modal::Theme { scroll } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::F(2) => state.modal = None,
            KeyCode::Char('j') | KeyCode::Down => {
                state.modal = Some(Modal::Theme {
                    scroll: scroll.saturating_add(1),
                });
            }
            KeyCode::Char('k') | KeyCode::Up => {
                state.modal = Some(Modal::Theme {
                    scroll: scroll.saturating_sub(1),
                });
            }
            _ => {}
        },
        Modal::Doctor { scroll } => match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::F(3) => state.modal = None,
            KeyCode::Char('j') | KeyCode::Down => {
                state.modal = Some(Modal::Doctor {
                    scroll: scroll.saturating_add(1),
                });
            }
            KeyCode::Char('k') | KeyCode::Up => {
                state.modal = Some(Modal::Doctor {
                    scroll: scroll.saturating_sub(1),
                });
            }
            _ => {}
        },
        Modal::Login {
            mut token,
            mut use_env_token,
            env_available,
        } => match key.code {
            KeyCode::Esc => state.modal = None,
            KeyCode::Char(' ') if env_available => {
                use_env_token = !use_env_token;
                state.modal = Some(Modal::Login {
                    token,
                    use_env_token,
                    env_available,
                });
            }
            KeyCode::Enter => {
                let value = if use_env_token {
                    std::env::var("TERMINUS_MACHINE_TOKEN").unwrap_or_default()
                } else {
                    token.clone()
                };
                if value.is_empty() {
                    state.show_toast(ToastLevel::Warning, "token is empty");
                } else {
                    state.modal = None;
                    crate::workflows::submit_login(state, value);
                }
            }
            KeyCode::Backspace if !use_env_token => {
                token.pop();
                state.modal = Some(Modal::Login {
                    token,
                    use_env_token,
                    env_available,
                });
            }
            KeyCode::Char(c)
                if !use_env_token && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                token.push(c);
                state.modal = Some(Modal::Login {
                    token,
                    use_env_token,
                    env_available,
                });
            }
            _ => {
                state.modal = Some(Modal::Login {
                    token,
                    use_env_token,
                    env_available,
                });
            }
        },
        Modal::ConfirmDestructive { plan } => match key.code {
            KeyCode::Esc => state.modal = None,
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                state.modal = None;
                crate::workflows::start_user_job(state, plan);
            }
            _ => {}
        },
        Modal::LiveGate {
            plan,
            expected,
            mut typed,
        } => match key.code {
            KeyCode::Esc => state.modal = None,
            KeyCode::Enter => {
                if typed == expected {
                    state.modal = None;
                    crate::workflows::start_user_job(state, plan);
                } else {
                    state.show_toast(ToastLevel::Warning, format!("type `{expected}` to confirm"));
                    state.modal = Some(Modal::LiveGate {
                        plan,
                        expected,
                        typed,
                    });
                }
            }
            KeyCode::Backspace => {
                typed.pop();
                state.modal = Some(Modal::LiveGate {
                    plan,
                    expected,
                    typed,
                });
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                typed.push(c);
                state.modal = Some(Modal::LiveGate {
                    plan,
                    expected,
                    typed,
                });
            }
            _ => {
                state.modal = Some(Modal::LiveGate {
                    plan,
                    expected,
                    typed,
                });
            }
        },
        Modal::Error { .. } => {
            if matches!(key.code, KeyCode::Esc | KeyCode::Enter) {
                state.modal = None;
            }
        }
        Modal::QuitConfirm => match key.code {
            KeyCode::Esc => state.modal = None,
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                crate::workflows::cancel_jobs(state);
                state.should_quit = true;
                return Ok(true);
            }
            _ => {}
        },
    }
    Ok(false)
}

fn handle_tree(state: &mut AppState, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => state.move_tree(1),
        KeyCode::Char('k') | KeyCode::Up => state.move_tree(-1),
        KeyCode::Char('g') => state.jump_tree(false),
        KeyCode::Char('G') => state.jump_tree(true),
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Char('l') | KeyCode::Right => {
            state.toggle_expand();
        }
        KeyCode::Enter => state.expand_or_select(),
        KeyCode::Char('/') => {
            state.modal = Some(Modal::Filter {
                query: state.filter.clone(),
            });
        }
        KeyCode::Char('T') => crate::workflows::tags::open_tag_picker(state),
        other => shared_action_keys(state, other),
    }
    Ok(false)
}

fn handle_inspector(state: &mut AppState, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if state.layout == crate::models::LayoutId::TabbedInspector
                && state.inspector_tab != InspectorTab::Actions
            {
                state.inspector_scroll = state.inspector_scroll.saturating_add(1);
            } else {
                state.move_actions(1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if state.layout == crate::models::LayoutId::TabbedInspector
                && state.inspector_tab != InspectorTab::Actions
            {
                state.inspector_scroll = state.inspector_scroll.saturating_sub(1);
            } else {
                state.move_actions(-1);
            }
        }
        KeyCode::Enter => {
            if let Some(id) = state.selected_action_id() {
                crate::workflows::stage_action(state, id);
            }
            crate::workflows::request_run(state);
        }
        KeyCode::Char('1') => state.inspector_tab = InspectorTab::Info,
        KeyCode::Char('2') => state.inspector_tab = InspectorTab::Metrics,
        KeyCode::Char('3') => state.inspector_tab = InspectorTab::Local,
        KeyCode::Char('4') => state.inspector_tab = InspectorTab::Actions,
        KeyCode::Char('h') | KeyCode::Left => {
            if state.layout == crate::models::LayoutId::TabbedInspector {
                state.inspector_tab = state.inspector_tab.prev();
            } else {
                crate::workflows::tags::cycle_chip(state, -1);
            }
        }
        KeyCode::Char('l') | KeyCode::Right => {
            if state.layout == crate::models::LayoutId::TabbedInspector {
                state.inspector_tab = state.inspector_tab.next();
            } else {
                crate::workflows::tags::cycle_chip(state, 1);
            }
        }
        KeyCode::Char('[') => crate::workflows::tags::cycle_chip(state, -1),
        KeyCode::Char(']') => crate::workflows::tags::cycle_chip(state, 1),
        KeyCode::Char('T') => crate::workflows::tags::open_tag_picker(state),
        KeyCode::Char('x') => {
            if let Some(tag) = state.selected_chip.clone() {
                crate::workflows::tags::stage_remove(state, &tag);
            }
        }
        KeyCode::Char('/') => {
            state.modal = Some(Modal::Filter {
                query: state.filter.clone(),
            });
        }
        other => shared_action_keys(state, other),
    }
    Ok(false)
}

fn handle_preview(state: &mut AppState, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Enter => crate::workflows::request_run(state),
        KeyCode::Char('y') => {
            state.show_toast(ToastLevel::Info, "copied (clipboard lands later)");
        }
        KeyCode::Char('c') => {}
        KeyCode::Char('j') | KeyCode::Down => {
            state.preview_scroll = state.preview_scroll.saturating_add(1);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.preview_scroll = state.preview_scroll.saturating_sub(1);
        }
        _ => {}
    }
    Ok(false)
}

fn handle_log(state: &mut AppState, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            state.log_scroll = state.log_scroll.saturating_add(1);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.log_scroll = state.log_scroll.saturating_sub(1);
        }
        _ => {}
    }
    Ok(false)
}

fn shared_action_keys(state: &mut AppState, code: KeyCode) {
    let id = match code {
        KeyCode::Char('b') => "backup",
        KeyCode::Char('c') => "cache",
        KeyCode::Char('e') => "deploy",
        KeyCode::Char('s') => "lando-start",
        KeyCode::Char('S') => "lando-stop",
        KeyCode::Char('n') => "n",
        KeyCode::Char('m') => "cms",
        KeyCode::Char('a') => "a",
        KeyCode::Char('r') => "r",
        _ => return,
    };
    crate::workflows::stage_action(state, id);
    if !matches!(id, "r" | "n" | "m" | "a") {
        crate::workflows::request_run(state);
    }
}

pub fn handle_mouse(state: &mut AppState, mouse: MouseEvent) -> Result<bool> {
    let (x, y) = (mouse.column, mouse.row);
    if let Some(area) = state.modal_area {
        if contains(area, x, y) {
            if let MouseEventKind::ScrollDown | MouseEventKind::ScrollUp = mouse.kind {
                let down = matches!(mouse.kind, MouseEventKind::ScrollDown);
                match &mut state.modal {
                    Some(Modal::Help { scroll })
                    | Some(Modal::Theme { scroll })
                    | Some(Modal::Doctor { scroll }) => {
                        if down {
                            *scroll = scroll.saturating_add(1);
                        } else {
                            *scroll = scroll.saturating_sub(1);
                        }
                    }
                    _ => {}
                }
            }
            return Ok(false);
        }
    }

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if contains(state.tree_area, x, y) {
                state.focus = FocusPane::Tree;
                if let Some(idx) = row_at(state.tree_area, y, state.tree_rows.len()) {
                    state.tree_state.select(Some(idx));
                    state.apply_row_selection(idx);
                }
            } else if contains(state.inspector_area, x, y) {
                state.focus = FocusPane::Inspector;
                let chips = state.tag_chips.clone();
                if let Some(chip) = chips
                    .iter()
                    .find(|c| contains(c.close, x, y) && c.close.width > 0)
                {
                    crate::workflows::tags::stage_remove(state, &chip.name);
                } else if let Some(chip) = chips.iter().find(|c| contains(c.body, x, y)) {
                    state.selected_chip = Some(chip.name.clone());
                    crate::workflows::tags::pin_filter(state, chip.name.clone());
                }
            } else if contains(state.preview_area, x, y) {
                state.focus = FocusPane::Preview;
            } else if contains(state.log_area, x, y) {
                state.focus = FocusPane::Log;
            }
        }
        MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
            let down = matches!(mouse.kind, MouseEventKind::ScrollDown);
            let pane = hit_pane(state, x, y).unwrap_or(state.focus);
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
                    if down {
                        state.log_scroll = state.log_scroll.saturating_add(1);
                    } else {
                        state.log_scroll = state.log_scroll.saturating_sub(1);
                    }
                }
            }
        }
        _ => {}
    }
    Ok(false)
}

fn hit_pane(state: &AppState, x: u16, y: u16) -> Option<FocusPane> {
    if contains(state.tree_area, x, y) {
        Some(FocusPane::Tree)
    } else if contains(state.inspector_area, x, y) {
        Some(FocusPane::Inspector)
    } else if contains(state.preview_area, x, y) {
        Some(FocusPane::Preview)
    } else if contains(state.log_area, x, y) {
        Some(FocusPane::Log)
    } else {
        None
    }
}

fn is_quit_key(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
}

fn contains(r: Rect, x: u16, y: u16) -> bool {
    r.width > 0 && r.height > 0 && x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}

fn row_at(area: Rect, y: u16, len: usize) -> Option<usize> {
    if area.height < 3 || y <= area.y {
        return None;
    }
    let inner_y = y.saturating_sub(area.y.saturating_add(1));
    let idx = inner_y as usize;
    if idx < len { Some(idx) } else { None }
}
