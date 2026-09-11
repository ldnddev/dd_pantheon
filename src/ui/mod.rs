mod actions;
mod help;
mod inspector;
mod layouts;
mod log;
mod metrics;
mod modals;
mod preview;
mod shell;
mod theme_modal;
mod tree;

pub use shell::footer_keys;

use crate::state::AppState;
use crate::toast::ToastLevel;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

pub fn draw(f: &mut Frame, state: &mut AppState) {
    state.last_frame = f.area();
    f.render_widget(Block::default().style(state.theme.app_shell), f.area());

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.area());

    shell::draw_header(f, state, outer[0]);
    let panes = layouts::split(state.layout, outer[1], state);
    state.tree_area = panes.tree;
    state.inspector_area = panes.inspector;
    state.preview_area = panes.preview;
    state.log_area = panes.log;
    state.footer_area = outer[2];

    tree::draw(f, state, panes.tree);
    inspector::draw(f, state, panes.inspector);
    preview::draw(f, state, panes.preview);
    log::draw(f, state, panes.log);
    shell::draw_footer(f, state, outer[2]);

    if state.modal.is_some() {
        draw_modal(f, state, f.area());
    } else {
        state.modal_area = None;
    }
    if state.toast.is_some() {
        draw_toast(f, state, f.area());
    } else {
        state.toast_area = None;
    }
}

fn draw_modal(f: &mut Frame, state: &mut AppState, area: Rect) {
    use crate::state::Modal;
    let modal_area = match &state.modal {
        Some(Modal::Filter { .. }) => centered_rect(58, 62, area),
        Some(Modal::Palette { form: None, .. }) => centered_rect(78, 74, area),
        _ => centered_rect(72, 70, area),
    };
    state.modal_area = Some(modal_area);
    f.render_widget(Clear, modal_area);

    match state.modal.clone() {
        Some(Modal::Help { scroll }) => help::draw(f, state, modal_area, scroll),
        Some(Modal::Theme { scroll }) => theme_modal::draw(f, state, modal_area, scroll),
        Some(Modal::Doctor { scroll }) => {
            let body = crate::doctor::render(
                &state.tools,
                &state.auth,
                &state.catalog,
                &state.doctor_warnings,
            );
            let p = Paragraph::new(body)
                .style(state.theme.modal_text)
                .wrap(Wrap { trim: false })
                .scroll((scroll, 0))
                .block(
                    Block::default()
                        .title("F3 Doctor")
                        .borders(Borders::ALL)
                        .border_style(state.theme.active_border)
                        .style(state.theme.modal),
                );
            f.render_widget(p, modal_area);
        }
        Some(Modal::Login {
            token,
            use_env_token,
            env_available,
        }) => {
            modals::draw_login(
                f,
                &state.theme,
                modal_area,
                &token,
                use_env_token,
                env_available,
            );
        }
        Some(Modal::ConfirmDestructive { plan }) => {
            modals::draw_destructive(f, state, modal_area, &plan);
        }
        Some(Modal::LiveGate {
            plan,
            expected,
            typed,
        }) => {
            modals::draw_livegate(f, &state.theme, modal_area, &plan, &expected, &typed);
        }
        Some(Modal::TagAdd { value }) => {
            modals::draw_tag_add(f, &state.theme, modal_area, &value);
        }
        Some(Modal::OrgPicker { orgs, selected, .. }) => {
            modals::draw_org_picker(f, &state.theme, modal_area, &orgs, selected);
        }
        Some(Modal::TagPicker { tags, selected }) => {
            modals::draw_tag_picker(f, &state.theme, modal_area, &tags, selected);
        }
        Some(Modal::DiffstatDirty { site, env, files }) => {
            modals::draw_diffstat_dirty(f, &state.theme, modal_area, &site, &env, &files);
        }
        Some(Modal::SiteCreate { form }) => {
            modals::draw_site_create(f, &state.theme, modal_area, &form);
        }
        Some(Modal::MultidevCreate {
            site,
            name,
            sources,
            source_idx,
        }) => {
            modals::draw_multidev_create(
                f,
                &state.theme,
                modal_area,
                &site,
                &name,
                &sources,
                source_idx,
            );
        }
        Some(Modal::CloneContent {
            site,
            target,
            origins,
            origin_idx,
            cc,
            db_only,
            files_only,
            updatedb,
        }) => {
            modals::draw_clone_content(
                f,
                &state.theme,
                modal_area,
                &site,
                &target,
                &origins,
                origin_idx,
                cc,
                db_only,
                files_only,
                updatedb,
            );
        }
        Some(Modal::DomainAdd { value, .. }) => {
            modals::draw_domain_add(f, &state.theme, modal_area, &value);
        }
        Some(Modal::DomainRemove {
            domains, selected, ..
        }) => {
            modals::draw_domain_remove(f, &state.theme, modal_area, &domains, selected);
        }
        Some(Modal::HttpsSet {
            cert,
            key,
            intermediate,
            focus,
            ..
        }) => {
            modals::draw_https_set(
                f,
                &state.theme,
                modal_area,
                &cert,
                &key,
                &intermediate,
                focus,
            );
        }
        Some(Modal::LockEnable {
            username,
            password,
            focus,
            ..
        }) => {
            modals::draw_lock_enable(f, &state.theme, modal_area, &username, &password, focus);
        }
        Some(Modal::Palette {
            query,
            selected,
            form,
        }) => {
            modals::draw_palette(f, state, modal_area, &query, selected, form.as_ref());
        }
        Some(Modal::Cms { form }) => {
            modals::draw_cms(f, state, modal_area, &form);
        }
        Some(Modal::BackupPick {
            files,
            selected,
            kind,
            ..
        }) => {
            modals::draw_backup_pick(f, &state.theme, modal_area, &files, selected, kind);
        }
        Some(Modal::DeployNote {
            env,
            note,
            sync_content,
            updatedb,
            focus,
            ..
        }) => {
            modals::draw_deploy_note(
                f,
                &state.theme,
                modal_area,
                &env,
                &note,
                sync_content,
                updatedb,
                focus,
            );
        }
        Some(Modal::Filter { query, selected }) => {
            modals::draw_filter(f, state, modal_area, &query, selected);
        }
        Some(Modal::Error { msg }) => {
            let p = Paragraph::new(msg)
                .style(state.theme.modal_text)
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title("Error")
                        .borders(Borders::ALL)
                        .border_style(state.theme.error)
                        .style(state.theme.modal),
                );
            f.render_widget(p, modal_area);
        }
        Some(Modal::QuitConfirm) => {
            let p = Paragraph::new("Quit? Enter/y yes, Esc cancel")
                .style(state.theme.modal_text)
                .block(
                    Block::default()
                        .title("Quit")
                        .borders(Borders::ALL)
                        .border_style(state.theme.active_border)
                        .style(state.theme.modal),
                );
            f.render_widget(p, modal_area);
        }
        None => {}
    }
}

