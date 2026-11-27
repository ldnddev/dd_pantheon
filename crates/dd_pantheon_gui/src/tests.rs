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
        assert!(
            text.contains("https://test-acme-wp.pantheonsite.io"),
            "{text}"
        );
        assert!(text.contains("series "), "{text}");
        assert!(text.contains("backup:create"), "{text}");
        assert!(text.contains("header:DEMO acme-wp.test"), "{text}");
        assert!(text.contains("focus:tree"), "{text}");
        assert!(text.contains("footer:F1: Settings  F3: Doctor"), "{text}");
        assert!(text.contains("Ctrl+Q: Quit"), "{text}");
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
        assert!(closed.contains("Ctrl+Q: Quit"), "{closed}");
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
        assert!(text.contains("modal:"), "{text}");
        assert!(text.contains("Doctor"), "{text}");
        assert!(text.contains("Tools"), "{text}");
        assert!(text.contains("whoami"), "{text}");
        assert!(shell.app.state.jobs.is_empty());
        assert!(shell.app.state.skip_detect);
        shell.handle_key("f3");
        assert!(
            !shell.visible_text().contains("modal:"),
            "{}",
            shell.visible_text()
        );
    });
}

#[gpui::test]
fn phase_panes_filter_and_palette_do_not_spawn(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let shell = mount(cx, dir.path());
    shell.update(cx, |shell, _| {
        shell.handle_key("tab");
        assert!(shell.visible_text().contains("focus:inspector"));
        shell.handle_key("shift-tab");
        assert!(shell.visible_text().contains("focus:tree"));

        shell.handle_key("/");
        shell.handle_key("f");
        shell.handle_key("r");
        shell.handle_key("o");
        shell.handle_key("z");
        let filtered = shell.visible_text();
        assert!(filtered.contains("Filter /froz"), "{filtered}");
        assert!(filtered.contains("frozen-lab"), "{filtered}");
        shell.handle_key("enter");
        assert!(
            shell.selected_label().contains("frozen-lab"),
            "{}",
            shell.selected_label()
        );
        assert!(shell.app.state.jobs.is_empty());

        shell.handle_key(":");
        let palette = shell.visible_text();
        assert!(palette.contains("Palette"), "{palette}");
        for ch in ["e", "n", "v", ":", "w", "i", "p", "e"] {
            shell.handle_key(ch);
        }
        let narrowed = shell.visible_text();
        assert!(narrowed.contains("env:wipe"), "{narrowed}");
        shell.handle_key("enter");
        let form = shell.visible_text();
        assert!(form.contains("env:wipe"), "{form}");
        assert!(form.contains("site_env frozen-lab.dev"), "{form}");
        shell.click_modal_line(2);
        let focus = match &shell.app.state.modal {
            Some(dd_pantheon::state::Modal::Palette {
                form: Some(form), ..
            }) => form.focus,
            other => panic!("palette form still open: {other:?}"),
        };
        assert_eq!(focus, 1);
        shell.handle_key("enter");
        let staged = shell.visible_text();
        assert!(
            staged.contains("env:wipe") || staged.contains("wipe"),
            "{staged}"
        );
        assert!(!staged.contains("modal:"), "{staged}");
        assert!(shell.app.state.jobs.is_empty());
    });
}

#[gpui::test]
fn phase_gates_login_and_quit_confirm(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let shell = mount(cx, dir.path());
    shell.update(cx, |shell, _| {
        shell.handle_key("ctrl-l");
        let login = shell.visible_text();
        assert!(login.contains("toast:terminus not on PATH"), "{login}");
        assert!(!login.contains("modal:"), "{login}");
        assert!(shell.app.state.jobs.is_empty());

        shell.handle_key("tab");
        shell.handle_key("tab");
        assert!(shell.visible_text().contains("focus:preview"));
        shell.handle_key("enter");
        let ran = shell.visible_text();
        assert!(ran.contains("toast:demo: no spawn"), "{ran}");
        assert!(shell.app.state.jobs.is_empty());

        shell.app.state.modal = Some(dd_pantheon::state::Modal::LiveGate {
            plan: dd_pantheon::plan::CommandPlan {
                tool: dd_pantheon::plan::ToolKind::Terminus,
                binary: std::path::PathBuf::from("terminus"),
                argv: vec!["env:wipe".into(), "acme-wp.live".into()],
                cwd: None,
                why: "wipe live".into(),
                safety: dd_pantheon::plan::SafetyTier::LiveGate,
                target: dd_pantheon::plan::PlanTarget::Env {
                    site: "acme-wp".into(),
                    env: "live".into(),
                },
                dry_run: true,
                timeout: None,
                expects_json: false,
                extra_env: vec![],
                redact: vec![],
                confirm_with_yes: true,
            },
            expected: "live".into(),
            typed: String::new(),
        });
        shell.handle_key("f1");
        let gate = shell.visible_text();
        assert!(gate.contains("LiveGate"), "{gate}");
        assert!(!gate.contains("settings-panel"), "{gate}");
        assert!(
            gate.contains("Enter does nothing until the word matches"),
            "{gate}"
        );
        assert!(gate.contains("type live"), "{gate}");
        assert!(!gate.contains("Enter: Run"), "{gate}");
        shell.handle_key("x");
        let mismatch = shell.visible_text();
        assert!(mismatch.contains("That does not match"), "{mismatch}");
        shell.handle_key("backspace");
        for ch in ["l", "i", "v", "e"] {
            shell.handle_key(ch);
        }
        let matched = shell.visible_text();
        assert!(matched.contains("Enter runs this command"), "{matched}");
        assert!(matched.contains("Enter: Run"), "{matched}");
        shell.handle_key("enter");
        assert!(shell.app.state.jobs.is_empty());
        assert!(
            shell.visible_text().contains("toast:dry_run: no spawn")
                || !shell.visible_text().contains("modal:")
        );

        shell.app.state.job_running = true;
        shell.handle_key("ctrl-q");
        assert!(!shell.quit_requested());
        assert!(shell.visible_text().contains("A job is still running"));
        shell.handle_key("escape");
        assert!(!shell.quit_requested());
        shell.app.state.job_running = false;
    });
}

