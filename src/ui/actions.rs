use crate::state::{AppState, FocusPane};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem};

pub fn draw(f: &mut Frame, state: &mut AppState, area: Rect, title: &str) {
    let focused = state.focus == FocusPane::Inspector;
    let items: Vec<ListItem> = state
        .actions
        .iter()
        .map(|a| {
            ListItem::new(Line::from(vec![
                Span::styled("▸ ", state.theme.label),
                Span::raw(a.label),
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(crate::ui::pane_block(title, focused, &state.theme))
        .highlight_style(state.theme.selected);
    f.render_stateful_widget(list, area, &mut state.action_state);
}
