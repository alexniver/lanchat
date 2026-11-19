use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Local;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};
use tokio::time::timeout;

use crate::config::{Identity, NetworkConfig};
use crate::net::discovery::{self, Discovery, DiscoveryEvent};
use crate::net::file::{self, IncomingFile, OutgoingFile};
use crate::net::protocol::{
    read_frame, validate_chat, validate_file_offer, validate_hello, write_frame, Bye, Chat,
    FileAccept, FileCancel, FileComplete, FileDataHello, FileError, FileOffer, FolderMeta, Hello,
    Ping, Pong, Ready, WireMessage, MAGIC, MAX_BODY_BYTES, PROTOCOL_VERSION, ROOM,
};

const CHANNEL_CAPACITY: usize = 64;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const PING_INTERVAL: Duration = Duration::from_secs(10);
const MAX_SEEN: usize = 10_000;
const MAX_PEERS: usize = 64;
const FILE_REQUEST_TIMEOUT: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Local,
    Connecting,
    Online,
    Reconnecting,
    Offline,
}

#[derive(Debug, Clone)]
pub enum NetCommand {
    SendChat {
        msg_id: String,
        body: String,
    },
    SendFile {
        node_id: String,
        transfer_id: String,
        path: PathBuf,
        name: String,
        size: u64,
        folder: Option<FolderMeta>,
    },
    AcceptFiles {
        node_id: String,
        transfer_ids: Vec<String>,
    },
    SendFolder {
        node_id: String,
        transfer_id: String,
        path: PathBuf,
        root_name: String,
        archive_size: u64,
        total_size: u64,
        file_count: u32,
        empty_dirs: u32,
    },
    SetName {
        name: String,
    },
    Reconnect,
    Disconnect,
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum NetEvent {
    PeerUp {
        node_id: String,
        name: String,
        short_no: u32,
        addr: SocketAddr,
    },
    PeerDown {
        node_id: String,
        reason: String,
    },
    ChatReceived(Chat),
    SendFailed {
        msg_id: String,
        reason: String,
    },
    FileOfferReceived {
        node_id: String,
        peer_name: String,
        addr: SocketAddr,
        transfer_id: String,
        file_name: String,
        size: u64,
        folder: Option<FolderMeta>,
    },
    FileOfferSent {
        node_id: String,
        transfer_id: String,
        file_name: String,
        folder: Option<FolderMeta>,
    },
    FileAccepted {
        node_id: String,
        transfer_ids: Vec<String>,
        is_folder: bool,
    },
    FileReceived {
        node_id: String,
        transfer_id: String,
        file_name: String,
        path: PathBuf,
        is_folder: bool,
    },
    FileSent {
        node_id: String,
        transfer_id: String,
        file_name: String,
        is_folder: bool,
    },
    FileFailed {
        node_id: String,
        transfer_id: String,
        file_name: String,
        reason: String,
        is_folder: bool,
    },
    FileCancelled {
        node_id: String,
        transfer_id: String,
        file_name: String,
        reason: String,
        is_folder: bool,
    },
    FileTimedOut {
        node_id: String,
        transfer_id: String,
        file_name: String,
        outgoing: bool,
        is_folder: bool,
    },
    FileCommandError {
        message: String,
    },
    LocalShortNoChanged {
        short_no: u32,
    },
    PeerShortNoChanged {
        node_id: String,
        short_no: u32,
    },
    PeerNameChanged {
        node_id: String,
        name: String,
    },
    Status {
        state: ConnectionState,
        detail: String,
    },
}

#[derive(Clone)]
pub struct NetHandle {
    tx: mpsc::Sender<NetCommand>,
    local_port: u16,
}

impl NetHandle {
    pub fn local_port(&self) -> u16 {
        self.local_port
    }

