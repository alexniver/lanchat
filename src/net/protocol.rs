use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const MAGIC: &str = "lanchat";
pub const PROTOCOL_VERSION: u8 = 1;
pub const ROOM: &str = "lobby";
pub const MAX_FRAME_BYTES: usize = 64 * 1024;
pub const MAX_BODY_BYTES: usize = 16 * 1024;
pub const MAX_NAME_BYTES: usize = 256;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WireMessage {
    Hello(Hello),
    Ready(Ready),
    Chat(Chat),
    Ping(Ping),
    Pong(Pong),
    Bye(Bye),
    Error(ProtocolError),
    FileOffer(FileOffer),
    FileAccept(FileAccept),
    FileDataHello(FileDataHello),
    FileDataReady(FileDataReady),
    FileChunk(FileChunk),
    FileDataDone(FileDataDone),
    FileComplete(FileComplete),
    FileCancel(FileCancel),
    FileError(FileError),
    ShortNoUpdate(ShortNoUpdate),
    NameUpdate(NameUpdate),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Hello {
    pub magic: String,
    pub protocol: u8,
    pub node_id: String,
    pub name: String,
    pub version: String,
    /// TCP listen port for reconnects.
    #[serde(default)]
    pub listen_port: u16,
    #[serde(default)]
    pub short_no: u32,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub nonce: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NameUpdate {
    pub node_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShortNoUpdate {
    pub node_id: String,
    pub short_no: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Ready {
    pub node_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Chat {
    pub msg_id: String,
    pub room: String,
    pub from: String,
    pub name: String,
    #[serde(default)]
    pub seq: u64,
    pub sent_at: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Ping {
    pub nonce: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pong {
    pub nonce: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bye {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolError {
    pub code: String,
    pub message: String,
}

pub fn encode_frame(message: &WireMessage) -> Result<Vec<u8>> {
    let payload = serde_json::to_vec(message)?;
    if payload.len() > MAX_FRAME_BYTES {
        bail!("帧过大: {} 字节", payload.len());
    }
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

pub fn decode_frame(payload: &[u8]) -> Result<WireMessage> {
    if payload.len() > MAX_FRAME_BYTES {
        bail!("帧过大: {} 字节", payload.len());
    }
    Ok(serde_json::from_slice(payload)?)
}

pub async fn read_frame<R>(reader: &mut R) -> Result<WireMessage>
where
    R: AsyncRead + Unpin,
{
    let mut header = [0u8; 4];
    reader.read_exact(&mut header).await?;
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 {
        bail!("帧长度为 0");
    }
    if length > MAX_FRAME_BYTES {
        bail!("帧过大: {length} 字节");
    }
    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload).await?;
    decode_frame(&payload)
}

pub async fn write_frame<W>(writer: &mut W, message: &WireMessage) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    let frame = encode_frame(message)?;
    writer.write_all(&frame).await?;
    writer.flush().await?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileOffer {
    pub transfer_id: String,
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileAccept {
    pub transfer_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileDataHello {
    pub node_id: String,
    pub transfer_ids: Vec<String>,
    pub nonce: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileDataReady {
    pub transfer_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileChunk {
    pub transfer_id: String,
    pub seq: u64,
    /// base64-encoded bytes; <= 16 KiB raw.
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileDataDone {
    pub transfer_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileComplete {
    pub transfer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileCancel {
    pub transfer_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileError {
    pub transfer_id: String,
    pub code: String,
    pub message: String,
}

pub fn validate_file_offer(offer: &FileOffer) -> Result<()> {
    if offer.transfer_id.trim().is_empty() {
        bail!("transfer_id 为空");
    }
    if offer.name.trim().is_empty() || offer.name.len() > 255 {
        bail!("文件名不合法");
    }
    if offer.name.contains('/') || offer.name.contains('\\') || offer.name.contains('\0') {
        bail!("文件名包含路径分隔符");
    }
    if offer.name == "." || offer.name == ".." {
        bail!("文件名不合法");
    }
    Ok(())
}

pub fn validate_hello(hello: &Hello, local_node_id: &str) -> Result<()> {
    if hello.magic != MAGIC {
        bail!("magic 不匹配");
    }
    if hello.protocol != PROTOCOL_VERSION {
        bail!("协议版本不兼容: {}", hello.protocol);
    }
    if hello.node_id.trim().is_empty() {
        bail!("node_id 为空");
    }
    if hello.node_id == local_node_id {
        bail!("node_id 冲突");
    }
    if hello.name.len() > MAX_NAME_BYTES {
        bail!("昵称过长");
    }
    if hello.nonce.is_empty() {
        bail!("nonce 为空");
    }
    Ok(())
}

pub fn validate_chat(chat: &Chat) -> Result<()> {
    if chat.msg_id.trim().is_empty() {
        bail!("msg_id 为空");
    }
    if chat.room != ROOM {
        bail!("未知房间: {}", chat.room);
    }
    if chat.from.trim().is_empty() {
        bail!("from 为空");
    }
    if chat.body.len() > MAX_BODY_BYTES {
        bail!("消息过长: {} 字节", chat.body.len());
    }
    if chat.body.chars().any(|ch| ch == '\0') {
        bail!("消息包含 NUL");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello() -> Hello {
        Hello {
            magic: MAGIC.into(),
            protocol: PROTOCOL_VERSION,
            node_id: "node-a".into(),
            name: "alice@arch".into(),
            version: "0.1.2".into(),
            listen_port: 47731,
            short_no: 1,
            capabilities: vec!["chat".into(), "short_no".into()],
            nonce: "nonce".into(),
        }
    }

    fn chat() -> Chat {
        Chat {
            msg_id: "msg-1".into(),
            room: ROOM.into(),
            from: "node-a".into(),
            name: "alice@arch".into(),
            seq: 1,
            sent_at: "2026-11-19T10:30:00+08:00".into(),
            body: "你好".into(),
        }
    }

    #[test]
    fn roundtrips_messages() {
        for message in [
            WireMessage::Hello(hello()),
            WireMessage::Ready(Ready {
                node_id: "node-b".into(),
            }),
            WireMessage::Chat(chat()),
            WireMessage::Ping(Ping { nonce: "p".into() }),
            WireMessage::Pong(Pong { nonce: "p".into() }),
            WireMessage::Bye(Bye {
                reason: "bye".into(),
            }),
            WireMessage::Error(ProtocolError {
                code: "x".into(),
                message: "y".into(),
            }),
            WireMessage::FileOffer(FileOffer {
                transfer_id: "t1".into(),
                name: "a.png".into(),
                size: 123,
            }),
            WireMessage::FileAccept(FileAccept {
                transfer_ids: vec!["t1".into()],
            }),
            WireMessage::FileDataHello(FileDataHello {
                node_id: "node-a".into(),
                transfer_ids: vec!["t1".into()],
                nonce: "n".into(),
            }),
            WireMessage::FileDataReady(FileDataReady {
                transfer_ids: vec!["t1".into()],
            }),
            WireMessage::FileChunk(FileChunk {
                transfer_id: "t1".into(),
                seq: 0,
                data: "YWJj".into(),
            }),
            WireMessage::FileDataDone(FileDataDone {
                transfer_ids: vec!["t1".into()],
            }),
            WireMessage::FileComplete(FileComplete {
                transfer_id: "t1".into(),
            }),
            WireMessage::FileCancel(FileCancel {
                transfer_id: "t1".into(),
                reason: "timeout".into(),
            }),
            WireMessage::FileError(FileError {
                transfer_id: "t1".into(),
                code: "x".into(),
                message: "y".into(),
            }),
            WireMessage::ShortNoUpdate(ShortNoUpdate {
                node_id: "node-a".into(),
                short_no: 2,
            }),
            WireMessage::NameUpdate(NameUpdate {
                node_id: "node-a".into(),
                name: "alice".into(),
            }),
        ] {
            let encoded = encode_frame(&message).unwrap();
            let length = u32::from_be_bytes(encoded[..4].try_into().unwrap()) as usize;
            assert_eq!(length, encoded.len() - 4);
            assert_eq!(decode_frame(&encoded[4..]).unwrap(), message);
        }
    }

    #[test]
    fn validates_hello_and_chat() {
        assert!(validate_hello(&hello(), "node-b").is_ok());
        assert!(validate_hello(&hello(), "node-a").is_err());
        assert!(validate_chat(&chat()).is_ok());
        assert!(validate_file_offer(&FileOffer {
            transfer_id: "t1".into(),
            name: "a.png".into(),
            size: 1
        })
        .is_ok());
        assert!(validate_file_offer(&FileOffer {
            transfer_id: "t1".into(),
            name: "../a.png".into(),
            size: 1
        })
        .is_err());
        let mut bad = chat();
        bad.body = "x".repeat(MAX_BODY_BYTES + 1);
        assert!(validate_chat(&bad).is_err());
    }

    #[tokio::test]
    async fn reads_and_writes_frames() {
        let (mut client, mut server) = tokio::io::duplex(1024);
        let message = WireMessage::Chat(chat());
        write_frame(&mut client, &message).await.unwrap();
        assert_eq!(read_frame(&mut server).await.unwrap(), message);
    }
}
