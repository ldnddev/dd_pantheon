//! Headless window tests. Mount does not draw a frame.

use std::fs;

use gpui::{AppContext, TestAppContext};

use crate::shell::{Boot, Shell};

fn mount(cx: &mut TestAppContext, dir: &std::path::Path) -> gpui::Entity<Shell> {
    cx.update(gpui_component::init);
    cx.new(|cx| Shell::open(Boot::demo(dir), cx))
}

#[gpui::test]
fn body_lists_the_demo_sites(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let shell = mount(cx, dir.path());
    shell.update(cx, |shell, _| {
        let text = shell.visible_text();
        assert!(text.contains("acme-wp"), "{text}");
        assert!(text.contains("acme-d10"), "{text}");
        assert!(text.contains("frozen-lab"), "{text}");
        assert!(text.contains("prod"), "{text}");
        assert!(text.contains("client-acme"), "{text}");
        assert!(text.contains(">     ▪ test  git"), "{text}");
        assert!(text.contains("framework WordPress"), "{text}");
        assert!(text.contains("tags [prod] [client-acme]"), "{text}");
        assert!(text.contains("runtime 8.3"), "{text}");
        assert!(text.contains("https://test-acme-wp.pantheonsite.io"), "{text}");
        assert!(text.contains("header:DEMO acme-wp.test"), "{text}");
        assert!(text.contains("focus:tree"), "{text}");
        assert!(
            text.contains("footer:F1: Settings  F3: Doctor"),
            "{text}"
        );
        assert!(!text.contains("settings-panel"), "{text}");
        assert!(shell.app.state.demo);
        assert!(shell.app.state.skip_detect);
        assert!(shell.app.state.jobs.is_empty());
    });
}

#[gpui::test]
fn settings_sheet_pages_revert_and_restore_the_palette(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let shell = mount(cx, dir.path());
    shell.update(cx, |shell, _| {
        let before = shell.palette_hex("base_background");
        shell.handle_key("f1");
        let open = shell.visible_text();
        assert!(open.contains("settings-panel:right:70"), "{open}");
        assert!(open.contains("settings-nav:Themes,Window,Help"), "{open}");
        assert!(open.contains("settings-page:Themes"), "{open}");
        assert!(
            open.contains("theme-list:flow") || open.contains("Theme color editor"),
            "{open}"
        );
        assert!(open.contains("Esc: Close"), "{open}");
        if !open.contains("Theme color editor") {
            shell.handle_key("e");
        }
        let editor = shell.visible_text();
        assert!(editor.contains("Theme color editor"), "{editor}");
        shell.handle_key("right");
        if shell.palette_hex("base_background") == before {
            shell.handle_key("left");
        }
        assert_ne!(shell.palette_hex("base_background"), before);

        shell.handle_key("tab");
        let window = shell.visible_text();
        assert!(window.contains("settings-page:Window"), "{window}");
        assert!(window.contains("Density"), "{window}");
        assert!(window.contains("Vim"), "{window}");

        shell.handle_key("tab");
        let help = shell.visible_text();
        assert!(help.contains("settings-page:Help"), "{help}");
        assert!(help.contains("Settings · F1 · F1"), "{help}");
        assert!(help.contains("F3 Doctor"), "{help}");

        shell.handle_key("f1");
        let closed = shell.visible_text();
        assert!(!closed.contains("settings-panel"), "{closed}");
        assert_eq!(shell.palette_hex("base_background"), before);
        assert!(
            closed.contains("footer:F1: Settings  F3: Doctor"),
            "{closed}"
        );
    });
}

#[gpui::test]
fn escape_steps_the_color_editor_then_closes(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let shell = mount(cx, dir.path());
    shell.update(cx, |shell, _| {
        let before = shell.palette_hex("base_background");
        shell.handle_key("f1");
        let direct = shell.visible_text().contains("Theme color editor");
        if !direct {
            shell.handle_key("e");
            assert!(
                shell.visible_text().contains("Theme color editor"),
                "{}",
                shell.visible_text()
            );
            shell.handle_key("escape");
            let back = shell.visible_text();
            assert!(back.contains("theme-list:flow"), "{back}");
            assert!(back.contains("settings-panel:right:70"), "{back}");
        }
        shell.handle_key("escape");
        let closed = shell.visible_text();
        assert!(!closed.contains("settings-panel"), "{closed}");
        assert_eq!(shell.palette_hex("base_background"), before);
    });
}

#[gpui::test]
fn gui_json_round_trip_leaves_the_site_document(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let shell = mount(cx, dir.path());
    let sentinel = dir.path().join("sites.toml");
    let bytes = b"sites = {}\n# sentinel\n";
    fs::write(&sentinel, bytes).unwrap();
    shell.update(cx, |shell, _| {
        assert!(shell.vim_keys());
        shell.handle_key("f1");
        shell.handle_key("tab");
        shell.handle_key("down");
        shell.handle_key("enter");
        assert!(!shell.vim_keys());
    });
    let saved = fs::read_to_string(dir.path().join("gui.json")).unwrap();
    assert!(saved.contains("\"vim_keys\": false"), "{saved}");
    assert_eq!(fs::read(&sentinel).unwrap(), bytes);
}

#[gpui::test]
fn doctor_uses_the_local_report_and_does_not_spawn(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let shell = mount(cx, dir.path());
    shell.update(cx, |shell, _| {
        assert!(shell.app.state.skip_detect);
        assert!(shell.app.state.demo);
        assert!(shell.app.state.jobs.is_empty());
        shell.handle_key("f3");
        let text = shell.visible_text();
        assert!(text.contains("Doctor"), "{text}");
        assert!(shell.app.state.jobs.is_empty());
        assert!(shell.app.state.skip_detect);
    });
}

#[gpui::test]
fn quit_is_ctrl_q_and_the_tree_moves(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let shell = mount(cx, dir.path());
    shell.update(cx, |shell, _| {
        let start = shell.selected_label();
        shell.handle_key("q");
        assert!(!shell.quit_requested());
        assert_eq!(shell.selected_label(), start);
        shell.handle_key("down");
        assert_ne!(shell.selected_label(), start);
        shell.handle_key("k");
        assert_eq!(shell.selected_label(), start);
        shell.handle_key("j");
        assert_ne!(shell.selected_label(), start);
        shell.handle_key("ctrl-q");
        assert!(shell.quit_requested());
    });
}
