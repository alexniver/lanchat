use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

use crate::protocol::FileEntry;
use crate::state::AppState;
use crate::tui::app::UiState;

/// 同步状态枚举
#[derive(Debug, PartialEq, Eq)]
enum SyncStatus {
    Synced,
    Available,
    Updated,
}

/// 渲染文件视图
pub fn render_files_view(
    f: &mut Frame,
    area: Rect,
    state: &AppState,
    ui: &UiState,
) {
    // 左右分栏
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),  // 左栏节点列表
            Constraint::Percentage(70),  // 右栏文件列表
        ])
        .split(area);

    // --- 左栏：节点列表 ---
    render_node_list(f, chunks[0], state, ui);

    // --- 右栏：文件列表 ---
    render_file_list(f, chunks[1], state, ui);
}

fn render_node_list(f: &mut Frame, area: Rect, state: &AppState, ui: &UiState) {
    // 构建节点列表项
    let items: Vec<ListItem> = {
        // 始终先显示本机
        let mut all: Vec<(String, bool)> = Vec::new();
        all.push((state.local_node.display_name.clone(), true));

        for peer in state.peers.values() {
            all.push((peer.display_name.clone(), peer.online));
        }

        all.iter()
            .enumerate()
            .map(|(i, (name, online))| {
                let icon = if *online { "●" } else { "○" };
                let label = if i == 0 {
                    format!("{} {} (本机)", icon, name)
                } else if !online {
                    format!("{} {} (离线)", icon, name)
                } else {
                    format!("{} {}", icon, name)
                };

                if i == ui.files_node_idx {
                    ListItem::new(label).style(Style::default().fg(Color::Black).bg(Color::White))
                } else {
                    let color = if *online { Color::Green } else { Color::DarkGray };
                    ListItem::new(label).style(Style::default().fg(color))
                }
            })
            .collect()
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" 节点 "));

    f.render_widget(list, area);
}

fn render_file_list(f: &mut Frame, area: Rect, state: &AppState, ui: &UiState) {
    // 获取当前选中节点的文件列表
    let files: Option<&Vec<FileEntry>> = if ui.files_node_idx == 0 {
        // 本机
        Some(&state.local_files)
    } else {
        // 远端节点
        let peer_names: Vec<&str> = state.peers.keys().map(|s| s.as_str()).collect();
        let peer_idx = ui.files_node_idx.saturating_sub(1);
        if let Some(node_id) = peer_names.get(peer_idx) {
            state.peer_files.get(*node_id)
        } else {
            None
        }
    };

    let items: Vec<ListItem> = match files {
        Some(list) if !list.is_empty() => {
            list.iter()
                .enumerate()
                .map(|(i, entry)| {
                    let status = sync_status(entry, state, ui.files_node_idx == 0);
                    let (status_text, status_color) = match status {
                        SyncStatus::Synced => ("[已同步]", Color::Green),
                        SyncStatus::Available => ("[同步]", Color::Blue),
                        SyncStatus::Updated => ("[有更新]", Color::Yellow),
                    };

                    let size_str = format_size(entry.size);
                    let label = format!(
                        "  {:<30} {:>8}  {}",
                        entry.name, size_str, status_text
                    );

                    if i == ui.files_file_idx {
                        ListItem::new(label)
                            .style(Style::default().fg(Color::Black).bg(Color::White))
                    } else {
                        let spans = Line::from(vec![
                            Span::raw(format!("  {:<30} {:>8}  ", entry.name, size_str)),
                            Span::styled(status_text, Style::default().fg(status_color)),
                        ]);
                        ListItem::new(spans)
                    }
                })
                .collect()
        }
        _ => {
            let hint = if ui.files_node_idx == 0 {
                "  将文件放入 ~/.local/share/lanchat/ 即可在此显示"
            } else {
                "  尚未获取该节点的文件列表"
            };
            vec![ListItem::new(hint).style(Style::default().fg(Color::DarkGray))]
        }
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" 文件 "));

    f.render_widget(list, area);
}

/// 判断文件的本地同步状态
fn sync_status(entry: &FileEntry, state: &AppState, is_local: bool) -> SyncStatus {
    if is_local {
        return SyncStatus::Synced;
    }

    if state.downloading.contains(&entry.name) {
        // 下载中不改变颜色但标记不可用，返回 Available 以避免干扰
        return SyncStatus::Available;
    }

    match state.local_files.iter().find(|f| f.name == entry.name) {
        Some(local) if local.sha256 == entry.sha256 => SyncStatus::Synced,
        Some(_) => SyncStatus::Updated,
        None => SyncStatus::Available,
    }
}

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{:.0}{}", size, UNITS[unit_idx])
    } else {
        format!("{:.1}{}", size, UNITS[unit_idx])
    }
}
