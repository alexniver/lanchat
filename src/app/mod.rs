pub mod action;
pub mod command;
pub mod compose;
pub mod input;
pub mod list;
pub mod message;

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use crossterm::event::Event;

use crate::clipboard::Clipboard;
use crate::config::Config;
use crate::key::{ActionId, Key, Keymap, Lookup};
use crate::net::file::{sanitize_name, MAX_FILE_BYTES};
use crate::net::{ConnectionState, NetEvent};
use crate::storage::HistoryHandle;

pub use action::{Effect, Status, StatusKind};
pub use compose::ComposeState;
pub use input::InputState;
pub use list::ListState;
pub use message::{Message, MessageKind, MAX_BODY_BYTES};

#[derive(Debug)]
pub enum Layer {
    List(ListState),
    Compose(ComposeState),
    Members(MemberState),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MemberState {
    pub selected: usize,
}

#[derive(Debug, Clone)]
pub struct PeerView {
    pub name: String,
    pub addr: SocketAddr,
}

#[derive(Debug, Clone)]
pub struct PendingFileOffer {
    pub node_id: String,
    pub transfer_id: String,
    pub file_name: String,
}

pub struct App {
    pub config: Config,
    pub keymap: Keymap,
    pub messages: Vec<Message>,
    pub layers: Vec<Layer>,
    pub peers: BTreeMap<String, PeerView>,
    pub connection: ConnectionState,
    pub unread: usize,
    pub pending_file_offers: Vec<PendingFileOffer>,
    pub status: Option<Status>,
    effects: Vec<Effect>,
    history: HistoryHandle,
    clipboard: Clipboard,
    pending: Vec<Key>,
    pending_at: Option<Instant>,
    member_space: bool,
    outgoing_files: BTreeMap<String, (String, String)>,
    dirty: bool,
    should_quit: bool,
}

impl App {
    pub fn new(
        config: Config,
        keymap: Keymap,
        history: HistoryHandle,
        messages: Vec<Message>,
        warnings: Vec<String>,
    ) -> Self {
        let layers = vec![Layer::List(ListState::new(messages.len()))];
        let status = if let Some(warning) = warnings.first() {
            Some(Status::new(
                warning.clone(),
                StatusKind::Error,
                Duration::from_secs(8),
            ))
        } else if config.ui.show_help_hint {
            Some(Status::new(
                format!(
                    "{}:输入 {}/{}:选择 {}/{}:复制 {}:退出",
                    keymap.hint(ActionId::OpenCompose),
                    keymap.hint(ActionId::CursorDown),
                    keymap.hint(ActionId::CursorUp),
                    keymap.hint(ActionId::CopyBody),
                    keymap.hint(ActionId::CopyLine),
                    keymap.hint(ActionId::Quit),
                ),
                StatusKind::Info,
                Duration::from_secs(6),
            ))
        } else {
            None
        };
        let connection = if config.network.enabled {
            ConnectionState::Connecting
        } else {
            ConnectionState::Local
        };
        Self {
            config,
            keymap,
            messages,
            layers,
            peers: BTreeMap::new(),
            connection,
            unread: 0,
            pending_file_offers: Vec::new(),
            status,
            effects: Vec::new(),
            history,
            clipboard: Clipboard::new(),
            pending: Vec::new(),
            pending_at: None,
            member_space: false,
            outgoing_files: BTreeMap::new(),
            dirty: true,
            should_quit: false,
        }
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn top_is_compose(&self) -> bool {
        matches!(self.layers.last(), Some(Layer::Compose(_)))
    }

    pub fn top_is_members(&self) -> bool {
        matches!(self.layers.last(), Some(Layer::Members(_)))
    }

    pub fn network_enabled(&self) -> bool {
        self.config.network.enabled
    }

    pub fn online_count(&self) -> usize {
        self.peers.len() + 1
    }

    pub fn member_count(&self) -> usize {
        self.peers.len() + 1
    }

    pub fn list(&self) -> Option<&ListState> {
        match self.layers.first() {
            Some(Layer::List(list)) => Some(list),
            _ => None,
        }
    }

    fn list_mut(&mut self) -> Option<&mut ListState> {
        match self.layers.first_mut() {
            Some(Layer::List(list)) => Some(list),
            _ => None,
        }
    }

    pub fn selected_message(&self) -> Option<&Message> {
        let selected = self.list()?.selected?;
        self.messages.get(selected)
    }

    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    pub fn handle_terminal(&mut self, event: Event) {
        match event {
            Event::Key(key) => {
                if let Some(key) = Key::from_event(key) {
                    self.handle_key(key);
                }
            }
            Event::Paste(text) => {
                if let Some(Layer::Compose(compose)) = self.layers.last_mut() {
                    compose.input.insert_text(&text);
                    self.dirty = true;
                }
            }
            Event::Resize(_, _) => {
                self.dirty = true;
            }
            _ => {}
        }
    }

    pub fn handle_key(&mut self, key: Key) {
        if self.top_is_compose() {
            self.handle_compose_key(key);
        } else if self.top_is_members() {
            self.handle_members_key(key);
        } else {
            self.handle_list_key(key);
        }
    }

    fn handle_list_key(&mut self, key: Key) {
        if key == Key::Esc && !self.pending.is_empty() {
            self.clear_pending();
            return;
        }
        if key == Key::Ctrl('c') {
            self.clear_pending();
            self.status = None;
            self.dirty = true;
            return;
        }

        let mut sequence = self.pending.clone();
        sequence.push(key);
        match self.keymap.lookup(&sequence) {
            Lookup::Action(action) => {
                self.pending.clear();
                self.pending_at = None;
                self.apply_list_action(action);
            }
            Lookup::Prefix => {
                self.pending = sequence;
                self.pending_at = Some(Instant::now());
                self.dirty = true;
            }
            Lookup::None => {
                if self.pending.is_empty() {
                    return;
                }
                self.clear_pending();
                if let Lookup::Action(action) = self.keymap.lookup(&[key]) {
                    self.apply_list_action(action);
                } else if self.keymap.lookup(&[key]) == Lookup::Prefix {
                    self.pending.push(key);
                    self.pending_at = Some(Instant::now());
                    self.dirty = true;
                }
            }
        }
    }

    fn open_compose(&mut self, prefix: &str) {
        let Some(list) = self.list() else {
            return;
        };
        let mut compose = ComposeState::new(list.selected, list.follow_latest);
        if !prefix.is_empty() {
            compose.input.insert_text(prefix);
        }
        self.layers.push(Layer::Compose(compose));
        self.status = None;
        self.dirty = true;
    }

    fn apply_list_action(&mut self, action: ActionId) {
        match action {
            ActionId::OpenCompose => self.open_compose(""),
            ActionId::OpenCommand => self.open_compose("/"),
            ActionId::CursorDown => {
                let len = self.messages.len();
                if let Some(list) = self.list_mut() {
                    list.move_selection(1, len);
                    if list.follow_latest {
                        self.unread = 0;
                    }
                }
                self.dirty = true;
            }
            ActionId::CursorUp => {
                let len = self.messages.len();
                if let Some(list) = self.list_mut() {
                    list.move_selection(-1, len);
                    if list.follow_latest {
                        self.unread = 0;
                    }
                }
                self.dirty = true;
            }
            ActionId::CopyBody => self.copy_selected(false),
            ActionId::CopyLine => self.copy_selected(true),
            ActionId::ToggleMembers => {
                self.layers.push(Layer::Members(MemberState::default()));
                self.member_space = false;
                self.status = None;
                self.dirty = true;
            }
            ActionId::Reconnect => {
                self.effects.push(Effect::Reconnect);
                self.set_status("重连中", StatusKind::Info);
            }
            ActionId::Disconnect => {
                self.effects.push(Effect::Disconnect);
                self.set_status("已断开", StatusKind::Info);
            }
            ActionId::Quit => {
                self.should_quit = true;
                self.dirty = true;
            }
            ActionId::Noop => {
                self.status = None;
                self.dirty = true;
            }
        }
    }

    fn handle_members_key(&mut self, key: Key) {
        if key == Key::Esc {
            self.close_members();
            return;
        }
        if key == Key::Space {
            self.member_space = true;
            return;
        }
        if self.member_space {
            self.member_space = false;
            match key {
                Key::Char('u') => self.close_members(),
                Key::Char('r') => {
                    self.effects.push(Effect::Reconnect);
                    self.set_status("重连中", StatusKind::Info);
                }
                Key::Char('d') => {
                    self.effects.push(Effect::Disconnect);
                    self.set_status("已断开", StatusKind::Info);
                }
                _ => {}
            }
            return;
        }
        match key {
            Key::Char('j') => self.move_member(1),
            Key::Char('k') => self.move_member(-1),
            Key::Char('Q') => {
                self.should_quit = true;
                self.dirty = true;
            }
            _ => {}
        }
    }

    fn move_member(&mut self, delta: i32) {
        let count = self.member_count();
        if count == 0 {
            return;
        }
        if let Some(Layer::Members(members)) = self.layers.last_mut() {
            let next = (members.selected as i32 + delta).clamp(0, count as i32 - 1) as usize;
            members.selected = next;
            self.dirty = true;
        }
    }

    fn close_members(&mut self) {
        if self.top_is_members() {
            self.layers.pop();
        }
        self.status = None;
        self.dirty = true;
    }

    pub fn handle_net_event(&mut self, event: NetEvent) {
        match event {
            NetEvent::PeerUp {
                node_id,
                name,
                addr,
            } => {
                self.peers.insert(
                    node_id.clone(),
                    PeerView {
                        name: name.clone(),
                        addr,
                    },
                );
                self.connection = ConnectionState::Online;
                self.set_status(format!("{name} 加入"), StatusKind::Info);
                self.dirty = true;
            }
            NetEvent::PeerDown { node_id, reason } => {
                let name = self
                    .peers
                    .remove(&node_id)
                    .map(|peer| peer.name)
                    .unwrap_or_else(|| node_id.clone());
                let had_pending = self
                    .pending_file_offers
                    .iter()
                    .any(|offer| offer.node_id == node_id);
                self.pending_file_offers
                    .retain(|offer| offer.node_id != node_id);
                self.outgoing_files
                    .retain(|_, (peer_id, _)| peer_id != &node_id);
                if had_pending {
                    self.push_system(format!("{name} 已断开，未完成的文件请求已取消。"));
                }
                self.connection = if self.peers.is_empty() && self.network_enabled() {
                    ConnectionState::Reconnecting
                } else if self.peers.is_empty() {
                    ConnectionState::Local
                } else {
                    ConnectionState::Online
                };
                self.set_status(format!("{name} 离开: {reason}"), StatusKind::Info);
                self.dirty = true;
            }
            NetEvent::ChatReceived(chat) => {
                let message = Message::remote(&chat);
                self.messages.push(message.clone());
                self.effects.push(Effect::Persist(message));
                let message_count = self.messages.len();
                let mut unread_status = false;
                if let Some(list) = self.list_mut() {
                    if list.follow_latest {
                        list.selected = message_count.checked_sub(1);
                        self.unread = 0;
                    } else {
                        self.unread += 1;
                        unread_status = true;
                    }
                }
                if unread_status {
                    self.set_status("有新消息", StatusKind::Info);
                }
                self.dirty = true;
            }
            NetEvent::SendFailed { msg_id, reason } => {
                tracing::warn!(%msg_id, %reason, "发送失败");
                self.set_status(format!("发送失败: {reason}"), StatusKind::Error);
            }
            NetEvent::FileOfferReceived {
                node_id,
                peer_name,
                addr,
                transfer_id,
                file_name,
                size,
            } => {
                self.pending_file_offers.push(PendingFileOffer {
                    node_id: node_id.clone(),
                    transfer_id,
                    file_name: file_name.clone(),
                });
                let last = addr.ip().to_string();
                let last = last.rsplit('.').next().unwrap_or(&last).to_string();
                self.push_system(format!(
                    "{peer_name} 想发送文件 {file_name}（{}）。输入 /recv file {last} 接收。",
                    format_size(size)
                ));
            }
            NetEvent::FileOfferSent {
                node_id,
                transfer_id,
                file_name,
            } => {
                let peer_name = self.peer_display(&node_id);
                self.outgoing_files
                    .insert(transfer_id, (node_id, file_name.clone()));
                self.push_system(format!(
                    "已向 {peer_name} 发送 {file_name} 的接收请求，等待对方接收（10 分钟内有效）。"
                ));
            }
            NetEvent::FileAccepted {
                node_id,
                transfer_ids,
            } => {
                let peer_name = self.peer_display(&node_id);
                let mut names = Vec::new();
                self.pending_file_offers.retain(|offer| {
                    if offer.node_id == node_id && transfer_ids.contains(&offer.transfer_id) {
                        names.push(offer.file_name.clone());
                        false
                    } else {
                        true
                    }
                });
                if !names.is_empty() {
                    self.push_system(format!(
                        "开始接收来自 {peer_name} 的文件：{}…",
                        names.join("、")
                    ));
                } else {
                    let names: Vec<String> = transfer_ids
                        .iter()
                        .filter_map(|id| self.outgoing_files.get(id).map(|(_, name)| name.clone()))
                        .collect();
                    if !names.is_empty() {
                        self.push_system(format!(
                            "{peer_name} 已接收 {}，开始发送…",
                            names.join("、")
                        ));
                    }
                }
            }
            NetEvent::FileReceived {
                node_id,
                transfer_id,
                file_name,
                path,
            } => {
                self.pending_file_offers
                    .retain(|offer| offer.transfer_id != transfer_id);
                let peer_name = self.peer_display(&node_id);
                self.push_system(format!(
                    "接收完成：来自 {peer_name} 的 {file_name} 已保存到 {}。",
                    path.display()
                ));
            }
            NetEvent::FileSent {
                node_id,
                transfer_id,
                file_name,
            } => {
                self.outgoing_files.remove(&transfer_id);
                let peer_name = self.peer_display(&node_id);
                self.push_system(format!("发送完成：{file_name} 已发送给 {peer_name}。"));
            }
            NetEvent::FileFailed {
                node_id,
                transfer_id,
                file_name,
                reason,
            } => {
                self.remove_file_transfer(&transfer_id);
                let peer_name = self.peer_display(&node_id);
                self.push_system(format!(
                    "文件 {file_name} 传输失败（{peer_name}）：{reason}。"
                ));
            }
            NetEvent::FileCancelled {
                node_id,
                transfer_id,
                file_name,
                reason,
            } => {
                self.remove_file_transfer(&transfer_id);
                let peer_name = self.peer_display(&node_id);
                self.push_system(format!("{peer_name} 取消了 {file_name} 的传输：{reason}。"));
            }
            NetEvent::FileTimedOut {
                node_id,
                transfer_id,
                file_name,
                outgoing,
            } => {
                self.remove_file_transfer(&transfer_id);
                let peer_name = self.peer_display(&node_id);
                if outgoing {
                    self.push_system(format!(
                        "{file_name} 的发送请求已取消：对方 10 分钟内没有接收。"
                    ));
                } else {
                    self.push_system(format!(
                        "{peer_name} 发来的 {file_name} 接收请求已取消：超过 10 分钟没有接收。"
                    ));
                }
            }
            NetEvent::FileCommandError { message } => {
                self.set_status(message, StatusKind::Error);
            }
            NetEvent::Status { state, detail } => {
                self.connection = state;
                if !detail.is_empty() {
                    self.set_status(detail, StatusKind::Info);
                }
                self.dirty = true;
            }
        }
    }

    fn handle_compose_key(&mut self, key: Key) {
        match key {
            Key::Enter => {
                self.submit_compose();
                return;
            }
            Key::Esc => {
                self.cancel_compose();
                return;
            }
            _ => {}
        }

        let Some(Layer::Compose(compose)) = self.layers.last_mut() else {
            return;
        };
        let changed = match key {
            Key::Char(ch) => {
                compose.input.insert_char(ch);
                true
            }
            Key::Space => {
                compose.input.insert_char(' ');
                true
            }
            Key::Backspace => {
                compose.input.backspace();
                true
            }
            Key::Delete => {
                compose.input.delete();
                true
            }
            Key::Left => {
                compose.input.left();
                true
            }
            Key::Right => {
                compose.input.right();
                true
            }
            Key::Home => {
                compose.input.home();
                true
            }
            Key::End => {
                compose.input.end();
                true
            }
            Key::Ctrl('w') => {
                compose.input.delete_word_back();
                true
            }
            Key::Ctrl('u') => {
                compose.input.delete_to_start();
                true
            }
            Key::Ctrl('k') => {
                compose.input.delete_to_end();
                true
            }
            Key::Ctrl('a') => {
                compose.input.home();
                true
            }
            Key::Ctrl('e') => {
                compose.input.end();
                true
            }
            _ => false,
        };
        if changed {
            self.dirty = true;
        }
    }

    fn submit_compose(&mut self) {
        let Some(Layer::Compose(compose)) = self.layers.last() else {
            return;
        };
        let text = compose.input.trimmed().to_string();
        if text.is_empty() {
            self.cancel_compose();
            return;
        }
        if !crate::app::message::body_is_valid(&text) {
            let message = format!("消息过长（上限 {MAX_BODY_BYTES} 字节）");
            self.set_status(message, StatusKind::Error);
            return;
        }
        if text.starts_with('/') {
            match command::parse(&text) {
                command::ParsedCommand::Unknown => {}
                command::ParsedCommand::Invalid { usage } => {
                    self.set_status(usage, StatusKind::Error);
                    return;
                }
                command::ParsedCommand::SendFile { path, peer } => {
                    self.handle_send_file(&path, &peer);
                    return;
                }
                command::ParsedCommand::RecvFile { peer } => {
                    self.handle_recv_file(&peer);
                    return;
                }
            }
        }
        let saved_selected = compose.saved_selected;
        let saved_follow_latest = compose.saved_follow_latest;
        let name = self.config.identity.name.clone();
        let mut message = Message::new(name, text);
        message.from = self.config.identity.node_id.clone();
        let msg_id = message.id.clone();
        let body = message.body.clone();
        self.messages.push(message.clone());
        self.effects.push(Effect::Persist(message));
        if self.config.network.enabled {
            self.effects.push(Effect::SendChat { msg_id, body });
        }
        self.layers.pop();
        let last = self.messages.len() - 1;
        if let Some(list) = self.list_mut() {
            if saved_follow_latest {
                list.on_new_message(last + 1);
            } else {
                list.selected = saved_selected;
                list.follow_latest = false;
            }
        }
        self.set_status("已发送", StatusKind::Success);
    }

    fn handle_send_file(&mut self, raw_path: &str, peer_text: &str) {
        if !self.config.network.enabled {
            self.set_status("当前是本地模式，不能发送文件。", StatusKind::Error);
            return;
        }
        let path = match command::resolve_path(raw_path) {
            Ok(path) => path,
            Err(error) => {
                self.set_status(error, StatusKind::Error);
                return;
            }
        };
        let metadata = match std::fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                self.set_status(
                    format!("无法读取 {}：{error}", path.display()),
                    StatusKind::Error,
                );
                return;
            }
        };
        if !metadata.is_file() {
            self.set_status(
                "不是普通文件；0.1.2 只支持发送单个文件。",
                StatusKind::Error,
            );
            return;
        }
        if metadata.len() > MAX_FILE_BYTES {
            self.set_status("文件太大（上限 1 GiB），请换一个文件。", StatusKind::Error);
            return;
        }
        let name = match path.file_name().and_then(|name| name.to_str()) {
            Some(name) => match sanitize_name(name) {
                Ok(name) => name,
                Err(error) => {
                    self.set_status(error.to_string(), StatusKind::Error);
                    return;
                }
            },
            None => {
                self.set_status("无法确定文件名", StatusKind::Error);
                return;
            }
        };
        let (node_id, peer_name, _addr) = match self.resolve_peer(peer_text) {
            Ok(peer) => peer,
            Err(error) => {
                self.set_status(error, StatusKind::Error);
                return;
            }
        };
        if self
            .pending_file_offers
            .iter()
            .any(|offer| offer.node_id == node_id && offer.file_name == name)
        {
            self.set_status(
                format!("{name} 的请求已经发给对方了，请等对方接收完成后再发。"),
                StatusKind::Error,
            );
            return;
        }
        let transfer_id = uuid::Uuid::new_v4().to_string();
        self.effects.push(Effect::SendFile {
            node_id: node_id.clone(),
            transfer_id,
            path,
            name: name.clone(),
            size: metadata.len(),
        });
        self.layers.pop();
        self.set_status(
            format!("正在向 {peer_name} 发送 {name} 的接收请求…"),
            StatusKind::Info,
        );
        self.dirty = true;
    }

