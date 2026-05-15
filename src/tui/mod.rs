pub mod app;
pub mod chat_view;
pub mod files_view;

use std::io;
use std::sync::{Arc, Mutex};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Paragraph, Tabs},
    Frame, Terminal,
};
use tokio::sync::mpsc;

use crate::protocol::Message;
use crate::state::AppState;
use crate::tui::app::{Tab, UiState};

/// TUI 主循环入口：接管终端、循环处理事件和渲染
pub async fn run(
    state: Arc<Mutex<AppState>>,
    _app_tx: mpsc::Sender<Message>,
    _ui_rx: mpsc::Receiver<Message>,
) -> anyhow::Result<()> {
    // 进入 raw mode 和 alternate screen
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut ui = UiState::new();

    let result = main_loop(&mut terminal, state, &mut ui).await;

    // 恢复终端
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn main_loop(
    terminal: &mut Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>,
    state: Arc<Mutex<AppState>>,
    ui: &mut UiState,
) -> anyhow::Result<()> {
    loop {
        // 渲染
        {
            let app_state = state.lock().unwrap();
            terminal.draw(|f| render_ui(f, &app_state, ui))?;
        }

        // 处理事件（非阻塞，带超时）
        if event::poll(std::time::Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Release {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => break,
                    KeyCode::Tab => ui.active_tab = ui.active_tab.toggle(),
                    _ => match ui.active_tab {
                        Tab::Chat => handle_chat_key(key.code, ui),
                        Tab::Files => handle_files_key(key.code, ui, &state.lock().unwrap()),
                    },
                }
            }
        }
    }

    Ok(())
}

fn handle_chat_key(code: KeyCode, ui: &mut UiState) {
    match code {
        KeyCode::Char(ch) => ui.input_insert(ch),
        KeyCode::Backspace => ui.input_backspace(),
        KeyCode::Delete => ui.input_delete(),
        KeyCode::Left => ui.cursor_left(),
        KeyCode::Right => ui.cursor_right(),
        KeyCode::Enter => {
            // Phase 1: 仅清空输入框，不发送消息
            ui.input.clear();
            ui.input_cursor = 0;
        }
        KeyCode::Up => {
            // 上键：消息列表向上滚动
            // 需要在合适的时机传 state，但在 handle_chat_key 这里我们无法直接获取
            // 先占位
        }
        KeyCode::Down => ui.chat_scroll_down(),
        KeyCode::PageUp => {
            // PageUp: 向上滚动多行
        }
        KeyCode::PageDown => {
            // PageDown: 向下滚动
            ui.chat_scroll_bottom();
        }
        _ => {}
    }
}

fn handle_files_key(code: KeyCode, ui: &mut UiState, state: &AppState) {
    let max_nodes = 1 + state.peers.len();

    match code {
        KeyCode::Up => ui.files_select_up(),
        KeyCode::Down => ui.files_select_down(max_nodes),
        KeyCode::Enter | KeyCode::Char('s') => {
            // Phase 1: 触发下载逻辑占位
        }
        KeyCode::Char('r') => {
            // Phase 1: 刷新文件列表占位
        }
        _ => {}
    }
}

/// 渲染整个 UI
fn render_ui(f: &mut Frame, state: &AppState, ui: &UiState) {
    let area = f.area();

    // 整体布局：顶部状态栏 + 主内容区
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),  // 顶部状态栏
            Constraint::Min(1),     // 主内容
        ])
        .split(area);

    // --- 顶部状态栏 ---
    render_top_bar(f, chunks[0], state, ui);

    // --- 主内容 ---
    render_main_content(f, chunks[1], state, ui);
}

fn render_top_bar(f: &mut Frame, area: Rect, state: &AppState, ui: &UiState) {
    // 两行布局：Tab 行 + 信息行
    let bar_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

    // 左侧：Tab 标签
    let tabs = Tabs::new(vec![
        format!(" 聊天 "),
        format!(" 文件 "),
    ])
    .select(match ui.active_tab {
        Tab::Chat => 0,
        Tab::Files => 1,
    })
    .style(Style::default().fg(Color::White))
    .highlight_style(Style::default().fg(Color::Black).bg(Color::White));

    f.render_widget(tabs, bar_chunks[0]);

    // 右侧：本机节点信息 + 在线数
    let online_count = state.peers.values().filter(|p| p.online).count();
    let info = format!(
        " {} | 在线: {} ",
        state.local_node.display_name, online_count
    );
    let info_widget = Paragraph::new(info)
        .style(Style::default().fg(Color::Gray))
        .alignment(ratatui::layout::Alignment::Right);
    f.render_widget(info_widget, bar_chunks[1]);
}

fn render_main_content(f: &mut Frame, area: Rect, state: &AppState, ui: &UiState) {
    match ui.active_tab {
        Tab::Chat => chat_view::render_chat_view(f, area, state, ui),
        Tab::Files => files_view::render_files_view(f, area, state, ui),
    }
}
