use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::ui::theme;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default().borders(Borders::ALL).title(" 帮助 ");
    let Some(help) = app.help() else {
        f.render_widget(block, area);
        return;
    };
    let lines: Vec<Line<'static>> = help
        .lines
        .iter()
        .map(|line| {
            if line.starts_with("LanChat ") {
                Line::from(line.clone()).style(
                    Style::default()
                        .fg(theme::SUCCESS)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Line::from(line.clone())
            }
        })
        .collect();
    let scroll = help.offset.min(u16::MAX as usize) as u16;
    f.render_widget(Paragraph::new(lines).block(block).scroll((scroll, 0)), area);
}
