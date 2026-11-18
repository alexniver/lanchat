use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

pub const MAX_BODY_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageKind {
    #[default]
    User,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Message {
    pub id: String,
    #[serde(default)]
    pub kind: MessageKind,
    /// Peer node id; empty for older/local records without a source.
    #[serde(default)]
    pub from: String,
    pub name: String,
    /// RFC3339 timestamp used for display (local receive time for remote messages).
    pub timestamp: String,
    /// Sender's timestamp, when known (remote messages).
    #[serde(default)]
    pub sent_at: String,
    pub body: String,
}

impl Message {
    pub fn new(name: impl Into<String>, body: impl Into<String>) -> Self {
        let timestamp = Local::now().to_rfc3339();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind: MessageKind::User,
            from: String::new(),
            name: name.into(),
            timestamp: timestamp.clone(),
            sent_at: timestamp,
            body: normalize_body(&body.into()),
        }
    }

    pub fn remote(chat: &crate::net::protocol::Chat) -> Self {
        Self {
            id: chat.msg_id.clone(),
            kind: MessageKind::User,
            from: chat.from.clone(),
            name: chat.name.clone(),
            timestamp: Local::now().to_rfc3339(),
            sent_at: chat.sent_at.clone(),
            body: normalize_body(&chat.body),
        }
    }

    pub fn system(body: impl Into<String>) -> Self {
        let timestamp = Local::now().to_rfc3339();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind: MessageKind::System,
            from: String::new(),
            name: "系统".to_string(),
            timestamp: timestamp.clone(),
            sent_at: timestamp,
            body: normalize_body(&body.into()),
        }
    }

    pub fn display_datetime(&self) -> String {
        DateTime::parse_from_rfc3339(&self.timestamp)
            .map(|dt| {
                dt.with_timezone(&Local)
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
            })
            .unwrap_or_else(|_| self.timestamp.clone())
    }

    pub fn format_line(&self) -> String {
        let name = match self.kind {
            MessageKind::User => self.name.as_str(),
            MessageKind::System => "系统",
        };
        format!("[{}][{}]: {}", name, self.display_datetime(), self.body)
    }
}

pub fn normalize_body(body: &str) -> String {
    let mut result = String::with_capacity(body.len());
    for ch in body.chars() {
        match ch {
            '\r' => {}
            '\n' | '\t' => result.push(' '),
            ch if ch.is_control() => result.push(' '),
            ch => result.push(ch),
        }
    }
    result.trim().to_string()
}

pub fn body_is_valid(body: &str) -> bool {
    !body.trim().is_empty() && body.len() <= MAX_BODY_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_line_as_required() {
        let message = Message {
            id: "id".into(),
            kind: MessageKind::User,
            from: String::new(),
            name: "alice@arch".into(),
            timestamp: "2026-11-19T10:30:05+08:00".into(),
            sent_at: String::new(),
            body: "你好".into(),
        };
        let line = message.format_line();
        assert!(line.starts_with("[alice@arch]["));
        assert!(line.ends_with("]: 你好"));
        assert!(!line.contains('\n'));
    }

    #[test]
    fn system_messages_have_their_own_prefix() {
        let message = Message::system("接收完成");
        assert_eq!(message.kind, MessageKind::System);
        assert!(message.format_line().starts_with("[系统]["));
        assert!(message.format_line().ends_with("]: 接收完成"));
    }

    #[test]
    fn normalizes_control_characters_and_newlines() {
        assert_eq!(normalize_body(" a\nb\tc\r\u{0}d "), "a b c d");
    }

    #[test]
    fn rejects_empty_and_oversized_bodies() {
        assert!(!body_is_valid("   "));
        assert!(!body_is_valid(&"x".repeat(MAX_BODY_BYTES + 1)));
        assert!(body_is_valid("ok"));
    }
}