fn draw_toast(f: &mut Frame, state: &mut AppState, area: Rect) {
    let Some(toast) = &state.toast else {
        state.toast_area = None;
        return;
    };
    if area.width < 8 || area.height < 5 {
        state.toast_area = None;
        return;
    }
    let max_width = area.width.saturating_sub(2).min(50);
    let longest = toast
        .message
        .lines()
        .map(|l| l.chars().count() as u16)
        .max()
        .unwrap_or(0);
    let width = longest.saturating_add(4).clamp(24, max_width);
    let height = (toast.message.lines().count().max(1) as u16)
        .saturating_add(2)
        .clamp(3, area.height.saturating_sub(1).min(7));
    let x = area.x + area.width.saturating_sub(width).saturating_sub(1);
    let y = area.y + area.height.saturating_sub(height).saturating_sub(1);
    let toast_area = Rect::new(x, y, width, height);
    state.toast_area = Some(toast_area);

    let (title, border) = match toast.level {
        ToastLevel::Info => ("Info", state.theme.info),
        ToastLevel::Success => ("Success", state.theme.success),
        ToastLevel::Warning => ("Warning", state.theme.warning_style),
        ToastLevel::Error => ("Error", state.theme.error),
    };
    let text = Paragraph::new(Line::from(toast.message.as_str()))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(border)
                .style(state.theme.modal),
        )
        .style(state.theme.modal_text);
    f.render_widget(Clear, toast_area);
    f.render_widget(text, toast_area);
}

pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(v[1])[1]
}

pub fn pane_block<'a>(
    title: impl Into<ratatui::text::Line<'a>>,
    focused: bool,
    theme: &'a crate::theme::Theme,
) -> Block<'a> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if focused {
            theme.active_border
        } else {
            theme.border
        })
        .style(theme.body)
}
