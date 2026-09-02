use crate::models::LayoutId;
use crate::state::AppState;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

#[derive(Clone, Copy, Debug, Default)]
pub struct PaneRects {
    pub tree: Rect,
    pub inspector: Rect,
    pub preview: Rect,
    pub log: Rect,
}

pub fn split(layout: LayoutId, body: Rect, state: &AppState) -> PaneRects {
    if body.width < 80 {
        return split_narrow(body, state);
    }
    match layout {
        LayoutId::ClassicStack => split_a(body),
        LayoutId::ThreeColumn => split_b(body),
        LayoutId::TabbedInspector => split_c(body, state),
    }
}

fn split_a(body: Rect) -> PaneRects {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6),
            Constraint::Length(5),
            Constraint::Min(3),
        ])
        .split(body);
    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(2, 5), Constraint::Ratio(3, 5)])
        .split(rows[0]);
    PaneRects {
        tree: top[0],
        inspector: top[1],
        preview: rows[1],
        log: rows[2],
    }
}

fn split_b(body: Rect) -> PaneRects {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 4),
            Constraint::Ratio(2, 4),
            Constraint::Ratio(1, 4),
        ])
        .split(body);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(3)])
        .split(cols[2]);
    PaneRects {
        tree: cols[0],
        inspector: cols[1],
        preview: right[0],
        log: right[1],
    }
}

fn split_c(body: Rect, state: &AppState) -> PaneRects {
    let log_h = if state.log_collapsed() {
        1
    } else {
        6.min(body.height.saturating_sub(10)).max(3)
    };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6),
            Constraint::Length(5),
            Constraint::Length(log_h),
        ])
        .split(body);
    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(2, 5), Constraint::Ratio(3, 5)])
        .split(rows[0]);
    PaneRects {
        tree: top[0],
        inspector: top[1],
        preview: rows[1],
        log: rows[2],
    }
}

fn split_narrow(body: Rect, state: &AppState) -> PaneRects {
    let mut log_h: u16 = 2;
    let mut inspector_h: u16 = 6;
    let preview_h: u16 = 4;
    if body.height < 18 {
        if state.job_running || matches!(state.focus, crate::state::FocusPane::Log) {
            log_h = 2;
        } else {
            log_h = 0;
        }
    }
    if body.height < 14 {
        inspector_h = 3;
    }
    let tree_h = body
        .height
        .saturating_sub(preview_h)
        .saturating_sub(log_h)
        .saturating_sub(inspector_h)
        .max(3);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(tree_h.max(1)),
            Constraint::Length(inspector_h.max(1)),
            Constraint::Length(preview_h.max(3)),
            Constraint::Length(log_h),
        ])
        .split(body);
    PaneRects {
        tree: rows[0],
        inspector: rows[1],
        preview: rows[2],
        log: if log_h == 0 {
            Rect::new(body.x, body.y, 0, 0)
        } else {
            rows[3]
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigStore;
    use crate::fixtures::demo_data;
    use crate::state::AppState;
    use crate::theme::Theme;

    fn demo_state() -> AppState {
        let dir = std::env::temp_dir().join(format!("dd_pantheon_layout_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        AppState::from_demo(Theme::default(), ConfigStore::load(&dir), demo_data())
    }

    #[test]
    fn classic_keeps_preview_and_log() {
        let state = demo_state();
        let panes = split(LayoutId::ClassicStack, Rect::new(0, 0, 120, 40), &state);
        assert!(panes.preview.height >= 5);
        assert!(panes.log.height >= 3);
        assert!(panes.tree.width >= 24);
    }

    #[test]
    fn tabbed_idle_log_is_stub() {
        let mut state = demo_state();
        state.layout = LayoutId::TabbedInspector;
        state.focus = crate::state::FocusPane::Tree;
        assert!(state.log_collapsed());
        let panes = split(LayoutId::TabbedInspector, Rect::new(0, 0, 120, 40), &state);
        assert_eq!(panes.log.height, 1);
    }

    #[test]
    fn narrow_stacks_vertically() {
        let state = demo_state();
        let panes = split(LayoutId::ThreeColumn, Rect::new(0, 0, 70, 30), &state);
        assert_eq!(panes.tree.x, panes.inspector.x);
        assert!(panes.tree.y < panes.inspector.y);
        assert!(panes.preview.height >= 3);
    }
}
