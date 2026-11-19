use std::time::{Duration, Instant};

use crate::app::message::Message;

#[derive(Debug, Clone)]
pub enum Effect {
    Persist(Message),
    Copy {
        text: String,
    },
    SendChat {
        msg_id: String,
        body: String,
    },
    SendFile {
        node_id: String,
        transfer_id: String,
        path: std::path::PathBuf,
        name: String,
        size: u64,
        folder: Option<crate::net::protocol::FolderMeta>,
    },
    SendFolder {
        node_id: String,
        transfer_id: String,
        path: std::path::PathBuf,
        root_name: String,
        archive_size: u64,
        total_size: u64,
        file_count: u32,
        empty_dirs: u32,
    },
    AcceptFiles {
        node_id: String,
        transfer_ids: Vec<String>,
    },
    SetName {
        name: String,
    },
    Reconnect,
    Disconnect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone)]
pub struct Status {
    pub text: String,
    pub kind: StatusKind,
    pub expires_at: Option<Instant>,
}

impl Status {
    pub fn new(text: impl Into<String>, kind: StatusKind, ttl: Duration) -> Self {
        Self {
            text: text.into(),
            kind,
            expires_at: Some(Instant::now() + ttl),
        }
    }

    pub fn is_expired(&self, now: Instant) -> bool {
        self.expires_at.is_some_and(|expires| now >= expires)
    }
}
