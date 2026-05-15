mod config;
mod network;
mod protocol;
mod state;
mod tui;

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::panic;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::Local;
use tokio::sync::mpsc;
use tracing_subscriber::fmt::layer;
use tracing_subscriber::prelude::*;
use tracing_appender::non_blocking::WorkerGuard;

use crate::network::AppEvent;
use crate::protocol::{FileRequestPayload, Message};
use crate::state::{AppState, ChatMessage, NodeInfo};

fn init_logging() -> WorkerGuard {
    std::fs::create_dir_all("log").ok();

    let now = Local::now();
    let log_file = format!("log/{}.log", now.format("%Y-%m-%d_%H-%M-%S"));
    let file = std::fs::File::create(&log_file).expect("failed to create log file");

    let (non_blocking, file_guard) = tracing_appender::non_blocking(file);

    // 文件日志：INFO 级别，但过滤掉 mdns_sd 第三方库的噪音
    let file_filter = tracing_subscriber::filter::FilterFn::new(|metadata| {
        if metadata.target().starts_with("mdns_sd") {
            return false; // 完全静默 mdns_sd
        }
        *metadata.level() <= tracing::Level::INFO
    });
    let file_layer = layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .with_filter(file_filter);

    tracing_subscriber::registry()
        .with(file_layer)
        .init();

    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("log/panic.log")
        {
            let now = Local::now();
            let _ = writeln!(
                f,
                "[{}] PANIC: {}",
                now.format("%Y-%m-%d %H:%M:%S%.3f"),
                info
            );
            if let Some(loc) = info.location() {
                let _ = writeln!(f, "  at {}:{}:{}", loc.file(), loc.line(), loc.column());
            }
            let backtrace = std::backtrace::Backtrace::force_capture();
            let _ = writeln!(f, "--- backtrace ---\n{backtrace}");
        }
        default_hook(info);
    }));

    tracing::info!("日志系统已初始化，日志文件: {}", log_file);

    file_guard
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _file_guard = init_logging();
    tracing::info!("lanchat 启动");

    // --- 配置 ---
    let app_config = config::parse_args();
    let share_dir = config::ensure_local_share_dir()?;

    // 启动时扫描本地文件
    let local_files = network::files::scan_local_files(&share_dir).unwrap_or_default();

    let local_node = NodeInfo {
        node_id: app_config.identity.node_id.clone(),
        display_name: app_config.identity.display_name.clone(),
    };

    let app_state = Arc::new(Mutex::new(AppState {
        local_node: local_node.clone(),
        peers: HashMap::new(),
        messages: Vec::new(),
        seen_msg_ids: HashSet::new(),
        peer_files: HashMap::new(),
        local_files,
        downloading: HashSet::new(),
    }));

    // --- Channel：AppEvent 总线 ---
    let (app_tx, mut app_rx) = mpsc::channel::<AppEvent>(512);

    // --- 启动 TCP Server ---
    let server_port = network::server::start(
        app_tx.clone(),
        local_node.node_id.clone(),
        local_node.display_name.clone(),
    )
    .await?;

    // --- 启动 mDNS 发现（独立线程） ---
    let _mdns_daemon = network::discovery::start(
        local_node.node_id.clone(),
        local_node.display_name.clone(),
        server_port,
        app_tx.clone(),
    )?;

    // --- 启动 Event Handler task ---
    let event_state = app_state.clone();
    let event_tx = app_tx.clone();
    let event_local_node = local_node.clone();
    let event_share_dir = share_dir.clone();
    tokio::spawn(async move {
        event_handler(
            event_state,
            event_local_node,
            server_port,
            event_tx,
            event_share_dir,
            &mut app_rx,
        )
        .await;
    });

    // --- 启动 TUI 主循环 ---
    // TUI 直接读取 app_state，app_tx 用于后续 Phase 5 聊天发送。
    tui::run(app_state, app_tx.clone()).await
}