    fn handle_recv_file(&mut self, peer_text: &str) {
        if !self.config.network.enabled {
            self.set_status("当前是本地模式，不能接收文件。", StatusKind::Error);
            return;
        }
        let (node_id, peer_name, _addr) = match self.resolve_peer(peer_text) {
            Ok(peer) => peer,
            Err(error) => {
                self.set_status(error, StatusKind::Error);
                return;
            }
        };
        let ids: Vec<String> = self
            .pending_file_offers
            .iter()
            .filter(|offer| offer.node_id == node_id)
            .map(|offer| offer.transfer_id.clone())
            .collect();
        if ids.is_empty() {
            self.set_status(
                format!("没有来自 {peer_name} 的待接收文件。"),
                StatusKind::Error,
            );
            return;
        }
        self.effects.push(Effect::AcceptFiles {
            node_id: node_id.clone(),
            transfer_ids: ids.clone(),
        });
        self.layers.pop();
        self.set_status(
            format!("开始接收来自 {peer_name} 的文件…"),
            StatusKind::Info,
        );
        self.dirty = true;
    }

    fn resolve_peer(&self, text: &str) -> Result<(String, String, SocketAddr), String> {
        let text = text.trim();
        let mut matches = Vec::new();
        for (node_id, peer) in &self.peers {
            let ip = peer.addr.ip().to_string();
            let last = ip.rsplit('.').next().unwrap_or("");
            if ip == text || last == text || peer.name.eq_ignore_ascii_case(text) {
                matches.push((node_id.clone(), peer.name.clone(), peer.addr));
            }
        }
        match matches.len() {
            1 => Ok(matches.remove(0)),
            0 => Err(format!(
                "找不到在线用户 {text}。请确认对方在线，或使用完整 IP。"
            )),
            _ => Err(format!("{text} 匹配到多个在线用户，请改用完整 IP。")),
        }
    }

