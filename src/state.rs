use std::collections::{HashMap, HashSet};
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
    pub tx: Option<mpsc::Sender<Message>>,
}

/// 全局共享状态
pub struct AppState {
    pub local_node: NodeInfo,
    pub peers: HashMap<String, PeerState>,
    pub messages: Vec<ChatPayload>,
    pub seen_msg_ids: HashSet<String>,
    pub peer_files: HashMap<String, Vec<FileEntry>>,
    pub local_files: Vec<FileEntry>,
    pub downloading: HashSet<String>,
}
