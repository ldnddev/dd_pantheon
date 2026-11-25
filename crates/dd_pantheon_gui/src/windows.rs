//! App id and the main window. On Wayland `app_id` is the xdg app id.
//! On X11 it is the window class. The desktop file uses the same string.

use gpui::{
    App, Bounds, Pixels, SharedString, TitlebarOptions, WindowBounds, WindowOptions, px, size,
};

pub const MAIN_APP_ID: &str = "dev.ldnddev.dd_pantheon";
pub const WINDOW_TITLE: &str = "dd_pantheon";

pub fn main_window_options(bounds: Bounds<Pixels>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some(SharedString::from(WINDOW_TITLE)),
            ..Default::default()
        }),
        focus: true,
        show: true,
        app_id: Some(MAIN_APP_ID.to_string()),
        ..Default::default()
    }
}

pub fn open_bounds(cx: &App) -> Bounds<Pixels> {
    Bounds::centered(None, size(px(1280.0), px(800.0)), cx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[test]
    fn app_id_matches_the_desktop_class() {
        assert_eq!(MAIN_APP_ID, "dev.ldnddev.dd_pantheon");
        assert_eq!(WINDOW_TITLE, "dd_pantheon");
    }

    #[gpui::test]
    fn window_opens_at_1280_by_800_with_the_app_id(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let options = main_window_options(open_bounds(cx));
            assert_eq!(options.app_id.as_deref(), Some(MAIN_APP_ID));
            assert!(options.focus);
            assert!(options.show);
            let bounds = options.window_bounds.expect("bounds").get_bounds();
            assert_eq!(bounds.size, size(px(1280.0), px(800.0)));
            let title = options
                .titlebar
                .as_ref()
                .and_then(|bar| bar.title.as_ref())
                .map(SharedString::to_string);
            assert_eq!(title.as_deref(), Some(WINDOW_TITLE));
        });
    }
}
