pub mod input;
pub mod layout;
pub mod list;
pub mod status;
pub mod text;
pub mod theme;

use ratatui::layout::Alignment;
use ratatui::text::Text;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();
    let Some(areas) = layout::split(area, app.top_is_compose()) else {
        let paragraph =
            Paragraph::new(Text::from("终端太小（至少 60x15）")).alignment(Alignment::Center);
        f.render_widget(paragraph, area);
        return;
    };

    list::render(f, areas.list, app);
    if let Some(compose_area) = areas.compose {
        input::render(f, compose_area, app);
    }
    status::render(f, areas.status, app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::message::Message;
    use crate::app::test_app;
    use crate::key::Key;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::Terminal;

    fn buffer_text(buffer: &Buffer) -> String {
        let mut text = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                if let Some(cell) = buffer.cell((x, y)) {
                    text.push_str(cell.symbol());
                }
            }
            text.push('\n');
        }
        text
    }

    #[test]
    fn renders_two_panels_and_compose_panel() {
        let mut app = test_app(vec![Message::new("alice@arch", "你好")]);
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &app)).unwrap();
        let text = buffer_text(terminal.backend().buffer()).replace(' ', "");
        assert!(text.contains("信息列表"));
        assert!(text.contains("状态"));
        assert!(text.contains("你好"));
        assert!(text.contains("在线1（本机）"));
        assert!(text.contains("自动"));

        app.handle_key(Key::Char('i'));
        app.handle_key(Key::Char('中'));
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let text = buffer_text(terminal.backend().buffer()).replace(' ', "");
        assert!(text.contains("输入"));
        assert!(text.contains("输入中"));
        assert!(text.contains("中"));
    }

    #[test]
    fn renders_too_small_hint() {
        let app = test_app(Vec::new());
        let backend = TestBackend::new(40, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let text = buffer_text(terminal.backend().buffer()).replace(' ', "");
        assert!(text.contains("终端太小"));
    }
}