/// Event Handler：处理所有 AppEvent，修改 AppState。
async fn event_handler(
    state: Arc<Mutex<AppState>>,
    local_node: NodeInfo,
    my_port: u16,
    app_tx: mpsc::Sender<AppEvent>,
    share_dir: PathBuf,
    rx: &mut mpsc::Receiver<AppEvent>,
) {
    tracing::info!("event_handler 已启动，等待事件...");
    while let Some(event) = rx.recv().await {
        tracing::info!("event_handler 收到事件: {:?}", event);
        match event {
            AppEvent::PeerDiscovered {
                node_id,
                display_name,
                addr,
            } => {
                // 去重：端口号较小的一方发起连接
                if network::connect::should_connect(my_port, addr.port()) {
                    tracing::info!(
                        "本机发起连接: {} -> {} ({})",
                        local_node.display_name,
                        display_name,
                        addr
                    );

                    let tx = app_tx.clone();
                    let my_nid = local_node.node_id.clone();
                    let my_name = local_node.display_name.clone();
                    tokio::spawn(async move {
                        if let Err(e) = network::connect::connect_and_handshake(
                            addr,
                            my_nid,
                            my_name,
                            tx,
                        )
                        .await
                        {
                            tracing::warn!("连接 {} 失败: {}", addr, e);
                        }
                    });
                } else {
                    tracing::info!(
                        "等待 {} 发起连接（本机 node_id 较大）",
                        display_name
                    );
                }

                // 记录到 state
                state
                    .lock()
                    .unwrap()
                    .register_discovered_peer(node_id, display_name, addr);
                tracing::info!(
                    "已登记发现的 peer，当前 peers: {:?}",
                    state.lock().unwrap().peers.keys().collect::<Vec<_>>()
                );
            }

            AppEvent::PeerVanished { node_id } => {
                tracing::info!("节点离线: {}", node_id);
                state.lock().unwrap().remove_peer(&node_id);
            }

            AppEvent::PeerConnected {
                node_id,
                display_name,
                addr,
                tx,
            } => {
                tracing::info!("节点已连接: {} ({}) @ {}", display_name, node_id, addr);
                state
                    .lock()
                    .unwrap()
                    .mark_peer_connected(&node_id, &display_name, addr, tx.clone());
                // 系统消息：xxx 已连接
                state.lock().unwrap().messages.push(ChatMessage::System {
                    content: format!("{} 已连接", display_name),
                    timestamp: chrono::Local::now().to_rfc3339(),
                });
                // 诊断：连接后立即确认 online_count
                let s = state.lock().unwrap();
                tracing::info!(
                    "DIAG: mark_peer_connected 后 online_count={}, peers keys: {:?}, 各 peer online: {:?}",
                    s.online_count(),
                    s.peers.keys().collect::<Vec<_>>(),
                    s.peers.iter().map(|(id, p)| (id, p.online)).collect::<Vec<_>>()
                );
                drop(s);

                // 握手完成后自动请求对方的文件列表
                let node_id_clone = node_id.clone();
                let tx_clone = tx.clone();
                tokio::spawn(async move {
                    // 稍等片刻确保对端已准备好接收消息
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    let _ = tx_clone.try_send(Message::FileListRequest);
                    tracing::info!("已向 {} 发送 FileListRequest", node_id_clone);
                });
            }

            AppEvent::PeerDisconnected { node_id } => {
                if node_id.is_empty() {
                    // node_id 为空，说明是 peer handler 断连时不知道 node_id
                    tracing::info!("一个 peer 连接断开");
                } else {
                    tracing::info!("节点断开: {}", node_id);
                    // 获取显示名后彻底删除节点（含 peer_files）
                    let name = state
                        .lock()
                        .unwrap()
                        .peers
                        .get(&node_id)
                        .map(|p| p.display_name.clone());
                    state.lock().unwrap().remove_peer(&node_id);
                    if let Some(name) = name {
                        state.lock().unwrap().messages.push(ChatMessage::System {
                            content: format!("{} 已断开", name),
                            timestamp: chrono::Local::now().to_rfc3339(),
                        });
                    }
                }
            }

            AppEvent::Message { msg, from } => {
                handle_network_message(&msg, from, &state, &local_node, &share_dir);
            }

            AppEvent::SendChat { content } => {
                let msg_id = uuid::Uuid::new_v4().to_string();
                let ts = chrono::Local::now().to_rfc3339();
                let chat = crate::protocol::ChatPayload {
                    msg_id: msg_id.clone(),
                    from_node_id: local_node.node_id.clone(),
                    from_name: local_node.display_name.clone(),
                    content,
                    timestamp: ts,
                };
                let mut s = state.lock().unwrap();
                s.seen_msg_ids.insert(msg_id);
                s.messages.push(ChatMessage::User(chat.clone()));
                // 广播给所有在线 peer
                for peer in s.peers.values() {
                    if peer.online {
                        if let Some(tx) = &peer.tx {
                            let _ = tx.try_send(Message::Chat(chat.clone()));
                        }
                    }
                }
            }

            AppEvent::RequestFileList { node_id } => {
                tracing::info!("请求 {} 的文件列表", node_id);
                let s = state.lock().unwrap();
                if let Some(peer) = s.peers.get(&node_id) {
                    if let Some(tx) = &peer.tx {
                        let _ = tx.try_send(Message::FileListRequest);
                    }
                }
            }

            AppEvent::DownloadFile {
                node_id,
                file_name,
            } => {
                tracing::info!("请求从 {} 下载文件: {}", node_id, file_name);
                let mut s = state.lock().unwrap();
                s.downloading.insert(file_name.clone());
                if let Some(peer) = s.peers.get(&node_id) {
                    if let Some(tx) = &peer.tx {
                        let _ = tx.try_send(Message::FileRequest(FileRequestPayload {
                            file_name,
                        }));
                    }
                }
            }
        }
    }
}

