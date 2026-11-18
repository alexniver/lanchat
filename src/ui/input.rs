use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{App, Layer};
use crate::ui::text::{char_width, display_width};

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let Some(Layer::Compose(compose)) = app.layers.last() else {
        return;
    };
    let block = Block::default().borders(Borders::ALL).title(" 输入 ");
    let inner = block.inner(area);
    if inner.width == 0 || inner.height == 0 {
        f.render_widget(block, area);
        return;
    }

    let text = &compose.input.text;
    let cursor = compose.input.cursor.min(text.len());
    let width = inner.width as usize;
    let start = scroll_start(text, cursor, width);
    let mut before = String::new();
    let mut after = String::new();
    let mut cursor_char = None;
    let mut used = 0;
    let mut cursor_rendered = false;

    for (byte, ch) in text.char_indices() {
        if byte < start {
            continue;
        }
        let ch_width = char_width(ch);
        if used + ch_width > width && used > 0 {
            break;
        }
        if byte < cursor {
            before.push(ch);
            used += ch_width;
        } else if byte == cursor {
            cursor_char = Some(ch);
            used += ch_width;
            cursor_rendered = true;
        } else if cursor_rendered {
            after.push(ch);
            used += ch_width;
        }
    }

    let cursor_style = Style::default().add_modifier(Modifier::REVERSED);
    let mut spans = vec![Span::raw(before)];
    if let Some(ch) = cursor_char {
        spans.push(Span::styled(ch.to_string(), cursor_style));
    } else if used < width {
        spans.push(Span::styled("▌", cursor_style));
    }
    if !after.is_empty() {
        spans.push(Span::raw(after));
    }

    let line = Line::from(spans);
    f.render_widget(Paragraph::new(line).block(block), area);
}

fn scroll_start(text: &str, cursor: usize, width: usize) -> usize {
    if width == 0 {
        return 0;
    }
    let mut start = 0;
    let cursor_width = display_width(&text[..cursor]);
    while start < cursor {
        let start_width = display_width(&text[..start]);
        if cursor_width.saturating_sub(start_width) < width {
            break;
        }
        let Some(ch) = text[start..].chars().next() else {
            break;
        };
        start += ch.len_utf8();
    }
    start
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrolls_to_keep_cursor_visible() {
        assert_eq!(scroll_start("abcdef", 6, 3), 4);
        assert_eq!(
            scroll_start("你好世界", "你好世界".len(), 4),
            "你好世".len()
        );
        assert_eq!(scroll_start("abc", 0, 3), 0);
    }
}
