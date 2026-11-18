use serde::{Deserialize, Serialize};

/// 所有网络消息，JSON 换行分隔
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "payload")]
pub enum Message {
    Hello(HelloPayload),
    Chat(ChatPayload),
    FileListRequest,
    FileListResponse(FileListPayload),
    FileRequest(FileRequestPayload),
    FileResponse(FileResponsePayload),
    Goodbye,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HelloPayload {
    pub node_id: String,
    pub display_name: String,
    pub version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatPayload {
    pub msg_id: String,
    pub from_node_id: String,
    pub from_name: String,
    pub content: String,
    pub timestamp: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileListPayload {
    pub node_id: String,
    pub files: Vec<FileEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub size: u64,
    pub sha256: String,
    pub modified: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileRequestPayload {
    pub file_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileResponsePayload {
    pub file_name: String,
    pub size: u64,
    pub sha256: String,
    pub data: String,
    pub not_found: bool,
}
