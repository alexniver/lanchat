use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState as WidgetListState};
use ratatui::Frame;

use crate::app::{App, Layer};
use crate::ui::theme;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default().borders(Borders::ALL).title(" 成员 ");
    let selected = match app.layers.last() {
        Some(Layer::Members(members)) => members.selected,
        _ => 0,
    };
    let mut items = Vec::new();
    items.push(member_item(
        &app.config.identity.name,
        "(you) · 本机",
        selected == 0,
    ));
    for (index, peer) in app.peers.values().enumerate() {
        items.push(member_item(
            &peer.name,
            &peer.addr.to_string(),
            selected == index + 1,
        ));
    }
    let mut state = WidgetListState::default();
    if !items.is_empty() {
        state.select(Some(selected.min(items.len() - 1)));
    }
    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(theme::SELECTED_BG)
            .fg(theme::SELECTED_FG)
            .add_modifier(Modifier::BOLD),
    );
    f.render_stateful_widget(list, area, &mut state);
}

fn member_item(name: &str, detail: &str, selected: bool) -> ListItem<'static> {
    let marker = if selected { "▌ " } else { "  " };
    let lines = vec![
        Line::from(vec![
            Span::raw(marker),
            Span::styled("● ", Style::default().fg(theme::SUCCESS)),
            Span::raw(name.to_string()),
        ]),
        Line::from(Span::styled(
            format!("   {detail}"),
            Style::default().fg(theme::MUTED),
        )),
    ];
    ListItem::new(lines)
}