    fn cancel_compose(&mut self) {
        let Some(layer) = self.layers.pop() else {
            return;
        };
        if let Layer::Compose(compose) = layer {
            let message_count = self.messages.len();
            if let Some(list) = self.list_mut() {
                list.restore(
                    compose.saved_selected,
                    compose.saved_follow_latest,
                    message_count,
                );
            }
        }
        self.status = None;
        self.dirty = true;
    }

    fn copy_selected(&mut self, full_line: bool) {
        let Some(message) = self.selected_message() else {
            self.set_status("没有可复制的消息", StatusKind::Error);
            return;
        };
        let text = if full_line {
            message.format_line()
        } else {
            message.body.clone()
        };
        self.effects.push(Effect::Copy { text });
        self.dirty = true;
    }

    pub fn on_clipboard_result(&mut self, result: anyhow::Result<()>) {
        match result {
            Ok(()) => self.set_status("复制成功", StatusKind::Success),
            Err(error) => self.set_status(format!("复制失败: {error}"), StatusKind::Error),
        }
    }

    pub fn on_history_error(&mut self, error: String) {
        self.set_status(error, StatusKind::Error);
    }

    pub fn on_tick(&mut self, now: Instant) {
        if let Some(started) = self.pending_at {
            if now.duration_since(started) >= Duration::from_secs(1) {
                self.clear_pending();
            }
        }
        if self
            .status
            .as_ref()
            .is_some_and(|status| status.is_expired(now))
        {
            self.status = None;
            self.dirty = true;
        }
    }