    pub async fn send(&self, command: NetCommand) -> bool {
        self.tx.send(command).await.is_ok()
    }
}

#[derive(Debug)]
enum PeerEvent {
    Connected {
        node_id: String,
        name: String,
        short_no: u32,
        addr: SocketAddr,
        listen_port: u16,
        capabilities: Vec<String>,
        stream: TcpStream,
    },
    DataHello {
        addr: SocketAddr,
        hello: FileDataHello,
        stream: TcpStream,
    },
    DataReceived {
        node_id: String,
        received: Vec<file::ReceivedFile>,
    },
    DataSent {
        node_id: String,
        transfer_ids: Vec<String>,
    },
    DataFailed {
        node_id: String,
        transfer_ids: Vec<String>,
        reason: String,
    },
    Message {
        node_id: String,
        message: WireMessage,
    },
    Closed {
        node_id: String,
        reason: String,
    },
    HandshakeFailed {
        addr: SocketAddr,
        node_id: Option<String>,
        reason: String,
    },
}

struct KnownPeer {
    addrs: Vec<SocketAddr>,
}

struct PeerEntry {
    #[allow(dead_code)]
    name: String,
    addr: SocketAddr,
    tx: mpsc::Sender<WireMessage>,
    close: watch::Sender<bool>,
    supports_files: bool,
    supports_short_no: bool,
    supports_name_update: bool,
    supports_folder: bool,
    short_no: u32,
    last_seen: Instant,
    last_ping: Instant,
}

struct OutgoingOffer {
    node_id: String,
    name: String,
    path: PathBuf,
    folder: Option<FolderMeta>,
    created: Instant,
    transferring: bool,
}

struct IncomingOffer {
    node_id: String,
    name: String,
    size: u64,
    folder: Option<FolderMeta>,
    created: Instant,
    accepted: bool,
}

struct ReconnectState {
    addr: SocketAddr,
    attempts: u32,
    next_at: Instant,
}

struct Manager {
    listener: TcpListener,
    local: Identity,
    identity_path: PathBuf,
    listen_port: u16,
    config: NetworkConfig,
    events: mpsc::Sender<NetEvent>,
    commands: mpsc::Receiver<NetCommand>,
    peer_tx: mpsc::Sender<PeerEvent>,
    peer_rx: mpsc::Receiver<PeerEvent>,
    discovery_rx: mpsc::Receiver<DiscoveryEvent>,
    _discovery: Option<Discovery>,
    _discovery_tx: Option<mpsc::Sender<DiscoveryEvent>>,
    known: HashMap<String, KnownPeer>,
    peers: HashMap<String, PeerEntry>,
    connecting_nodes: HashSet<String>,
    connecting_addrs: HashSet<SocketAddr>,
    reconnects: HashMap<String, ReconnectState>,
    deferred: HashMap<String, Instant>,
    files_dir: PathBuf,
    outgoing: HashMap<String, OutgoingOffer>,
    incoming: HashMap<String, IncomingOffer>,
    seen: HashSet<String>,
    seen_order: VecDeque<String>,
    seq: u64,
    manual_disconnect: bool,
    last_state: Option<ConnectionState>,
}

pub async fn start(
    config: NetworkConfig,
    identity: Identity,
    identity_path: PathBuf,
    files_dir: PathBuf,
) -> Result<(NetHandle, mpsc::Receiver<NetEvent>)> {
    let (event_tx, event_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (command_tx, command_rx) = mpsc::channel(CHANNEL_CAPACITY);
    if !config.enabled {
        return Ok((
            NetHandle {
                tx: command_tx,
                local_port: 0,
            },
            event_rx,
        ));
    }

    let listener = TcpListener::bind(("0.0.0.0", config.port))
        .await
        .with_context(|| format!("监听端口 {} 失败", config.port))?;
    let local_port = listener.local_addr()?.port();
    tokio::fs::create_dir_all(&files_dir)
        .await
        .with_context(|| format!("创建文件目录失败: {}", files_dir.display()))?;

    let (discovery_tx, discovery_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let discovery = if config.mdns {
        match discovery::start(
            identity.node_id.clone(),
            identity.name.clone(),
            local_port,
            discovery_tx.clone(),
        ) {
            Ok(handle) => Some(handle),
            Err(error) => {
                let _ = event_tx
                    .send(NetEvent::Status {
                        state: ConnectionState::Offline,
                        detail: format!("mDNS 不可用: {error}"),
                    })
                    .await;
                None
            }
        }
    } else {
        None
    };
    let keep_discovery_tx = discovery.is_none().then_some(discovery_tx);

    let (peer_tx, peer_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let mut manager = Manager {
        listener,
        local: identity,
        identity_path,
        listen_port: local_port,
        config: config.clone(),
        events: event_tx.clone(),
        commands: command_rx,
        peer_tx: peer_tx.clone(),
        peer_rx,
        discovery_rx,
        _discovery: discovery,
        _discovery_tx: keep_discovery_tx,
        known: HashMap::new(),
        peers: HashMap::new(),
        connecting_nodes: HashSet::new(),
        connecting_addrs: HashSet::new(),
        reconnects: HashMap::new(),
        deferred: HashMap::new(),
        files_dir,
        outgoing: HashMap::new(),
        incoming: HashMap::new(),
        seen: HashSet::new(),
        seen_order: VecDeque::new(),
        seq: 0,
        manual_disconnect: false,
        last_state: None,
    };
    manager.emit_status(ConnectionState::Connecting, "启动网络".to_string());
    for seed in &config.seeds {
        match seed.to_socket_addrs() {
            Ok(mut addrs) => {
                if let Some(addr) = addrs.next() {
                    manager.spawn_connect(addr, None);
                } else {
                    manager.emit_status(ConnectionState::Offline, format!("无法解析 seed: {seed}"));
                }
            }
            Err(error) => manager.emit_status(
                ConnectionState::Offline,
                format!("无法解析 seed {seed}: {error}"),
            ),
        }
    }
    tokio::spawn(manager.run());

    Ok((
        NetHandle {
            tx: command_tx,
            local_port,
        },
        event_rx,
    ))
}

impl Manager {
    async fn run(mut self) {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                accepted = self.listener.accept() => {
                    match accepted {
                        Ok((stream, addr)) => self.spawn_inbound(stream, addr),
                        Err(error) => self.emit_status(ConnectionState::Offline, format!("accept 失败: {error}")),
                    }
                }
                Some(command) = self.commands.recv() => {
                    if self.handle_command(command).await {
                        break;
                    }
                }
                Some(event) = self.peer_rx.recv() => {
                    self.handle_peer_event(event).await;
                }
                Some(event) = self.discovery_rx.recv() => {
                    self.handle_discovery(event);
                }
                _ = tick.tick() => {
                    self.tick().await;
                }
            }
        }
        self.shutdown_peers("shutdown").await;
    }

    async fn handle_command(&mut self, command: NetCommand) -> bool {
        match command {
            NetCommand::SendChat { msg_id, body } => {
                self.send_chat(msg_id, body).await;
                false
            }
            NetCommand::SendFile {
                node_id,
                transfer_id,
                path,
                name,
                size,
                folder,
            } => {
                self.send_file_offer(node_id, transfer_id, path, name, size, folder)
                    .await;
                false
            }
            NetCommand::SendFolder {
                node_id,
                transfer_id,
                path,
                root_name,
                archive_size,
                total_size,
                file_count,
                empty_dirs,
            } => {
                let folder = FolderMeta {
                    root_name: root_name.clone(),
                    total_size,
                    file_count,
                    empty_dirs,
                };
                self.send_folder_offer(node_id, transfer_id, path, root_name, archive_size, folder)
                    .await;
                false
            }
            NetCommand::AcceptFiles {
                node_id,
                transfer_ids,
            } => {
                self.accept_files(node_id, transfer_ids).await;
                false
            }
            NetCommand::SetName { name } => {
                self.local.name = name.clone();
                let update = WireMessage::NameUpdate(crate::net::protocol::NameUpdate {
                    node_id: self.local.node_id.clone(),
                    name,
                });
                for peer in self.peers.values() {
                    if peer.supports_name_update {
                        let _ = peer.tx.try_send(update.clone());
                    }
                }
                false
            }
            NetCommand::Reconnect => {
                self.manual_disconnect = false;
                self.config.reconnect = true;
                let mut targets: Vec<(String, SocketAddr)> = self
                    .known
                    .iter()
                    .filter_map(|(node_id, known)| {
                        if self.peers.contains_key(node_id) || known.addrs.is_empty() {
                            None
                        } else {
                            Some((node_id.clone(), known.addrs[0]))
                        }
                    })
                    .collect();
                for (node_id, addr) in targets.drain(..) {
                    self.spawn_connect(addr, Some(node_id));
                }
                self.emit_status(ConnectionState::Connecting, "重连".to_string());
                false
            }
            NetCommand::Disconnect => {
                self.manual_disconnect = true;
                self.config.reconnect = false;
                self.reconnects.clear();
                self.deferred.clear();
                self.shutdown_peers("manual disconnect").await;
                self.emit_status(ConnectionState::Offline, "已断开".to_string());
                false
            }
            NetCommand::Shutdown => true,
        }
    }

    async fn send_chat(&mut self, msg_id: String, body: String) {
        if body.len() > MAX_BODY_BYTES {
            let _ = self
                .events
                .send(NetEvent::SendFailed {
                    msg_id,
                    reason: "消息过长".to_string(),
                })
                .await;
            return;
        }
        if self.peers.is_empty() {
            let _ = self
                .events
                .send(NetEvent::SendFailed {
                    msg_id,
                    reason: "无在线节点".to_string(),
                })
                .await;
            return;
        }
        self.seq = self.seq.wrapping_add(1);
        let chat = Chat {
            msg_id: msg_id.clone(),
            room: ROOM.to_string(),
            from: self.local.node_id.clone(),
            name: self.local.name.clone(),
            seq: self.seq,
            sent_at: Local::now().to_rfc3339(),
            body,
        };
        if let Err(error) = validate_chat(&chat) {
            let _ = self
                .events
                .send(NetEvent::SendFailed {
                    msg_id,
                    reason: error.to_string(),
                })
                .await;
            return;
        }
        let mut failed = None;
        for peer in self.peers.values() {
            if let Err(error) = peer.tx.try_send(WireMessage::Chat(chat.clone())) {
                if failed.is_none() {
                    failed = Some(error.to_string());
                }
            }
        }
        if let Some(reason) = failed {
            let _ = self
                .events
                .send(NetEvent::SendFailed { msg_id, reason })
                .await;
        }
    }

    async fn send_file_offer(
        &mut self,
        node_id: String,
        transfer_id: String,
        path: PathBuf,
        name: String,
        size: u64,
        folder: Option<FolderMeta>,
    ) {
        let Some(peer) = self.peers.get(&node_id) else {
            self.file_command_error("对方不在线".to_string()).await;
            return;
        };
        if !peer.supports_files {
            self.file_command_error("对方版本不支持文件传递，请升级后重试".to_string())
                .await;
            return;
        }
        if folder.is_some() && !peer.supports_folder {
            self.file_command_error("对方版本不支持文件夹传递，请升级后重试".to_string())
                .await;
            return;
        }
        if self
            .outgoing
            .values()
            .any(|offer| offer.node_id == node_id && offer.name == name)
        {
            self.file_command_error(format!(
                "{name} 的请求已经发给对方了，请等对方接收完成后再发。"
            ))
            .await;
            return;
        }
        let offer = FileOffer {
            transfer_id: transfer_id.clone(),
            name: name.clone(),
            size,
            folder: folder.clone(),
        };
        if validate_file_offer(&offer).is_err() {
            self.file_command_error("文件名不合法".to_string()).await;
            return;
        }
        let Some(peer) = self.peers.get(&node_id) else {
            self.file_command_error("对方不在线".to_string()).await;
            return;
        };
        if peer.tx.try_send(WireMessage::FileOffer(offer)).is_err() {
            self.file_command_error("发送文件请求失败".to_string())
                .await;
            return;
        }
        self.outgoing.insert(
            transfer_id.clone(),
            OutgoingOffer {
                node_id: node_id.clone(),
                name: name.clone(),
                path,
                folder: folder.clone(),
                created: Instant::now(),
                transferring: false,
            },
        );
        let _ = self
            .events
            .send(NetEvent::FileOfferSent {
                node_id,
                transfer_id,
                file_name: name,
                folder,
            })
            .await;
    }

    async fn send_folder_offer(
        &mut self,
        node_id: String,
        transfer_id: String,
        path: PathBuf,
        root_name: String,
        archive_size: u64,
        folder: FolderMeta,
    ) {
        self.send_file_offer(
            node_id,
            transfer_id,
            path,
            root_name,
            archive_size,
            Some(folder),
        )
        .await;
    }

    async fn accept_files(&mut self, node_id: String, transfer_ids: Vec<String>) {
        let mut accepted = Vec::new();
        let mut required = 0u64;
        let mut folders = std::collections::HashMap::new();
        for id in &transfer_ids {
            let Some(offer) = self.incoming.get(id) else {
                continue;
            };
            if offer.node_id != node_id || offer.accepted {
                continue;
            }
            let size = offer
                .folder
                .as_ref()
                .map(|folder| folder.total_size)
                .unwrap_or(offer.size);
            required = required.saturating_add(size);
            folders.insert(id.clone(), offer.folder.is_some());
            accepted.push(id.clone());
        }
        if accepted.is_empty() {
            self.file_command_error("没有来自该用户的待接收文件。".to_string())
                .await;
            return;
        }
        let free = file::available_space(&self.files_dir).ok();
        if free.is_none_or(|free| free < file::required_for_transfer(required)) {
            for id in &accepted {
                if let Some(offer) = self.incoming.remove(id) {
                    if let Some(peer) = self.peers.get(&node_id) {
                        let _ = peer.tx.try_send(WireMessage::FileError(FileError {
                            transfer_id: id.clone(),
                            code: "no_space".to_string(),
                            message: "磁盘空间不足，传送失败".to_string(),
                        }));
                    }
                    let _ = self
                        .events
                        .send(NetEvent::FileFailed {
                            node_id: node_id.clone(),
                            transfer_id: id.clone(),
                            file_name: offer.name,
                            reason: "磁盘空间不足，传送失败".to_string(),
                            is_folder: offer.folder.is_some(),
                        })
                        .await;
                }
            }
            return;
        }
        let is_folder = accepted.iter().any(|id| {
            self.incoming
                .get(id)
                .is_some_and(|offer| offer.folder.is_some())
        });
        for id in &accepted {
            if let Some(offer) = self.incoming.get_mut(id) {
                offer.accepted = true;
            }
        }
        if let Some(peer) = self.peers.get(&node_id) {
            let _ = peer.tx.try_send(WireMessage::FileAccept(FileAccept {
                transfer_ids: accepted.clone(),
            }));
        }
        let _ = self
            .events
            .send(NetEvent::FileAccepted {
                node_id,
                transfer_ids: accepted,
                is_folder,
            })
            .await;
    }

    async fn file_command_error(&self, message: String) {
        let _ = self
            .events
            .send(NetEvent::FileCommandError { message })
            .await;
    }

    async fn handle_peer_event(&mut self, event: PeerEvent) {
        match event {
            PeerEvent::Connected {
                node_id,
                name,
                short_no,
                addr,
                listen_port,
                capabilities,
                stream,
            } => {
                self.connecting_nodes.remove(&node_id);
                self.connecting_addrs.remove(&addr);
                self.reconnects.remove(&node_id);
                self.deferred.remove(&node_id);
                let listen_addr = SocketAddr::new(
                    addr.ip(),
                    if listen_port == 0 {
                        addr.port()
                    } else {
                        listen_port
                    },
                );
                let supports_short_no = capabilities.iter().any(|item| item == "short_no");
                let peer_short_no = short_no;
                let mut local_short_changed = false;
                if supports_short_no
                    && short_no != 0
                    && short_no == self.local.short_no
                    && self.local.node_id > node_id
                {
                    let mut candidate = 1u32;
                    while candidate == short_no
                        || self.peers.values().any(|peer| peer.short_no == candidate)
                    {
                        candidate = candidate.saturating_add(1);
                    }
                    self.local.short_no = candidate;
                    if let Err(error) =
                        crate::config::save_identity(&self.identity_path, &self.local)
                    {
                        tracing::warn!(%error, "保存 short_no 失败");
                    }
                    local_short_changed = true;
                }
                if self.peers.contains_key(&node_id) {
                    let mut stream = stream;
                    let _ = write_frame(
                        &mut stream,
                        &WireMessage::Bye(Bye {
                            reason: "duplicate".to_string(),
                        }),
                    )
                    .await;
                    return;
                }
                if self.peers.len() >= MAX_PEERS {
                    let mut stream = stream;
                    let _ = write_frame(
                        &mut stream,
                        &WireMessage::Bye(Bye {
                            reason: "too many peers".to_string(),
                        }),
                    )
                    .await;
                    return;
                }
                let (peer_tx, peer_rx) = mpsc::channel(CHANNEL_CAPACITY);
                let (read_half, write_half) = stream.into_split();
                let (close_tx, close_rx) = watch::channel(false);
                let reader_events = self.peer_tx.clone();
                let reader_node = node_id.clone();
                let reader_close = close_rx.clone();
                tokio::spawn(async move {
                    reader_loop(read_half, reader_node, reader_events, reader_close).await;
                });
                let writer_events = self.peer_tx.clone();
                let writer_node = node_id.clone();
                tokio::spawn(async move {
                    if let Err(error) = writer_loop(write_half, peer_rx, close_rx).await {
                        let _ = writer_events
                            .send(PeerEvent::Closed {
                                node_id: writer_node,
                                reason: error.to_string(),
                            })
                            .await;
                    }
                });
                self.peers.insert(
                    node_id.clone(),
                    PeerEntry {
                        name: name.clone(),
                        addr: listen_addr,
                        tx: peer_tx,
                        close: close_tx,
                        supports_files: capabilities.iter().any(|item| item == "file_transfer"),
                        supports_folder: capabilities.iter().any(|item| item == "folder_transfer"),
                        supports_short_no,
                        supports_name_update: capabilities.iter().any(|item| item == "name_update"),
                        short_no: peer_short_no,
                        last_seen: Instant::now(),
                        last_ping: Instant::now(),
                    },
                );
                self.known.entry(node_id.clone()).or_insert(KnownPeer {
                    addrs: vec![listen_addr],
                });
                let _ = self
                    .events
                    .send(NetEvent::PeerUp {
                        node_id,
                        name,
                        short_no: peer_short_no,
                        addr: listen_addr,
                    })
                    .await;
                if local_short_changed {
                    let _ = self
                        .events
                        .send(NetEvent::LocalShortNoChanged {
                            short_no: self.local.short_no,
                        })
                        .await;
                    let update = WireMessage::ShortNoUpdate(crate::net::protocol::ShortNoUpdate {
                        node_id: self.local.node_id.clone(),
                        short_no: self.local.short_no,
                    });
                    for peer in self.peers.values() {
                        if peer.supports_short_no {
                            let _ = peer.tx.try_send(update.clone());
                        }
                    }
                }
                self.emit_status(ConnectionState::Online, "已连接".to_string());
            }
            PeerEvent::Message { node_id, message } => {
                self.handle_message(node_id, message).await;
            }
            PeerEvent::Closed { node_id, reason } => {
                self.remove_peer(&node_id, &reason).await;
            }
            PeerEvent::DataHello {
                addr,
                hello,
                stream,
            } => {
                self.handle_data_hello(addr, hello, stream).await;
            }
            PeerEvent::DataReceived { node_id, received } => {
                for file in received {
                    if let Some(peer) = self.peers.get(&node_id) {
                        let _ = peer.tx.try_send(WireMessage::FileComplete(FileComplete {
                            transfer_id: file.transfer_id.clone(),
                        }));
                    }
                    self.incoming.remove(&file.transfer_id);
                    let _ = self
                        .events
                        .send(NetEvent::FileReceived {
                            node_id: node_id.clone(),
                            transfer_id: file.transfer_id,
                            file_name: file.name,
                            path: file.path,
                            is_folder: file.is_folder,
                        })
                        .await;
                }
            }
            PeerEvent::DataSent {
                node_id,
                transfer_ids,
            } => {
                let _ = (node_id, transfer_ids);
            }
            PeerEvent::DataFailed {
                node_id,
                transfer_ids,
                reason,
            } => {
                for transfer_id in transfer_ids {
                    let removed = self.take_file_offer(&transfer_id);
                    let name = removed
                        .as_ref()
                        .map(|(name, _, _)| name.clone())
                        .unwrap_or_else(|| "文件".to_string());
                    let is_folder = removed.as_ref().is_some_and(|(_, folder, _)| *folder);
                    if let Some((_, true, Some(path))) = removed {
                        let _ = std::fs::remove_file(path);
                    }
                    if let Some(peer) = self.peers.get(&node_id) {
                        let _ = peer.tx.try_send(WireMessage::FileError(FileError {
                            transfer_id: transfer_id.clone(),
                            code: "transfer_failed".to_string(),
                            message: reason.clone(),
                        }));
                    }
                    let _ = self
                        .events
                        .send(NetEvent::FileFailed {
                            node_id: node_id.clone(),
                            transfer_id,
                            file_name: name,
                            reason: reason.clone(),
                            is_folder,
                        })
                        .await;
                }
            }
            PeerEvent::HandshakeFailed {
                addr,
                node_id,
                reason,
            } => {
                self.connecting_addrs.remove(&addr);
                if let Some(node_id) = node_id {
                    self.connecting_nodes.remove(&node_id);
                    if self.config.reconnect && !self.reconnects.contains_key(&node_id) {
                        self.reconnects.insert(
                            node_id,
                            ReconnectState {
                                addr,
                                attempts: 0,
                                next_at: Instant::now() + Duration::from_secs(1),
                            },
                        );
                    }
                }
                let state = if self.peers.is_empty() {
                    ConnectionState::Offline
                } else {
                    ConnectionState::Online
                };
                self.emit_status(state, format!("连接 {addr} 失败: {reason}"));
            }
        }
    }

    async fn handle_data_hello(
        &mut self,
        addr: SocketAddr,
        hello: FileDataHello,
        stream: TcpStream,
    ) {
        let Some(peer) = self.peers.get(&hello.node_id) else {
            return;
        };
        if !peer.supports_files {
            return;
        }
        let expected: Vec<IncomingFile> = hello
            .transfer_ids
            .iter()
            .filter_map(|id| {
                self.incoming.get(id).and_then(|offer| {
                    (offer.node_id == hello.node_id && offer.accepted).then(|| IncomingFile {
                        transfer_id: id.clone(),
                        name: offer.name.clone(),
                        size: offer.size,
                        folder: offer.folder.clone(),
                    })
                })
            })
            .collect();
        if expected.len() != hello.transfer_ids.len() {
            let mut stream = stream;
            let _ = write_frame(
                &mut stream,
                &WireMessage::FileError(FileError {
                    transfer_id: String::new(),
                    code: "not_accepted".to_string(),
                    message: "数据连接包含未接受的文件".to_string(),
                }),
            )
            .await;
            return;
        }
        let events = self.peer_tx.clone();
        let dir = self.files_dir.clone();
        let node_id = hello.node_id.clone();
        let ids = hello.transfer_ids.clone();
        let _ = addr;
        let peer_node = node_id.clone();
        tokio::spawn(async move {
            match file::receive_files(stream, hello, &expected, &dir).await {
                Ok(received) => {
                    let _ = events
                        .send(PeerEvent::DataReceived {
                            node_id: peer_node,
                            received,
                        })
                        .await;
                }
                Err(error) => {
                    let _ = events
                        .send(PeerEvent::DataFailed {
                            node_id: peer_node,
                            transfer_ids: ids,
                            reason: error.to_string(),
                        })
                        .await;
                }
            }
        });
        let _ = node_id;
    }

    fn take_file_offer(&mut self, transfer_id: &str) -> Option<(String, bool, Option<PathBuf>)> {
        if let Some(offer) = self.outgoing.remove(transfer_id) {
            Some((offer.name, offer.folder.is_some(), Some(offer.path)))
        } else {
            self.incoming
                .remove(transfer_id)
                .map(|offer| (offer.name, offer.folder.is_some(), None))
        }
    }

    async fn handle_message(&mut self, node_id: String, message: WireMessage) {
        let Some(peer) = self.peers.get_mut(&node_id) else {
            return;
        };
        peer.last_seen = Instant::now();
        match message {
            WireMessage::Chat(chat) => {
                if chat.from != node_id || validate_chat(&chat).is_err() {
                    return;
                }
                if !self.mark_seen(&chat.msg_id) {
                    return;
                }
                let _ = self.events.send(NetEvent::ChatReceived(chat)).await;
            }
            WireMessage::Ping(Ping { nonce }) => {
                let _ = peer.tx.try_send(WireMessage::Pong(Pong { nonce }));
            }
            WireMessage::Pong(_) => {}
            WireMessage::Bye(_) => {
                self.remove_peer(&node_id, "peer bye").await;
            }
            WireMessage::Error(error) => {
                self.emit_status(
                    ConnectionState::Offline,
                    format!("协议错误: {} {}", error.code, error.message),
                );
            }
            WireMessage::FileOffer(offer) => {
                if validate_file_offer(&offer).is_err()
                    || self.incoming.len() >= file::MAX_PENDING_TOTAL
                {
                    let _ = peer.tx.try_send(WireMessage::FileError(FileError {
                        transfer_id: offer.transfer_id.clone(),
                        code: "invalid_offer".to_string(),
                        message: "文件请求不合法或超出限制".to_string(),
                    }));
                    return;
                }
                let duplicate = self
                    .incoming
                    .values()
                    .any(|item| item.node_id == node_id && item.name == offer.name);
                if duplicate {
                    let _ = peer.tx.try_send(WireMessage::FileError(FileError {
                        transfer_id: offer.transfer_id.clone(),
                        code: "duplicate".to_string(),
                        message: "文件请求已经存在".to_string(),
                    }));
                    return;
                }
                if self
                    .incoming
                    .values()
                    .filter(|item| item.node_id == node_id)
                    .count()
                    >= file::MAX_PENDING_PER_PEER
                {
                    let _ = peer.tx.try_send(WireMessage::FileError(FileError {
                        transfer_id: offer.transfer_id.clone(),
                        code: "too_many".to_string(),
                        message: "待接收文件太多".to_string(),
                    }));
                    return;
                }
                self.incoming.insert(
                    offer.transfer_id.clone(),
                    IncomingOffer {
                        node_id: node_id.clone(),
                        name: offer.name.clone(),
                        size: offer.size,
                        folder: offer.folder.clone(),
                        created: Instant::now(),
                        accepted: false,
                    },
                );
                let _ = self
                    .events
                    .send(NetEvent::FileOfferReceived {
                        node_id,
                        peer_name: peer.name.clone(),
                        addr: peer.addr,
                        transfer_id: offer.transfer_id,
                        file_name: offer.name,
                        size: offer.size,
                        folder: offer.folder,
                    })
                    .await;
            }
            WireMessage::FileAccept(accept) => {
                let mut files = Vec::new();
                for transfer_id in &accept.transfer_ids {
                    let Some(offer) = self.outgoing.get_mut(transfer_id) else {
                        continue;
                    };
                    if offer.node_id != node_id || offer.transferring {
                        continue;
                    }
                    offer.transferring = true;
                    files.push(OutgoingFile {
                        transfer_id: transfer_id.clone(),
                        path: offer.path.clone(),
                    });
                }
                if files.is_empty() {
                    return;
                }
                let ids: Vec<String> = files.iter().map(|file| file.transfer_id.clone()).collect();
                let addr = peer.addr;
                let local = self.local.node_id.clone();
                let events = self.peer_tx.clone();
                let peer_node = node_id.clone();
                tokio::spawn(async move {
                    match file::send_files(addr, &local, &files).await {
                        Ok(()) => {
                            let _ = events
                                .send(PeerEvent::DataSent {
                                    node_id: peer_node.clone(),
                                    transfer_ids: ids,
                                })
                                .await;
                        }
                        Err(error) => {
                            let _ = events
                                .send(PeerEvent::DataFailed {
                                    node_id: peer_node.clone(),
                                    transfer_ids: ids,
                                    reason: error.to_string(),
                                })
                                .await;
                        }
                    }
                });
                let is_folder = accept.transfer_ids.iter().any(|id| {
                    self.outgoing
                        .get(id)
                        .is_some_and(|offer| offer.folder.is_some())
                });
                let _ = self
                    .events
                    .send(NetEvent::FileAccepted {
                        node_id,
                        transfer_ids: accept.transfer_ids,
                        is_folder,
                    })
                    .await;
            }
            WireMessage::FileComplete(complete) => {
                let removed = self.take_file_offer(&complete.transfer_id);
                let name = removed
                    .as_ref()
                    .map(|(name, _, _)| name.clone())
                    .unwrap_or_else(|| "文件".to_string());
                let is_folder = removed.as_ref().is_some_and(|(_, folder, _)| *folder);
                if let Some((_, true, Some(path))) = removed {
                    let _ = std::fs::remove_file(path);
                }
                let _ = self
                    .events
                    .send(NetEvent::FileSent {
                        node_id,
                        transfer_id: complete.transfer_id,
                        file_name: name,
                        is_folder,
                    })
                    .await;
            }
            WireMessage::FileCancel(cancel) => {
                let removed = self.take_file_offer(&cancel.transfer_id);
                let name = removed
                    .as_ref()
                    .map(|(name, _, _)| name.clone())
                    .unwrap_or_else(|| "文件".to_string());
                let is_folder = removed.as_ref().is_some_and(|(_, folder, _)| *folder);
                if let Some((_, true, Some(path))) = removed {
                    let _ = std::fs::remove_file(path);
                }
                let _ = self
                    .events
                    .send(NetEvent::FileCancelled {
                        node_id,
                        transfer_id: cancel.transfer_id,
                        file_name: name,
                        reason: cancel.reason,
                        is_folder,
                    })
                    .await;
            }
            WireMessage::NameUpdate(update) => {
                if update.node_id == node_id && !update.name.trim().is_empty() {
                    if let Some(peer) = self.peers.get_mut(&node_id) {
                        peer.name = update.name.clone();
                        let _ = self
                            .events
                            .send(NetEvent::PeerNameChanged {
                                node_id: node_id.clone(),
                                name: update.name,
                            })
                            .await;
                    }
                }
            }
            WireMessage::ShortNoUpdate(update) => {
                if update.node_id == node_id && update.short_no != 0 {
                    if let Some(peer) = self.peers.get_mut(&node_id) {
                        peer.short_no = update.short_no;
                        let _ = self
                            .events
                            .send(NetEvent::PeerShortNoChanged {
                                node_id: node_id.clone(),
                                short_no: update.short_no,
                            })
                            .await;
                    }
                }
            }
            WireMessage::FileError(error) => {
                let removed = self.take_file_offer(&error.transfer_id);
                let name = removed
                    .as_ref()
                    .map(|(name, _, _)| name.clone())
                    .unwrap_or_else(|| "文件".to_string());
                let is_folder = removed.as_ref().is_some_and(|(_, folder, _)| *folder);
                if let Some((_, true, Some(path))) = removed {
                    let _ = std::fs::remove_file(path);
                }
                let _ = self
                    .events
                    .send(NetEvent::FileFailed {
                        node_id,
                        transfer_id: error.transfer_id,
                        file_name: name,
                        reason: error.message,
                        is_folder,
                    })
                    .await;
            }
            _ => {}
        }
    }

    fn mark_seen(&mut self, msg_id: &str) -> bool {
        if self.seen.contains(msg_id) {
            return false;
        }
        self.seen.insert(msg_id.to_string());
        self.seen_order.push_back(msg_id.to_string());
        while self.seen_order.len() > MAX_SEEN {
            if let Some(old) = self.seen_order.pop_front() {
                self.seen.remove(&old);
            }
        }
        true
    }

    async fn remove_peer(&mut self, node_id: &str, reason: &str) {
        let Some(peer) = self.peers.remove(node_id) else {
            return;
        };
        let _ = peer.tx.try_send(WireMessage::Bye(Bye {
            reason: reason.to_string(),
        }));
        let _ = peer.close.send(true);
        for offer in self.outgoing.values() {
            if offer.node_id == node_id && offer.folder.is_some() {
                let _ = std::fs::remove_file(&offer.path);
            }
        }
        self.outgoing.retain(|_, offer| offer.node_id != node_id);
        self.incoming.retain(|_, offer| offer.node_id != node_id);
        let _ = self
            .events
            .send(NetEvent::PeerDown {
                node_id: node_id.to_string(),
                reason: reason.to_string(),
            })
            .await;
        if self.config.reconnect && !self.connecting_nodes.contains(node_id) {
            let addr = self
                .known
                .get(node_id)
                .and_then(|known| known.addrs.first().copied())
                .unwrap_or(peer.addr);
            self.reconnects.insert(
                node_id.to_string(),
                ReconnectState {
                    addr,
                    attempts: 0,
                    next_at: Instant::now() + Duration::from_secs(1),
                },
            );
        }
        self.emit_status(
            if self.peers.is_empty() {
                ConnectionState::Reconnecting
            } else {
                ConnectionState::Online
            },
            format!("{node_id} 断开: {reason}"),
        );
    }

    fn handle_discovery(&mut self, event: DiscoveryEvent) {
        if self.manual_disconnect {
            return;
        }
        match event {
            DiscoveryEvent::Resolved {
                node_id,
                name,
                addrs,
                port,
            } => {
                let socket_addrs = discovery::socket_addrs(&addrs, port);
                if socket_addrs.is_empty() {
                    return;
                }
                let _ = name;
                self.known.insert(
                    node_id.clone(),
                    KnownPeer {
                        addrs: socket_addrs.clone(),
                    },
                );
                if self.peers.contains_key(&node_id) || self.connecting_nodes.contains(&node_id) {
                    return;
                }
                if self.local.node_id < node_id {
                    self.spawn_connect(socket_addrs[0], Some(node_id));
                } else {
                    self.deferred
                        .entry(node_id)
                        .or_insert_with(|| Instant::now() + Duration::from_secs(2));
                }
            }
            DiscoveryEvent::Removed { node_id } => {
                self.deferred.remove(&node_id);
                if !self.peers.contains_key(&node_id) {
                    self.reconnects.remove(&node_id);
                }
            }
            DiscoveryEvent::Error(error) => {
                self.emit_status(ConnectionState::Offline, error);
            }
        }
    }

    async fn tick(&mut self) {
        let now = Instant::now();
        let timed_out: Vec<String> = self
            .peers
            .iter()
            .filter(|(_, peer)| now.duration_since(peer.last_seen) > IDLE_TIMEOUT)
            .map(|(node_id, _)| node_id.clone())
            .collect();
        for node_id in timed_out {
            self.remove_peer(&node_id, "idle timeout").await;
        }
        let ping_targets: Vec<String> = self
            .peers
            .iter()
            .filter(|(_, peer)| now.duration_since(peer.last_ping) >= PING_INTERVAL)
            .map(|(node_id, _)| node_id.clone())
            .collect();
        for node_id in ping_targets {
            if let Some(peer) = self.peers.get_mut(&node_id) {
                peer.last_ping = now;
                let _ = peer.tx.try_send(WireMessage::Ping(Ping {
                    nonce: uuid::Uuid::new_v4().to_string(),
                }));
            }
        }
        let expired_outgoing: Vec<String> = self
            .outgoing
            .iter()
            .filter(|(_, offer)| {
                !offer.transferring && now.duration_since(offer.created) >= FILE_REQUEST_TIMEOUT
            })
            .map(|(id, _)| id.clone())
            .collect();
        for transfer_id in expired_outgoing {
            if let Some(offer) = self.outgoing.remove(&transfer_id) {
                if offer.folder.is_some() {
                    let _ = std::fs::remove_file(&offer.path);
                }
                if let Some(peer) = self.peers.get(&offer.node_id) {
                    let _ = peer.tx.try_send(WireMessage::FileCancel(FileCancel {
                        transfer_id: transfer_id.clone(),
                        reason: "timeout".to_string(),
                    }));
                }
                let _ = self
                    .events
                    .send(NetEvent::FileTimedOut {
                        node_id: offer.node_id,
                        transfer_id,
                        file_name: offer.name,
                        outgoing: true,
                        is_folder: offer.folder.is_some(),
                    })
                    .await;
            }
        }
        let expired_incoming: Vec<String> = self
            .incoming
            .iter()
            .filter(|(_, offer)| {
                !offer.accepted && now.duration_since(offer.created) >= FILE_REQUEST_TIMEOUT
            })
            .map(|(id, _)| id.clone())
            .collect();
        for transfer_id in expired_incoming {
            if let Some(offer) = self.incoming.remove(&transfer_id) {
                if let Some(peer) = self.peers.get(&offer.node_id) {
                    let _ = peer.tx.try_send(WireMessage::FileCancel(FileCancel {
                        transfer_id: transfer_id.clone(),
                        reason: "timeout".to_string(),
                    }));
                }
                let _ = self
                    .events
                    .send(NetEvent::FileTimedOut {
                        node_id: offer.node_id,
                        transfer_id,
                        file_name: offer.name,
                        outgoing: false,
                        is_folder: offer.folder.is_some(),
                    })
                    .await;
            }
        }
        if self.manual_disconnect {
            return;
        }
        let deferred: Vec<(String, SocketAddr)> = self
            .deferred
            .iter()
            .filter(|(node_id, next_at)| {
                now >= **next_at
                    && !self.peers.contains_key(*node_id)
                    && !self.connecting_nodes.contains(*node_id)
            })
            .filter_map(|(node_id, _)| {
                self.known
                    .get(node_id)
                    .and_then(|known| known.addrs.first().copied())
                    .map(|addr| (node_id.clone(), addr))
            })
            .collect();
        for (node_id, addr) in deferred {
            self.deferred.remove(&node_id);
            self.spawn_connect(addr, Some(node_id));
        }
        let due: Vec<(String, SocketAddr)> = self
            .reconnects
            .iter()
            .filter(|(_, state)| now >= state.next_at)
            .map(|(node_id, state)| (node_id.clone(), state.addr))
            .collect();
        for (node_id, addr) in due {
            if let Some(state) = self.reconnects.get_mut(&node_id) {
                state.attempts = state.attempts.saturating_add(1);
                state.next_at = now + backoff(state.attempts);
            }
            self.spawn_connect(addr, Some(node_id));
        }
    }

    fn spawn_connect(&mut self, addr: SocketAddr, node_id: Option<String>) {
        if self.connecting_addrs.contains(&addr) {
            return;
        }
        if let Some(node_id) = &node_id {
            if self.peers.contains_key(node_id) || self.connecting_nodes.contains(node_id) {
                return;
            }
            self.connecting_nodes.insert(node_id.clone());
        }
        self.connecting_addrs.insert(addr);
        let local = self.local.clone();
        let listen_port = self.listen_port;
        let events = self.peer_tx.clone();
        let node_hint = node_id.clone();
        tokio::spawn(async move {
            let result = timeout(CONNECT_TIMEOUT, TcpStream::connect(addr))
                .await
                .map_err(|_| anyhow::anyhow!("connect timeout"))
                .and_then(|result| result.map_err(anyhow::Error::from));
            match result {
                Ok(stream) => {
                    let _ = stream.set_nodelay(true);
                    match handshake_outbound(stream, &local, listen_port).await {
                        Ok((stream, hello)) => {
                            let _ = events
                                .send(PeerEvent::Connected {
                                    node_id: hello.node_id,
                                    name: hello.name,
                                    short_no: hello.short_no,
                                    addr,
                                    listen_port: hello.listen_port,
                                    capabilities: hello.capabilities.clone(),
                                    stream,
                                })
                                .await;
                        }
                        Err(error) => {
                            let _ = events
                                .send(PeerEvent::HandshakeFailed {
                                    addr,
                                    node_id: node_hint,
                                    reason: error.to_string(),
                                })
                                .await;
                        }
                    }
                }
                Err(error) => {
                    let _ = events
                        .send(PeerEvent::HandshakeFailed {
                            addr,
                            node_id: node_hint,
                            reason: error.to_string(),
                        })
                        .await;
                }
            }
        });
        self.emit_status(ConnectionState::Connecting, format!("连接 {addr}"));
    }

    fn spawn_inbound(&mut self, stream: TcpStream, addr: SocketAddr) {
        if self.manual_disconnect {
            return;
        }
        let local = self.local.clone();
        let listen_port = self.listen_port;
        let events = self.peer_tx.clone();
        tokio::spawn(async move {
            let _ = stream.set_nodelay(true);
            let mut stream = stream;
            let first = match timeout(HANDSHAKE_TIMEOUT, read_frame(&mut stream)).await {
                Ok(Ok(message)) => message,
                Ok(Err(error)) => {
                    let _ = events
                        .send(PeerEvent::HandshakeFailed {
                            addr,
                            node_id: None,
                            reason: error.to_string(),
                        })
                        .await;
                    return;
                }
                Err(_) => {
                    let _ = events
                        .send(PeerEvent::HandshakeFailed {
                            addr,
                            node_id: None,
                            reason: "handshake timeout".to_string(),
                        })
                        .await;
                    return;
                }
            };
            match first {
                WireMessage::Hello(hello) => {
                    match handshake_inbound_with_hello(stream, hello, &local, listen_port).await {
                        Ok((stream, hello)) => {
                            let _ = events
                                .send(PeerEvent::Connected {
                                    node_id: hello.node_id,
                                    name: hello.name,
                                    short_no: hello.short_no,
                                    addr,
                                    listen_port: hello.listen_port,
                                    capabilities: hello.capabilities.clone(),
                                    stream,
                                })
                                .await;
                        }
                        Err(error) => {
                            let _ = events
                                .send(PeerEvent::HandshakeFailed {
                                    addr,
                                    node_id: None,
                                    reason: error.to_string(),
                                })
                                .await;
                        }
                    }
                }
                WireMessage::FileDataHello(hello) => {
                    let _ = events
                        .send(PeerEvent::DataHello {
                            addr,
                            hello,
                            stream,
                        })
                        .await;
                }
                _ => {
                    let _ = events
                        .send(PeerEvent::HandshakeFailed {
                            addr,
                            node_id: None,
                            reason: "首帧既不是 Hello 也不是 FileDataHello".to_string(),
                        })
                        .await;
                }
            }
        });
    }

    async fn shutdown_peers(&mut self, reason: &str) {
        for peer in self.peers.values() {
            let _ = peer.tx.try_send(WireMessage::Bye(Bye {
                reason: reason.to_string(),
            }));
            let _ = peer.close.send(true);
        }
        self.peers.clear();
        self.emit_status(ConnectionState::Offline, reason.to_string());
    }

    fn emit_status(&mut self, state: ConnectionState, detail: String) {
        if self.last_state == Some(state) {
            return;
        }
        self.last_state = Some(state);
        let _ = self.events.try_send(NetEvent::Status { state, detail });
    }
}

fn backoff(attempts: u32) -> Duration {
    let seconds = 1u64 << attempts.min(5);
    Duration::from_secs(seconds.min(30))
}

fn local_hello(local: &Identity, listen_port: u16) -> Hello {
    Hello {
        magic: MAGIC.to_string(),
        protocol: PROTOCOL_VERSION,
        node_id: local.node_id.clone(),
        name: local.name.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        listen_port,
        short_no: local.short_no,
        capabilities: vec![
            "chat".to_string(),
            "file_transfer".to_string(),
            "short_no".to_string(),
            "name_update".to_string(),
            "folder_transfer".to_string(),
        ],
        nonce: uuid::Uuid::new_v4().to_string(),
    }
}

async fn handshake_outbound(
    mut stream: TcpStream,
    local: &Identity,
    listen_port: u16,
) -> Result<(TcpStream, Hello)> {
    let hello = local_hello(local, listen_port);
    timeout(HANDSHAKE_TIMEOUT, async {
        write_frame(&mut stream, &WireMessage::Hello(hello)).await?;
        let received = read_frame(&mut stream).await?;
        let WireMessage::Hello(peer) = received else {
            anyhow::bail!("握手首帧不是 Hello");
        };
        validate_hello(&peer, &local.node_id)?;
        write_frame(
            &mut stream,
            &WireMessage::Ready(Ready {
                node_id: local.node_id.clone(),
            }),
        )
        .await?;
        match read_frame(&mut stream).await? {
            WireMessage::Ready(_) => Ok((stream, peer)),
            other => anyhow::bail!("握手期望 Ready，收到 {other:?}"),
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("handshake timeout"))?
}

async fn handshake_inbound_with_hello(
    mut stream: TcpStream,
    peer: Hello,
    local: &Identity,
    listen_port: u16,
) -> Result<(TcpStream, Hello)> {
    timeout(HANDSHAKE_TIMEOUT, async {
        validate_hello(&peer, &local.node_id)?;
        write_frame(
            &mut stream,
            &WireMessage::Hello(local_hello(local, listen_port)),
        )
        .await?;
        write_frame(
            &mut stream,
            &WireMessage::Ready(Ready {
                node_id: local.node_id.clone(),
            }),
        )
        .await?;
        match read_frame(&mut stream).await? {
            WireMessage::Ready(_) => Ok((stream, peer)),
            other => anyhow::bail!("握手期望 Ready，收到 {other:?}"),
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("handshake timeout"))?
}

async fn reader_loop<R>(
    mut reader: R,
    node_id: String,
    events: mpsc::Sender<PeerEvent>,
    mut close: watch::Receiver<bool>,
) where
    R: AsyncRead + Unpin,
{
    loop {
        let message = tokio::select! {
            changed = close.changed() => {
                if changed.is_err() || *close.borrow() {
                    break;
                }
                continue;
            }
            result = read_frame(&mut reader) => result,
        };
        match message {
            Ok(message) => {
                if events
                    .send(PeerEvent::Message {
                        node_id: node_id.clone(),
                        message,
                    })
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Err(error) => {
                let _ = events
                    .send(PeerEvent::Closed {
                        node_id,
                        reason: error.to_string(),
                    })
                    .await;
                break;
            }
        }
    }
}

async fn writer_loop<W>(
    mut writer: W,
    mut rx: mpsc::Receiver<WireMessage>,
    mut close: watch::Receiver<bool>,
) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    loop {
        tokio::select! {
            changed = close.changed() => {
                if changed.is_err() || *close.borrow() {
                    break;
                }
            }
            message = rx.recv() => {
                let Some(message) = message else { break };
                write_frame(&mut writer, &message).await?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_is_bounded() {
        assert_eq!(backoff(0), Duration::from_secs(1));
        assert_eq!(backoff(5), Duration::from_secs(30));
        assert_eq!(backoff(50), Duration::from_secs(30));
    }

    fn config(port: u16, seeds: Vec<String>) -> NetworkConfig {
        NetworkConfig {
            enabled: true,
            port,
            mdns: false,
            reconnect: false,
            seeds,
        }
    }

    async fn wait_peer_up(rx: &mut mpsc::Receiver<NetEvent>) -> bool {
        // Windows CI can take longer than 500ms to establish the loopback
        // connection; keep waiting for the whole 10 second budget.
        for _ in 0..20 {
            match timeout(Duration::from_millis(500), rx.recv()).await {
                Ok(Some(NetEvent::PeerUp { .. })) => return true,
                Ok(Some(_)) => continue,
                Ok(None) => return false,
                Err(_) => continue,
            }
        }
        false
    }

    async fn wait_chat(rx: &mut mpsc::Receiver<NetEvent>) -> Option<Chat> {
        for _ in 0..20 {
            match timeout(Duration::from_millis(500), rx.recv()).await {
                Ok(Some(NetEvent::ChatReceived(chat))) => return Some(chat),
                Ok(Some(_)) => continue,
                Ok(None) => return None,
                Err(_) => continue,
            }
        }
        None
    }

    #[tokio::test]
    async fn name_update_reaches_peer() {
        let alice = Identity {
            node_id: "alice-node".into(),
            name: "alice".into(),
            short_no: 1,
        };
        let bob = Identity {
            node_id: "bob-node".into(),
            name: "bob".into(),
            short_no: 2,
        };
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        let (alice_handle, mut alice_rx) = start(
            config(0, Vec::new()),
            alice,
            dir_a.path().join("identity.json"),
            dir_a.path().join("files"),
        )
        .await
        .unwrap();
        let alice_port = alice_handle.local_port();
        let (bob_handle, mut bob_rx) = start(
            config(0, vec![format!("127.0.0.1:{alice_port}")]),
            bob,
            dir_b.path().join("identity.json"),
            dir_b.path().join("files"),
        )
        .await
        .unwrap();
        assert!(wait_peer_up(&mut alice_rx).await);
        assert!(wait_peer_up(&mut bob_rx).await);

        alice_handle
            .send(NetCommand::SetName {
                name: "alice2".into(),
            })
            .await;
        let mut updated = false;
        for _ in 0..20 {
            if let Ok(Some(NetEvent::PeerNameChanged { name, .. })) =
                timeout(Duration::from_millis(500), bob_rx.recv()).await
            {
                if name == "alice2" {
                    updated = true;
                    break;
                }
            }
        }
        assert!(updated, "bob did not see alice2");
        let _ = alice_handle.send(NetCommand::Shutdown).await;
        let _ = bob_handle.send(NetCommand::Shutdown).await;
    }

    #[tokio::test]
    async fn two_managers_transfer_a_file_over_localhost() {
        let alice = Identity {
            node_id: "alice-node".into(),
            name: "alice".into(),
            short_no: 1,
        };
        let bob = Identity {
            node_id: "bob-node".into(),
            name: "bob".into(),
            short_no: 1,
        };
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        let (alice_handle, mut alice_rx) = start(
            config(0, Vec::new()),
            alice,
            dir_a.path().join("identity.json"),
            dir_a.path().join("files"),
        )
        .await
        .unwrap();
        let alice_port = alice_handle.local_port();
        let (bob_handle, mut bob_rx) = start(
            config(0, vec![format!("127.0.0.1:{alice_port}")]),
            bob,
            dir_b.path().join("identity.json"),
            dir_b.path().join("files"),
        )
        .await
        .unwrap();
        assert!(wait_peer_up(&mut alice_rx).await);
        assert!(wait_peer_up(&mut bob_rx).await);

        std::fs::write(dir_b.path().join("files/hello.txt"), b"existing").unwrap();
        let source = dir_a.path().join("hello.txt");
        std::fs::write(&source, b"hello file transfer").unwrap();
        let size = std::fs::metadata(&source).unwrap().len();
        alice_handle
            .send(NetCommand::SendFile {
                node_id: "bob-node".into(),
                transfer_id: "transfer-1".into(),
                path: source,
                name: "hello.txt".into(),
                size,
                folder: None,
            })
            .await;

        let mut offer = None;
        for _ in 0..20 {
            if let Ok(Some(NetEvent::FileOfferReceived {
                transfer_id,
                file_name,
                size,
                ..
            })) = timeout(Duration::from_millis(500), bob_rx.recv()).await
            {
                offer = Some((transfer_id, file_name, size));
                break;
            }
        }
        let (transfer_id, file_name, offered_size) = offer.expect("no file offer");
        assert_eq!(file_name, "hello.txt");
        assert_eq!(offered_size, size);

        bob_handle
            .send(NetCommand::AcceptFiles {
                node_id: "alice-node".into(),
                transfer_ids: vec![transfer_id.clone()],
            })
            .await;

        let mut received_path = None;
        let mut sent = false;
        for _ in 0..40 {
            match timeout(Duration::from_millis(250), bob_rx.recv()).await {
                Ok(Some(NetEvent::FileReceived {
                    transfer_id: id,
                    path,
                    ..
                })) if id == transfer_id => {
                    received_path = Some(path);
                }
                _ => {}
            }
            match timeout(Duration::from_millis(50), alice_rx.recv()).await {
                Ok(Some(NetEvent::FileSent {
                    transfer_id: id, ..
                })) if id == transfer_id => {
                    sent = true;
                }
                _ => {}
            }
            if received_path.is_some() && sent {
                break;
            }
        }
        let path = received_path.expect("receiver did not finish");
        assert!(sent, "sender did not see completion");
        assert_eq!(path.file_name().unwrap(), "hello[0].txt");
        assert_eq!(std::fs::read(path).unwrap(), b"hello file transfer");

        let _ = alice_handle.send(NetCommand::Shutdown).await;
        let _ = bob_handle.send(NetCommand::Shutdown).await;
    }

    #[tokio::test]
    async fn duplicate_file_offer_is_rejected_while_pending() {
        let alice = Identity {
            node_id: "alice-node".into(),
            name: "alice".into(),
            short_no: 1,
        };
        let bob = Identity {
            node_id: "bob-node".into(),
            name: "bob".into(),
            short_no: 1,
        };
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        let (alice_handle, mut alice_rx) = start(
            config(0, Vec::new()),
            alice,
            dir_a.path().join("identity.json"),
            dir_a.path().join("files"),
        )
        .await
        .unwrap();
        let alice_port = alice_handle.local_port();
        let (bob_handle, mut bob_rx) = start(
            config(0, vec![format!("127.0.0.1:{alice_port}")]),
            bob,
            dir_b.path().join("identity.json"),
            dir_b.path().join("files"),
        )
        .await
        .unwrap();
        assert!(wait_peer_up(&mut alice_rx).await);
        assert!(wait_peer_up(&mut bob_rx).await);

        let source = dir_a.path().join("a.txt");
        std::fs::write(&source, b"aaa").unwrap();
        for id in ["t1", "t2"] {
            alice_handle
                .send(NetCommand::SendFile {
                    node_id: "bob-node".into(),
                    transfer_id: id.into(),
                    path: source.clone(),
                    name: "a.txt".into(),
                    size: 3,
                    folder: None,
                })
                .await;
        }
        let mut duplicate_error = false;
        for _ in 0..20 {
            match timeout(Duration::from_millis(500), alice_rx.recv()).await {
                Ok(Some(NetEvent::FileCommandError { message }))
                    if message.contains("已经发给") =>
                {
                    duplicate_error = true;
                    break;
                }
                Ok(Some(_)) => continue,
                _ => break,
            }
        }
        assert!(duplicate_error, "duplicate offer was not rejected");
        let offer = loop {
            match timeout(Duration::from_millis(500), bob_rx.recv()).await {
                Ok(Some(NetEvent::FileOfferReceived { transfer_id, .. })) => break transfer_id,
                Ok(Some(_)) => continue,
                _ => panic!("no initial offer"),
            }
        };
        assert_eq!(offer, "t1");

        let _ = alice_handle.send(NetCommand::Shutdown).await;
        let _ = bob_handle.send(NetCommand::Shutdown).await;
    }

    #[tokio::test]
    async fn multiple_offers_are_received_in_one_accept() {
        let alice = Identity {
            node_id: "alice-node".into(),
            name: "alice".into(),
            short_no: 1,
        };
        let bob = Identity {
            node_id: "bob-node".into(),
            name: "bob".into(),
            short_no: 1,
        };
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        let (alice_handle, mut alice_rx) = start(
            config(0, Vec::new()),
            alice,
            dir_a.path().join("identity.json"),
            dir_a.path().join("files"),
        )
        .await
        .unwrap();
        let alice_port = alice_handle.local_port();
        let (bob_handle, mut bob_rx) = start(
            config(0, vec![format!("127.0.0.1:{alice_port}")]),
            bob,
            dir_b.path().join("identity.json"),
            dir_b.path().join("files"),
        )
        .await
        .unwrap();
        assert!(wait_peer_up(&mut alice_rx).await);
        assert!(wait_peer_up(&mut bob_rx).await);

        for (id, name, body) in [
            ("t1", "a.txt", b"aaa".as_slice()),
            ("t2", "b.txt", b"bbb".as_slice()),
        ] {
            let path = dir_a.path().join(name);
            std::fs::write(&path, body).unwrap();
            alice_handle
                .send(NetCommand::SendFile {
                    node_id: "bob-node".into(),
                    transfer_id: id.into(),
                    path,
                    name: name.into(),
                    size: body.len() as u64,
                    folder: None,
                })
                .await;
        }

        let mut ids = Vec::new();
        while ids.len() < 2 {
            match timeout(Duration::from_millis(500), bob_rx.recv()).await {
                Ok(Some(NetEvent::FileOfferReceived { transfer_id, .. })) => {
                    ids.push(transfer_id);
                }
                Ok(Some(_)) => continue,
                _ => break,
            }
        }
        assert_eq!(ids.len(), 2);
        bob_handle
            .send(NetCommand::AcceptFiles {
                node_id: "alice-node".into(),
                transfer_ids: ids.clone(),
            })
            .await;

        let mut received = 0;
        let mut sent = 0;
        while received < 2 || sent < 2 {
            if let Ok(Some(NetEvent::FileReceived { .. })) =
                timeout(Duration::from_millis(500), bob_rx.recv()).await
            {
                received += 1;
            }
            if let Ok(Some(NetEvent::FileSent { .. })) =
                timeout(Duration::from_millis(50), alice_rx.recv()).await
            {
                sent += 1;
            }
        }
        let files = std::fs::read_dir(dir_b.path().join("files"))
            .unwrap()
            .count();
        assert_eq!(files, 2);

        let _ = alice_handle.send(NetCommand::Shutdown).await;
        let _ = bob_handle.send(NetCommand::Shutdown).await;
    }

    #[tokio::test]
    async fn two_managers_connect_and_chat_over_localhost() {
        let alice = Identity {
            node_id: "alice-node".into(),
            name: "alice".into(),
            short_no: 1,
        };
        let bob = Identity {
            node_id: "bob-node".into(),
            name: "bob".into(),
            short_no: 1,
        };
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        let (alice_handle, mut alice_rx) = start(
            config(0, Vec::new()),
            alice,
            dir_a.path().join("identity.json"),
            dir_a.path().to_path_buf(),
        )
        .await
        .unwrap();
        let alice_port = alice_handle.local_port();
        let (bob_handle, mut bob_rx) = start(
            config(0, vec![format!("127.0.0.1:{alice_port}")]),
            bob,
            dir_b.path().join("identity.json"),
            dir_b.path().to_path_buf(),
        )
        .await
        .unwrap();

        assert!(wait_peer_up(&mut alice_rx).await, "alice did not see bob");
        assert!(wait_peer_up(&mut bob_rx).await, "bob did not see alice");

        bob_handle
            .send(NetCommand::SendChat {
                msg_id: "msg-1".into(),
                body: "hello".into(),
            })
            .await;
        let chat = wait_chat(&mut alice_rx)
            .await
            .expect("alice did not receive chat");
        assert_eq!(chat.body, "hello");
        assert_eq!(chat.from, "bob-node");

        let _ = alice_handle.send(NetCommand::Shutdown).await;
        let _ = bob_handle.send(NetCommand::Shutdown).await;
    }
}
