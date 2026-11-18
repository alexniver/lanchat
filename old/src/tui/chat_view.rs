use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::state::{AppState, ChatMessage};
use crate::tui::app::UiState;

/// 渲染聊天视图
pub fn render_chat_view(
    f: &mut Frame,
    area: Rect,
    state: &AppState,
    ui: &UiState,
) {
    // 上下分割：消息列表 + 输入框
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),            // 消息区域占满剩余
            Constraint::Length(3),         // 输入框固定 3 行
        ])
        .split(area);

    // --- 消息列表 ---
    render_messages(f, chunks[0], state, ui);

    // --- 输入框 ---
    render_input(f, chunks[1], ui);
}

fn render_messages(f: &mut Frame, area: Rect, state: &AppState, ui: &UiState) {
    let msgs = &state.messages;
    let total = msgs.len();

    // 确定可见范围：从底部向上取，跳过 scroll 条
    let visible_count = area.height.saturating_sub(2) as usize; // 减掉上下边框
    let start = total.saturating_sub(visible_count).saturating_sub(ui.chat_scroll);
    let end = total.saturating_sub(ui.chat_scroll);

    let lines: Vec<Line> = msgs[start..end]
        .iter()
        .flat_map(|msg| match msg {
            ChatMessage::System { content, timestamp } => {
                let meta = format!("[{}] {}", format_timestamp(timestamp), content);
                vec![Line::styled(meta, Style::default().fg(Color::Gray))]
            }
            ChatMessage::User(chat) => {
                vec![
                    // 时间戳 + 发送者名
                    {
                        let meta = format!(
                            "[{}] {}",
                            format_timestamp(&chat.timestamp),
                            chat.from_name
                        );
                        let style = if chat.from_node_id == state.local_node.node_id {
                            Style::default().fg(Color::Cyan)
                        } else {
                            Style::default().fg(Color::Yellow)
                        };
                        Line::styled(meta, style)
                    },
                    // 消息内容
                    Line::from(Span::raw(&chat.content)),
                ]
            }
        })
        .collect();

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::NONE));

    f.render_widget(paragraph, area);
}

fn render_input(f: &mut Frame, area: Rect, ui: &UiState) {
    let block_border = Block::default()
        .borders(Borders::ALL)
        .title(" 输入 ");
    let inner = block_border.inner(area);

    // 光标位置视觉（input_cursor 是 UTF-8 字节位置，始终在字符边界上）
    let text = if ui.input.is_empty() {
        // 空输入时显示光标占位符
        Line::from(Span::styled("▌", Style::default().fg(Color::White)))
    } else {
        let cursor = ui.input_cursor.min(ui.input.len());
        let before = &ui.input[..cursor];

        // 获取光标位置的字符及其字节长度
        let (at, after) = if cursor < ui.input.len() {
            let ch = ui.input[cursor..].chars().next().unwrap();
            let ch_len = ch.len_utf8();
            (ch, &ui.input[cursor + ch_len..])
        } else {
            (' ', "")
        };

        let mut spans = vec![Span::raw(before.to_string())];
        spans.push(Span::styled(
            at.to_string(),
            Style::default().fg(Color::Black).bg(Color::White),
        ));
        if !after.is_empty() {
            spans.push(Span::raw(after.to_string()));
        }
        Line::from(spans)
    };

    let paragraph = Paragraph::new(text);
    f.render_widget(paragraph, inner);

    f.render_widget(block_border, area);
}

/// 将 RFC3339 时间戳简化为 HH:MM:SS 显示
fn format_timestamp(ts: &str) -> String {
    // 尝试取时间部分 "T10:30:00..." -> "10:30:00"
    if let Some(pos) = ts.find('T') {
        let time_part = &ts[pos + 1..];
        // 取前 8 个字符 HH:MM:SS
        if time_part.len() >= 8 {
            return time_part[..8].to_string();
        }
    }
    ts.to_string()
}
