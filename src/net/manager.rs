use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{SocketAddr, ToSocketAddrs};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Local;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};
use tokio::time::timeout;

use crate::config::{Identity, NetworkConfig};
use crate::net::discovery::{self, Discovery, DiscoveryEvent};
use crate::net::protocol::{
    read_frame, validate_chat, validate_hello, write_frame, Bye, Chat, Hello, Ping, Pong, Ready,
    WireMessage, MAGIC, MAX_BODY_BYTES, PROTOCOL_VERSION, ROOM,
};

const CHANNEL_CAPACITY: usize = 64;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const PING_INTERVAL: Duration = Duration::from_secs(10);
const MAX_SEEN: usize = 10_000;
const MAX_PEERS: usize = 64;

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
    SendChat { msg_id: String, body: String },
    Reconnect,
    Disconnect,
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum NetEvent {
    PeerUp {
        node_id: String,
        name: String,
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
        addr: SocketAddr,
        listen_port: u16,
        stream: TcpStream,
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
    last_seen: Instant,
    last_ping: Instant,
}

struct ReconnectState {
    addr: SocketAddr,
    attempts: u32,
    next_at: Instant,
}

struct Manager {
    listener: TcpListener,
    local: Identity,
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
    seen: HashSet<String>,
    seen_order: VecDeque<String>,
    seq: u64,
    manual_disconnect: bool,
    last_state: Option<ConnectionState>,
}

pub async fn start(
    config: NetworkConfig,
    identity: Identity,
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

    async fn handle_peer_event(&mut self, event: PeerEvent) {
        match event {
            PeerEvent::Connected {
                node_id,
                name,
                addr,
                listen_port,
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
                        addr: listen_addr,
                    })
                    .await;
                self.emit_status(ConnectionState::Online, "已连接".to_string());
            }
            PeerEvent::Message { node_id, message } => {
                self.handle_message(node_id, message).await;
            }
            PeerEvent::Closed { node_id, reason } => {
                self.remove_peer(&node_id, &reason).await;
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
                                    addr,
                                    listen_port: hello.listen_port,
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
            match handshake_inbound(stream, &local, listen_port).await {
                Ok((stream, hello)) => {
                    let _ = events
                        .send(PeerEvent::Connected {
                            node_id: hello.node_id,
                            name: hello.name,
                            addr,
                            listen_port: hello.listen_port,
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
        capabilities: vec!["chat".to_string()],
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

async fn handshake_inbound(
    mut stream: TcpStream,
    local: &Identity,
    listen_port: u16,
) -> Result<(TcpStream, Hello)> {
    timeout(HANDSHAKE_TIMEOUT, async {
        let received = read_frame(&mut stream).await?;
        let WireMessage::Hello(peer) = received else {
            anyhow::bail!("握手首帧不是 Hello");
        };
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
        for _ in 0..20 {
            match timeout(Duration::from_millis(500), rx.recv()).await {
                Ok(Some(NetEvent::PeerUp { .. })) => return true,
                Ok(Some(_)) => continue,
                Ok(None) => return false,
                Err(_) => return false,
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
                Err(_) => return None,
            }
        }
        None
    }

    #[tokio::test]
    async fn two_managers_connect_and_chat_over_localhost() {
        let alice = Identity {
            node_id: "alice-node".into(),
            name: "alice".into(),
        };
        let bob = Identity {
            node_id: "bob-node".into(),
            name: "bob".into(),
        };
        let (alice_handle, mut alice_rx) = start(config(0, Vec::new()), alice).await.unwrap();
        let alice_port = alice_handle.local_port();
        let (bob_handle, mut bob_rx) =
            start(config(0, vec![format!("127.0.0.1:{alice_port}")]), bob)
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
