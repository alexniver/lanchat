use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use tokio::sync::mpsc;

use crate::protocol::{ChatPayload, FileEntry, Message};

/// 本机节点标识
#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub node_id: String,
    pub display_name: String,
}

/// 单个 peer 的连接状态
#[derive(Debug)]
pub struct PeerState {
    pub node_id: String,
    pub display_name: String,
    pub online: bool,
    /// 该 peer 的 socket 地址
    pub addr: Option<SocketAddr>,
    /// 向该 peer 发送消息的通道
    pub tx: Option<mpsc::Sender<Message>>,
}

/// 聊天消息（用户消息或系统通知）
#[derive(Debug, Clone)]
pub enum ChatMessage {
    /// 系统通知，如 "lin@arch 已连接" / "lin@arch 已断开"
    System {
        content: String,
        timestamp: String,
    },
    /// 用户聊天消息
    User(ChatPayload),
}

/// 全局共享状态
pub struct AppState {
    pub local_node: NodeInfo,
    pub peers: HashMap<String, PeerState>,
    pub messages: Vec<ChatMessage>,
    pub seen_msg_ids: HashSet<String>,
    pub peer_files: HashMap<String, Vec<FileEntry>>,
    pub local_files: Vec<FileEntry>,
    pub downloading: HashSet<String>,
}

impl AppState {
    /// 当前在线节点总数（包含本机）
    pub fn online_count(&self) -> usize {
        self.peers.values().filter(|p| p.online).count() + 1
    }

    /// 登记一个新发现的 peer（尚未连接）
    pub fn register_discovered_peer(&mut self, node_id: String, display_name: String, addr: SocketAddr) {
        if self.peers.contains_key(&node_id) {
            if let Some(p) = self.peers.get_mut(&node_id) {
                p.addr = Some(addr);
                p.display_name = display_name;
            }
        } else {
            self.peers.insert(
                node_id.clone(),
                PeerState {
                    node_id,
                    display_name,
                    online: false,
                    addr: Some(addr),
                    tx: None,
                },
            );
        }
    }

    /// 标记 peer 为已连接（握手完成）
    pub fn mark_peer_connected(
        &mut self,
        node_id: &str,
        display_name: &str,
        addr: SocketAddr,
        tx: mpsc::Sender<Message>,
    ) {
        if let Some(p) = self.peers.get_mut(node_id) {
            p.online = true;
            p.display_name = display_name.to_string();
            p.addr = Some(addr);
            p.tx = Some(tx);
        } else {
            self.peers.insert(
                node_id.to_string(),
                PeerState {
                    node_id: node_id.to_string(),
                    display_name: display_name.to_string(),
                    online: true,
                    addr: Some(addr),
                    tx: Some(tx),
                },
            );
        }
    }

    /// 彻底移除 peer（从 HashMap 中删除，含文件列表）
    pub fn remove_peer(&mut self, node_id: &str) {
        self.peers.remove(node_id);
        self.peer_files.remove(node_id);
    }

    /// 根据 socket 地址查找 peer 的 node_id
    pub fn find_node_by_addr(&self, addr: &SocketAddr) -> Option<String> {
        self.peers
            .iter()
            .find(|(_, p)| p.addr.as_ref() == Some(addr))
            .map(|(id, _)| id.clone())
    }
}