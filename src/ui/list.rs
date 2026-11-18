use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{App, Message};
use crate::ui::text::wrap_text;
use crate::ui::theme;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default().borders(Borders::ALL).title(" 信息列表 ");
    let inner = block.inner(area);
    if inner.width == 0 || inner.height == 0 {
        f.render_widget(block, area);
        return;
    }

    let selected_index = app.list().and_then(|list| list.selected);
    let highlight = if app.top_is_compose() || app.top_is_members() {
        None
    } else {
        selected_index
    };
    let (lines, ranges) = build_lines(&app.messages, inner.width as usize, highlight);
    let offset = scroll_offset(&ranges, selected_index, inner.height as usize, lines.len());

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((offset.min(u16::MAX as usize) as u16, 0));
    f.render_widget(paragraph, area);
}

fn build_lines(
    messages: &[Message],
    width: usize,
    selected: Option<usize>,
) -> (Vec<Line<'static>>, Vec<(usize, usize)>) {
    let mut lines = Vec::new();
    let mut ranges = Vec::with_capacity(messages.len());
    if messages.is_empty() {
        lines.push(Line::from("按 i 输入第一条消息").style(Style::default().fg(theme::MUTED)));
    }
    for (index, message) in messages.iter().enumerate() {
        let start = lines.len();
        let style = if selected == Some(index) {
            Style::default()
                .bg(theme::SELECTED_BG)
                .fg(theme::SELECTED_FG)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        for text in wrap_text(&message.format_line(), width) {
            lines.push(Line::from(text).style(style));
        }
        ranges.push((start, lines.len().saturating_sub(1)));
    }
    (lines, ranges)
}

fn scroll_offset(
    ranges: &[(usize, usize)],
    selected: Option<usize>,
    visible_height: usize,
    total_lines: usize,
) -> usize {
    let visible = visible_height.max(1);
    let max_offset = total_lines.saturating_sub(visible);
    let Some(index) = selected else {
        return max_offset;
    };
    let Some((first, last)) = ranges.get(index).copied() else {
        return max_offset;
    };
    let mut offset = (last + 1).saturating_sub(visible);
    if first < offset {
        offset = first;
    }
    offset.min(max_offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrolls_to_selected_message() {
        let ranges = vec![(0, 0), (1, 1), (2, 2), (3, 3)];
        assert_eq!(scroll_offset(&ranges, Some(3), 2, 4), 2);
        assert_eq!(scroll_offset(&ranges, Some(0), 2, 4), 0);
        assert_eq!(scroll_offset(&ranges, Some(1), 2, 4), 0);
        assert_eq!(scroll_offset(&ranges, Some(2), 2, 4), 1);
    }

    #[test]
    fn selected_message_style_applies_to_all_lines() {
        let message = Message::new("alice", "a long message that wraps");
        let messages = [message];
        let (lines, ranges) = build_lines(&messages, 10, Some(0));
        assert!(lines.len() > 1);
        assert_eq!(ranges[0], (0, lines.len() - 1));
        assert!(lines
            .iter()
            .all(|line| line.style.bg == Some(theme::SELECTED_BG)));
    }
}