    pub fn persist_message(&self, message: &Message) {
        self.history.append(message);
    }

    pub fn copy_text(&mut self, text: &str) -> anyhow::Result<()> {
        self.clipboard.copy(text)
    }

    pub fn shutdown(&mut self) {
        self.history.shutdown();
    }

    fn clear_pending(&mut self) {
        self.pending.clear();
        self.pending_at = None;
        self.dirty = true;
    }

    fn push_system(&mut self, text: impl Into<String>) {
        let message = Message::system(text);
        self.messages.push(message);
        let message_count = self.messages.len();
        let mut unread_status = false;
        if let Some(list) = self.list_mut() {
            if list.follow_latest {
                list.selected = message_count.checked_sub(1);
                self.unread = 0;
            } else {
                self.unread += 1;
                unread_status = true;
            }
        }
        if unread_status {
            self.set_status("有新消息", StatusKind::Info);
        }
        self.dirty = true;
    }

    fn remove_file_transfer(&mut self, transfer_id: &str) {
        self.pending_file_offers
            .retain(|offer| offer.transfer_id != transfer_id);
        self.outgoing_files.remove(transfer_id);
    }

    fn peer_display(&self, node_id: &str) -> String {
        self.peers
            .get(node_id)
            .map(|peer| peer.name.clone())
            .unwrap_or_else(|| node_id.to_string())
    }

