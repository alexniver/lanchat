use std::time::Instant;

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{App, StatusKind};
use crate::net::ConnectionState;
use crate::ui::text::truncate_display;
use crate::ui::theme;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default().borders(Borders::ALL).title(" 状态 ");
    let inner = block.inner(area);
    if inner.width == 0 || inner.height == 0 {
        f.render_widget(block, area);
        return;
    }
    let text = truncate_display(&status_text(app), inner.width as usize);
    let style = Style::default().fg(status_color(app));
    f.render_widget(Paragraph::new(text).style(style).block(block), area);
}

pub fn status_text(app: &App) -> String {
    if app.top_is_help() {
        return help_status(app);
    }
    let message_count = app.messages.len();
    let selected = app.list().and_then(|list| list.selected);
    let position = selected
        .map(|index| format!("{}/{}", index + 1, message_count))
        .unwrap_or_else(|| "-/-".to_string());
    let mode = if app.list().map(|list| list.follow_latest).unwrap_or(true) {
        "自动"
    } else {
        "手动"
    };

    let mut parts = if app.top_is_compose() {
        vec![
            "输入中".to_string(),
            "Enter 发送".to_string(),
            "Esc 取消".to_string(),
            format!("消息 {message_count}"),
        ]
    } else {
        let mut parts = vec![
            format!("在线 {}", app.online_count()),
            format!("消息 {message_count}"),
            format!("选中 {position}"),
            mode.to_string(),
        ];
        if app.network_enabled() {
            parts.push(connection_text(app.connection, true).to_string());
        }
        if app.unread > 0 && !app.list().map(|list| list.follow_latest).unwrap_or(true) {
            parts.push(format!("新消息 {}", app.unread));
        }
        parts
    };

    if let Some(status) = &app.status {
        if !status.is_expired(Instant::now()) {
            parts.push(status.text.clone());
        }
    }
    parts.join(" · ")
}

pub fn help_status(app: &App) -> String {
    let Some(help) = app.help() else {
        return "按 Esc 退出帮助".to_string();
    };
    let position = if help.max_offset == 0 {
        "全部显示".to_string()
    } else {
        format!("{}/{}", help.offset + 1, help.lines.len())
    };
    format!(
        "按 Esc 退出帮助 · j/k 滚动 · 帮助 {position} · LanChat {}",
        env!("CARGO_PKG_VERSION")
    )
}

pub fn connection_text(state: ConnectionState, enabled: bool) -> &'static str {
    if !enabled {
        return "";
    }
    match state {
        ConnectionState::Online => "● online",
        ConnectionState::Connecting => "◌ connecting",
        ConnectionState::Reconnecting => "◌ reconnecting",
        ConnectionState::Offline => "! offline",
        ConnectionState::Local => "local",
    }
}

fn status_color(app: &App) -> ratatui::style::Color {
    app.status
        .as_ref()
        .map(|status| match status.kind {
            StatusKind::Info => theme::INFO,
            StatusKind::Success => theme::SUCCESS,
            StatusKind::Error => theme::ERROR,
        })
        .unwrap_or(theme::INFO)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Message;
    use crate::config::{Config, HistoryConfig, Identity, Paths, UiConfig};
    use crate::key::Keymap;
    use crate::storage::HistoryHandle;
    use std::path::PathBuf;

    fn app_with(messages: Vec<Message>) -> App {
        let config = Config {
            paths: Paths {
                config_dir: PathBuf::from("/tmp/lanchat-status-test"),
                config_file: PathBuf::from("/tmp/lanchat-status-test/config.toml"),
                keymap_file: PathBuf::from("/tmp/lanchat-status-test/keymap.toml"),
                identity_file: PathBuf::from("/tmp/lanchat-status-test/identity.json"),
                data_dir: PathBuf::from("/tmp/lanchat-status-test/data"),
                history_dir: PathBuf::from("/tmp/lanchat-status-test/history"),
                state_dir: PathBuf::from("/tmp/lanchat-status-test/state"),
                log_dir: PathBuf::from("/tmp/lanchat-status-test/logs"),
                panic_log: PathBuf::from("/tmp/lanchat-status-test/panic.log"),
            },
            identity: Identity {
                node_id: "node".into(),
                name: "alice".into(),
            },
            history: HistoryConfig {
                enabled: false,
                ..HistoryConfig::default()
            },
            ui: UiConfig {
                show_help_hint: false,
            },
            network: crate::config::NetworkConfig {
                enabled: false,
                ..Default::default()
            },
            warnings: Vec::new(),
        };
        App::new(
            config,
            Keymap::defaults(),
            HistoryHandle::disabled(),
            messages,
            Vec::new(),
        )
    }

    #[test]
    fn list_status_contains_mode_and_position() {
        let app = app_with(vec![Message::new("alice", "one")]);
        let text = status_text(&app);
        assert!(text.contains("在线 1"));
        assert!(text.contains("选中 1/1"));
        assert!(text.contains("自动"));
    }

    #[test]
    fn list_status_switches_to_manual() {
        let mut app = app_with(vec![
            Message::new("alice", "one"),
            Message::new("alice", "two"),
        ]);
        app.handle_key(crate::key::Key::Char('k'));
        let text = status_text(&app);
        assert!(text.contains("选中 1/2"));
        assert!(text.contains("手动"));
    }

    #[test]
    fn compose_status_mentions_send_and_cancel() {
        let mut app = app_with(vec![]);
        app.handle_key(crate::key::Key::Char('i'));
        let text = status_text(&app);
        assert!(text.contains("输入中"));
        assert!(text.contains("Enter 发送"));
        assert!(text.contains("Esc 取消"));
    }
}