#[gpui::test]
fn phase_workflows_stage_without_spawning(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let shell = mount(cx, dir.path());
    shell.update(cx, |shell, _| {
        shell.handle_key("w");
        assert_eq!(
            shell.app.state.metrics_period,
            dd_pantheon::models::MetricsPeriod::Week
        );
        shell.handle_key("d");
        assert_eq!(
            shell.app.state.metrics_period,
            dd_pantheon::models::MetricsPeriod::Day
        );
        shell.handle_key("shift-m");
        assert_eq!(
            shell.app.state.metrics_period,
            dd_pantheon::models::MetricsPeriod::Month
        );
        assert!(
            shell.visible_text().contains("[M]"),
            "{}",
            shell.visible_text()
        );

        shell.handle_key("m");
        let cms = shell.visible_text();
        assert!(cms.contains("CMS"), "{cms}");
        assert!(cms.contains("cms wp"), "{cms}");
        assert!(cms.contains("target local"), "{cms}");
        shell.handle_key("escape");

        shell.handle_key("e");
        let deploy = shell.visible_text();
        assert!(deploy.contains("Deploy to acme-wp.test"), "{deploy}");
        assert!(deploy.contains("--sync-content"), "{deploy}");
        shell.handle_key("escape");

        shell.handle_key("n");
        let create = shell.visible_text();
        assert!(create.contains("Create site"), "{create}");
        assert!(create.contains("org ldnddev"), "{create}");
        assert!(create.contains("upstream WordPress"), "{create}");
        shell.handle_key("escape");

        shell.handle_key("shift-t");
        let tags = shell.visible_text();
        assert!(tags.contains("Tag filter"), "{tags}");
        assert!(tags.contains("prod"), "{tags}");
        assert!(tags.contains("client-acme"), "{tags}");
        shell.handle_key("escape");

        shell.handle_key("a");
        shell.handle_key("q");
        shell.handle_key("a");
        shell.handle_key("enter");
        let added = shell.visible_text();
        assert!(added.contains("toast:demo: no spawn"), "{added}");
        assert!(shell.app.state.jobs.is_empty());

        shell.handle_key("b");
        let backup = shell.visible_text();
        assert!(backup.contains("backup:create"), "{backup}");
        assert!(backup.contains("toast:demo: no spawn"), "{backup}");
        assert!(shell.app.state.jobs.is_empty());

        shell.preview_click("cancel");
        assert!(
            shell.visible_text().contains("no plan staged")
                || shell.visible_text().contains("plan cancelled")
        );
        assert!(shell.app.state.jobs.is_empty());
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

#[gpui::test]
fn phase_tag_chips_and_inspector_scroll(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let shell = mount(cx, dir.path());
    shell.update(cx, |shell, _| {
        assert_eq!(present_title(&shell.app.state), "Available sites (3)");
        shell.click_tag("prod", false);
        assert_eq!(shell.app.state.tag_filter.as_deref(), Some("prod"));
        assert!(shell.app.state.focus == dd_pantheon::state::FocusPane::Inspector);
        let titled = present_title(&shell.app.state);
        assert!(titled.contains("tag:prod"), "{titled}");
        assert!(
            !shell.visible_text().contains("frozen-lab"),
            "{}",
            shell.visible_text()
        );
        assert!(shell.app.state.jobs.is_empty());

        shell.app.state.selected_chip = Some("prod".into());
        shell.click_tag("prod", true);
        let removed = shell.visible_text();
        assert!(removed.contains("toast:demo: no spawn"), "{removed}");
        assert!(shell.app.state.jobs.is_empty());
        assert!(shell.visible_text().contains("focus:inspector"));
        let before = shell.app.state.inspector_scroll;
        shell.handle_key("j");
        assert!(shell.app.state.inspector_scroll > before);
    });
}

fn present_title(state: &dd_pantheon::state::AppState) -> String {
    crate::present::tree_title(state)
}