    fn set_status(&mut self, text: impl Into<String>, kind: StatusKind) {
        self.status = Some(Status::new(text, kind, Duration::from_secs(2)));
        self.dirty = true;
    }
}

#[cfg(test)]
pub(crate) fn test_app(messages: Vec<Message>) -> App {
    use crate::config::{HistoryConfig, Identity, Paths, UiConfig};
    use crate::storage::HistoryHandle;
    use std::path::PathBuf;

    let config = Config {
        paths: Paths {
            config_dir: PathBuf::from("/tmp/lanchat-ui-test"),
            config_file: PathBuf::from("/tmp/lanchat-ui-test/config.toml"),
            keymap_file: PathBuf::from("/tmp/lanchat-ui-test/keymap.toml"),
            identity_file: PathBuf::from("/tmp/lanchat-ui-test/identity.json"),
            data_dir: PathBuf::from("/tmp/lanchat-ui-test/data"),
            history_dir: PathBuf::from("/tmp/lanchat-ui-test/history"),
            state_dir: PathBuf::from("/tmp/lanchat-ui-test/state"),
            log_dir: PathBuf::from("/tmp/lanchat-ui-test/logs"),
            panic_log: PathBuf::from("/tmp/lanchat-ui-test/panic.log"),
        },
        identity: Identity {
            node_id: "node".to_string(),
            name: "alice@arch".to_string(),
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

fn format_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.1} GiB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.1} MB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.1} KB", bytes / KIB)
    } else {
        format!("{} B", bytes as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{HistoryConfig, Identity, Paths, UiConfig};
    use crate::storage::HistoryHandle;
    use std::path::PathBuf;

    fn test_config() -> Config {
        Config {
            paths: Paths {
                config_dir: PathBuf::from("/tmp/lanchat-test"),
                config_file: PathBuf::from("/tmp/lanchat-test/config.toml"),
                keymap_file: PathBuf::from("/tmp/lanchat-test/keymap.toml"),
                identity_file: PathBuf::from("/tmp/lanchat-test/identity.json"),
                data_dir: PathBuf::from("/tmp/lanchat-test/data"),
                history_dir: PathBuf::from("/tmp/lanchat-test/history"),
                state_dir: PathBuf::from("/tmp/lanchat-test/state"),
                log_dir: PathBuf::from("/tmp/lanchat-test/logs"),
                panic_log: PathBuf::from("/tmp/lanchat-test/panic.log"),
            },
            identity: Identity {
                node_id: "node".to_string(),
                name: "alice@arch".to_string(),
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
        }
    }

    fn app_with(messages: Vec<Message>) -> App {
        App::new(
            test_config(),
            Keymap::defaults(),
            HistoryHandle::disabled(),
            messages,
            Vec::new(),
        )
    }

    fn message(body: &str) -> Message {
        Message::new("alice@arch", body)
    }

    #[test]
    fn compose_sends_and_auto_selects_latest() {
        let mut app = app_with(vec![message("one"), message("two")]);
        app.handle_key(Key::Char('i'));
        assert!(app.top_is_compose());
        for ch in "你好".chars() {
            app.handle_key(Key::Char(ch));
        }
        app.handle_key(Key::Enter);
        assert!(!app.top_is_compose());
        assert_eq!(app.messages.len(), 3);
        assert_eq!(app.messages[2].body, "你好");
        assert_eq!(app.list().unwrap().selected, Some(2));
        assert!(app.list().unwrap().follow_latest);
        assert!(matches!(
            app.take_effects().first(),
            Some(Effect::Persist(_))
        ));
    }

    #[test]
    fn compose_ignores_list_shortcuts_and_esc_cancels() {
        let mut app = app_with(vec![message("one"), message("two")]);
        app.handle_key(Key::Char('i'));
        for key in ['j', 'k', 'y', 'Y', 'i'] {
            app.handle_key(Key::Char(key));
        }
        assert_eq!(app.messages.len(), 2);
        let Layer::Compose(compose) = app.layers.last().unwrap() else {
            panic!("compose expected");
        };
        assert_eq!(compose.input.text, "jkyYi");
        app.handle_key(Key::Esc);
        assert!(!app.top_is_compose());
        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.list().unwrap().selected, Some(1));
    }

    #[test]
    fn manual_mode_does_not_follow_self_sent_messages() {
        let mut app = app_with(vec![message("one"), message("two")]);
        app.handle_key(Key::Char('k'));
        assert_eq!(app.list().unwrap().selected, Some(0));
        assert!(!app.list().unwrap().follow_latest);

        app.handle_key(Key::Char('i'));
        app.handle_key(Key::Char('x'));
        app.handle_key(Key::Enter);
        assert_eq!(app.messages.len(), 3);
        assert_eq!(app.list().unwrap().selected, Some(0));
        assert!(!app.list().unwrap().follow_latest);

        app.handle_key(Key::Char('j'));
        assert_eq!(app.list().unwrap().selected, Some(1));
        assert!(!app.list().unwrap().follow_latest);
        app.handle_key(Key::Char('j'));
        assert_eq!(app.list().unwrap().selected, Some(2));
        assert!(app.list().unwrap().follow_latest);
    }

    #[test]
    fn empty_enter_cancels_without_message() {
        let mut app = app_with(vec![message("one")]);
        app.handle_key(Key::Char('i'));
        app.handle_key(Key::Char(' '));
        app.handle_key(Key::Enter);
        assert_eq!(app.messages.len(), 1);
        assert!(!app.top_is_compose());
    }

    #[test]
    fn q_quits_in_list_but_is_text_in_compose() {
        let mut app = app_with(vec![]);
        app.handle_key(Key::Char('i'));
        app.handle_key(Key::Char('Q'));
        assert!(!app.should_quit());
        let Layer::Compose(compose) = app.layers.last().unwrap() else {
            panic!("compose expected");
        };
        assert_eq!(compose.input.text, "Q");
        app.handle_key(Key::Esc);
        app.handle_key(Key::Char('Q'));
        assert!(app.should_quit());
    }

    #[test]
    fn y_and_shift_y_copy_body_and_full_line() {
        let mut app = app_with(vec![message("hello")]);
        app.handle_key(Key::Char('y'));
        let effects = app.take_effects();
        assert!(matches!(&effects[0], Effect::Copy { text } if text == "hello"));

        app.handle_key(Key::Char('Y'));
        let effects = app.take_effects();
        assert!(
            matches!(&effects[0], Effect::Copy { text } if text.starts_with("[alice@arch][") && text.ends_with("]: hello"))
        );
    }

    #[test]
    fn clipboard_result_updates_status() {
        let mut app = app_with(vec![]);
        app.on_clipboard_result(Ok(()));
        assert_eq!(app.status.as_ref().unwrap().text, "复制成功");
        app.on_clipboard_result(Err(anyhow::anyhow!("no clipboard")));
        assert!(app.status.as_ref().unwrap().text.contains("复制失败"));
    }

    fn chat(body: &str) -> crate::net::protocol::Chat {
        crate::net::protocol::Chat {
            msg_id: format!("remote-{body}"),
            room: "lobby".into(),
            from: "peer-1".into(),
            name: "bob@arch".into(),
            seq: 1,
            sent_at: "2026-11-19T10:30:00+08:00".into(),
            body: body.into(),
        }
    }

    #[test]
    fn network_peer_events_update_online_count() {
        let mut app = app_with(vec![]);
        app.handle_net_event(NetEvent::PeerUp {
            node_id: "peer-1".into(),
            name: "bob@arch".into(),
            addr: "127.0.0.1:47732".parse().unwrap(),
        });
        assert_eq!(app.online_count(), 2);
        assert_eq!(app.connection, ConnectionState::Online);
        app.handle_net_event(NetEvent::PeerDown {
            node_id: "peer-1".into(),
            reason: "test".into(),
        });
        assert_eq!(app.online_count(), 1);
    }

    #[test]
    fn remote_chat_uses_auto_and_manual_selection() {
        let mut app = app_with(vec![message("one"), message("two")]);
        app.handle_net_event(NetEvent::ChatReceived(chat("three")));
        assert_eq!(app.messages.len(), 3);
        assert_eq!(app.messages[2].from, "peer-1");
        assert_eq!(app.list().unwrap().selected, Some(2));
        assert!(matches!(
            app.take_effects().first(),
            Some(Effect::Persist(_))
        ));

        app.handle_key(Key::Char('k'));
        assert!(!app.list().unwrap().follow_latest);
        app.handle_net_event(NetEvent::ChatReceived(chat("four")));
        assert_eq!(app.list().unwrap().selected, Some(1));
        assert_eq!(app.unread, 1);
        app.handle_key(Key::Char('j'));
        app.handle_key(Key::Char('j'));
        assert_eq!(app.list().unwrap().selected, Some(3));
        assert!(app.list().unwrap().follow_latest);
        assert_eq!(app.unread, 0);
    }

    #[test]
    fn member_sidebar_toggles_and_navigates() {
        let mut app = app_with(vec![]);
        app.handle_net_event(NetEvent::PeerUp {
            node_id: "peer-1".into(),
            name: "bob@arch".into(),
            addr: "127.0.0.1:47732".parse().unwrap(),
        });
        app.handle_key(Key::Space);
        app.handle_key(Key::Char('u'));
        assert!(app.top_is_members());
        app.handle_key(Key::Char('j'));
        let Layer::Members(members) = app.layers.last().unwrap() else {
            panic!("members expected");
        };
        assert_eq!(members.selected, 1);
        app.handle_key(Key::Space);
        app.handle_key(Key::Char('u'));
        assert!(!app.top_is_members());
    }

    #[test]
    fn member_sidebar_space_r_and_esc() {
        let mut app = app_with(vec![]);
        app.handle_key(Key::Space);
        app.handle_key(Key::Char('u'));
        app.handle_key(Key::Space);
        app.handle_key(Key::Char('r'));
        assert!(app
            .take_effects()
            .iter()
            .any(|effect| matches!(effect, Effect::Reconnect)));
        app.handle_key(Key::Esc);
        assert!(!app.top_is_members());
    }

    #[test]
    fn reconnect_and_disconnect_actions_emit_effects() {
        let mut app = app_with(vec![]);
        app.handle_key(Key::Space);
        app.handle_key(Key::Char('r'));
        assert!(app
            .take_effects()
            .iter()
            .any(|effect| matches!(effect, Effect::Reconnect)));
        app.handle_key(Key::Space);
        app.handle_key(Key::Char('d'));
        assert!(app
            .take_effects()
            .iter()
            .any(|effect| matches!(effect, Effect::Disconnect)));
    }

    #[test]
    fn manual_selection_restores_on_cancel() {
        let mut app = app_with(vec![message("one"), message("two"), message("three")]);
        app.handle_key(Key::Char('k'));
        let before = app.list().unwrap().selected;
        app.handle_key(Key::Char('i'));
        app.handle_key(Key::Esc);
        assert_eq!(app.list().unwrap().selected, before);
        assert!(!app.list().unwrap().follow_latest);
    }
    fn type_text(app: &mut App, text: &str) {
        for ch in text.chars() {
            app.handle_key(Key::Char(ch));
        }
    }

    #[test]
    fn slash_opens_prefilled_command_input() {
        let mut app = app_with(vec![]);
        app.handle_key(Key::Char('/'));
        assert!(app.top_is_compose());
        let Layer::Compose(compose) = app.layers.last().unwrap() else {
            panic!("compose expected");
        };
        assert_eq!(compose.input.text, "/");
        assert_eq!(compose.input.cursor, 1);
    }

    #[test]
    fn unknown_slash_command_is_sent_as_chat() {
        let mut app = app_with(vec![]);
        app.handle_key(Key::Char('/'));
        type_text(&mut app, "hello");
        app.handle_key(Key::Enter);
        assert!(!app.top_is_compose());
        assert_eq!(app.messages.len(), 1);
        assert_eq!(app.messages[0].body, "/hello");
        assert_eq!(app.messages[0].kind, MessageKind::User);
    }

    #[test]
    fn malformed_send_shows_usage_and_keeps_input() {
        let mut app = app_with(vec![]);
        app.handle_key(Key::Char('/'));
        type_text(&mut app, "send file");
        app.handle_key(Key::Enter);
        assert!(app.top_is_compose());
        assert!(app.messages.is_empty());
        assert!(app.status.as_ref().unwrap().text.contains("/send file"));
    }

    #[test]
    fn file_offer_creates_a_system_message() {
        let mut app = app_with(vec![]);
        app.handle_net_event(NetEvent::FileOfferReceived {
            node_id: "peer-1".into(),
            peer_name: "bob".into(),
            addr: "192.168.1.101:47731".parse().unwrap(),
            transfer_id: "t1".into(),
            file_name: "a.png".into(),
            size: 1200,
        });
        assert_eq!(app.pending_file_offers.len(), 1);
        assert_eq!(app.messages.len(), 1);
        assert_eq!(app.messages[0].kind, MessageKind::System);
        assert!(app.messages[0].body.contains("/recv file 101"));
        assert!(app.messages[0].body.contains("a.png"));
    }

    #[test]
    fn recv_file_emits_accept_for_all_pending() {
        let mut app = app_with(vec![]);
        app.config.network.enabled = true;
        app.handle_net_event(NetEvent::PeerUp {
            node_id: "peer-1".into(),
            name: "bob".into(),
            addr: "192.168.1.101:47731".parse().unwrap(),
        });
        app.handle_net_event(NetEvent::FileOfferReceived {
            node_id: "peer-1".into(),
            peer_name: "bob".into(),
            addr: "192.168.1.101:47731".parse().unwrap(),
            transfer_id: "t1".into(),
            file_name: "a.png".into(),
            size: 1200,
        });
        app.handle_net_event(NetEvent::FileOfferReceived {
            node_id: "peer-1".into(),
            peer_name: "bob".into(),
            addr: "192.168.1.101:47731".parse().unwrap(),
            transfer_id: "t2".into(),
            file_name: "b.png".into(),
            size: 1300,
        });
        app.handle_key(Key::Char('/'));
        type_text(&mut app, "recv file 101");
        app.handle_key(Key::Enter);
        let effects = app.take_effects();
        assert!(effects.iter().any(|effect| matches!(
            effect,
            Effect::AcceptFiles { transfer_ids, .. } if transfer_ids == &vec!["t1".to_string(), "t2".to_string()]
        )));
        assert!(!app.top_is_compose());
    }

    #[test]
    fn send_file_emits_effect_for_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.png");
        std::fs::write(&path, b"png").unwrap();
        let mut app = app_with(vec![]);
        app.config.network.enabled = true;
        app.handle_net_event(NetEvent::PeerUp {
            node_id: "peer-1".into(),
            name: "bob".into(),
            addr: "127.0.0.1:47731".parse().unwrap(),
        });
        app.handle_key(Key::Char('/'));
        type_text(
            &mut app,
            &format!("send file {} to 127.0.0.1", path.display()),
        );
        app.handle_key(Key::Enter);
        let effects = app.take_effects();
        assert!(effects.iter().any(|effect| matches!(
            effect,
            Effect::SendFile { name, size, .. } if name == "a.png" && *size == 3
        )));
        assert!(!app.top_is_compose());
    }
}