/// 处理从网络收到的单条消息。
fn handle_network_message(
    msg: &Message,
    from: std::net::SocketAddr,
    state: &Arc<Mutex<AppState>>,
    local_node: &NodeInfo,
    share_dir: &PathBuf,
) {
    match msg {
        Message::Hello(_) => {
            // 已在 PeerConnected 中处理，这里忽略
        }
        Message::Chat(chat) => {
            let mut s = state.lock().unwrap();
            if s.seen_msg_ids.contains(&chat.msg_id) {
                return;
            }
            s.seen_msg_ids.insert(chat.msg_id.clone());
            s.messages.push(ChatMessage::User(chat.clone()));
            // 广播给所有其他在线 peer
            for peer in s.peers.values() {
                if peer.online {
                    if let Some(tx) = &peer.tx {
                        let _ = tx.try_send(Message::Chat(chat.clone()));
                    }
                }
            }
        }
        Message::FileListRequest => {
            tracing::info!("收到 FileListRequest 来自 {}", from);
            // 扫描本地文件并回复
            let files = crate::network::files::scan_local_files(share_dir).unwrap_or_default();
            let s = state.lock().unwrap();
            // 找到请求方 node_id
            if let Some(node_id) = s.find_node_by_addr(&from) {
                if let Some(peer) = s.peers.get(&node_id) {
                    if let Some(tx) = &peer.tx {
                        let response = Message::FileListResponse(crate::protocol::FileListPayload {
                            node_id: local_node.node_id.clone(),
                            files,
                        });
                        let _ = tx.try_send(response);
                        tracing::info!("已向 {} 回复 FileListResponse ({} 个文件)", node_id, s.peer_files.get(&node_id).map(|f| f.len()).unwrap_or(0));
                    }
                }
            }
        }
        Message::FileListResponse(payload) => {
            tracing::info!(
                "收到 FileListResponse 来自 {} ({} 个文件)",
                payload.node_id,
                payload.files.len()
            );
            let mut s = state.lock().unwrap();
            s.peer_files
                .insert(payload.node_id.clone(), payload.files.clone());
        }
        Message::FileRequest(payload) => {
            tracing::info!("收到 FileRequest: {} 来自 {}", payload.file_name, from);
            // 读取本地文件，base64 编码后回复
            let file_path = share_dir.join(&payload.file_name);
            let response = if file_path.exists() && file_path.is_file() {
                match std::fs::read(&file_path) {
                    Ok(data) => {
                        use base64::Engine;
                        let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
                        let metadata = std::fs::metadata(&file_path).unwrap();
                        let sha256 = {
                            use sha2::Digest;
                            let mut hasher = sha2::Sha256::new();
                            hasher.update(&data);
                            format!("{:x}", hasher.finalize())
                        };
                        Message::FileResponse(crate::protocol::FileResponsePayload {
                            file_name: payload.file_name.clone(),
                            size: metadata.len(),
                            sha256,
                            data: encoded,
                            not_found: false,
                        })
                    }
                    Err(e) => {
                        tracing::warn!("读取文件 {} 失败: {}", file_path.display(), e);
                        Message::FileResponse(crate::protocol::FileResponsePayload {
                            file_name: payload.file_name.clone(),
                            size: 0,
                            sha256: String::new(),
                            data: String::new(),
                            not_found: true,
                        })
                    }
                }
            } else {
                Message::FileResponse(crate::protocol::FileResponsePayload {
                    file_name: payload.file_name.clone(),
                    size: 0,
                    sha256: String::new(),
                    data: String::new(),
                    not_found: true,
                })
            };

            let s = state.lock().unwrap();
            if let Some(node_id) = s.find_node_by_addr(&from) {
                if let Some(peer) = s.peers.get(&node_id) {
                    if let Some(tx) = &peer.tx {
                        let _ = tx.try_send(response);
                        tracing::info!("已向 {} 回复 FileResponse", node_id);
                    }
                }
            }
        }
        Message::FileResponse(payload) => {
            tracing::info!(
                "收到 FileResponse: {} (size={}, not_found={})",
                payload.file_name,
                payload.size,
                payload.not_found
            );
            if payload.not_found {
                tracing::warn!("远端文件不存在: {}", payload.file_name);
                state.lock().unwrap().downloading.remove(&payload.file_name);
                return;
            }

            // base64 解码
            use base64::Engine;
            let decoded = match base64::engine::general_purpose::STANDARD.decode(&payload.data) {
                Ok(d) => d,
                Err(e) => {
                    tracing::error!("base64 解码失败: {}", e);
                    state.lock().unwrap().downloading.remove(&payload.file_name);
                    return;
                }
            };

            // 处理文件名冲突：如果本地已存在同名文件且 sha256 不同，添加 .conflict 后缀
            let dest_path = {
                let existing = share_dir.join(&payload.file_name);
                if existing.exists() {
                    // 检查 sha256
                    let local_hash = match std::fs::read(&existing) {
                        Ok(data) => {
                            use sha2::Digest;
                            let mut hasher = sha2::Sha256::new();
                            hasher.update(&data);
                            format!("{:x}", hasher.finalize())
                        }
                        Err(_) => String::new(),
                    };
                    if local_hash != payload.sha256 && !local_hash.is_empty() {
                        // 冲突：添加 .conflict 后缀
                        // 分离文件名和扩展名
                        let conflict_name = if let Some(dot_pos) = payload.file_name.rfind('.') {
                            let (name, ext) = payload.file_name.split_at(dot_pos);
                            format!("{}.conflict{}", name, ext)
                        } else {
                            format!("{}.conflict", payload.file_name)
                        };
                        tracing::warn!(
                            "文件 {} SHA256 冲突，保存为 {}",
                            payload.file_name,
                            conflict_name
                        );
                        share_dir.join(&conflict_name)
                    } else {
                        existing
                    }
                } else {
                    existing
                }
            };

            match std::fs::write(&dest_path, &decoded) {
                Ok(_) => {
                    tracing::info!(
                        "文件 {} 已保存到 {}",
                        payload.file_name,
                        dest_path.display()
                    );
                    // 重新扫描本地文件列表
                    let new_files =
                        crate::network::files::scan_local_files(share_dir).unwrap_or_default();
                    let mut s = state.lock().unwrap();
                    s.local_files = new_files;
                    s.downloading.remove(&payload.file_name);
                }
                Err(e) => {
                    tracing::error!("写入文件 {} 失败: {}", dest_path.display(), e);
                    state.lock().unwrap().downloading.remove(&payload.file_name);
                }
            }
        }
        Message::Goodbye => {
            // 已在 PeerDisconnected 中处理
            tracing::debug!("收到 Goodbye 来自 {}", from);
        }
    }
}