use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use base64::Engine;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::net::protocol::{
    read_frame, write_frame, FileChunk, FileDataDone, FileDataHello, FileDataReady, WireMessage,
};

pub const MAX_FILE_BYTES: u64 = 1024 * 1024 * 1024;
pub const MAX_PENDING_PER_PEER: usize = 20;
pub const MAX_PENDING_TOTAL: usize = 100;
pub const CHUNK_BYTES: usize = 16 * 1024;
const DATA_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct OutgoingFile {
    pub transfer_id: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct IncomingFile {
    pub transfer_id: String,
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct ReceivedFile {
    pub transfer_id: String,
    pub name: String,
    pub path: PathBuf,
}

pub async fn send_files(
    addr: std::net::SocketAddr,
    node_id: &str,
    files: &[OutgoingFile],
) -> Result<()> {
    let mut stream = timeout(Duration::from_secs(3), TcpStream::connect(addr))
        .await
        .map_err(|_| anyhow::anyhow!("连接数据端口超时"))?
        .context("连接数据端口失败")?;
    let _ = stream.set_nodelay(true);
    let ids: Vec<String> = files.iter().map(|file| file.transfer_id.clone()).collect();
    write_frame(
        &mut stream,
        &WireMessage::FileDataHello(FileDataHello {
            node_id: node_id.to_string(),
            transfer_ids: ids.clone(),
            nonce: uuid::Uuid::new_v4().to_string(),
        }),
    )
    .await?;
    match read_frame(&mut stream).await? {
        WireMessage::FileDataReady(ready) => {
            if ready.transfer_ids.is_empty() {
                bail!("对方拒绝接收文件");
            }
        }
        WireMessage::FileError(error) => bail!("{}", error.message),
        _ => bail!("数据连接握手响应不合法"),
    }

    let mut buffer = vec![0u8; CHUNK_BYTES];
    for file in files {
        let mut input = tokio::fs::File::open(&file.path)
            .await
            .with_context(|| format!("打开文件失败: {}", file.path.display()))?;
        let mut seq = 0u64;
        loop {
            use tokio::io::AsyncReadExt;
            let read = input.read(&mut buffer).await?;
            if read == 0 {
                break;
            }
            let data = base64::engine::general_purpose::STANDARD.encode(&buffer[..read]);
            write_frame(
                &mut stream,
                &WireMessage::FileChunk(FileChunk {
                    transfer_id: file.transfer_id.clone(),
                    seq,
                    data,
                }),
            )
            .await?;
            seq += 1;
        }
    }
    write_frame(
        &mut stream,
        &WireMessage::FileDataDone(FileDataDone { transfer_ids: ids }),
    )
    .await?;
    stream.shutdown().await.ok();
    Ok(())
}

pub async fn receive_files(
    mut stream: TcpStream,
    hello: FileDataHello,
    expected: &[IncomingFile],
    files_dir: &Path,
) -> Result<Vec<ReceivedFile>> {
    let by_id: std::collections::HashMap<&str, &IncomingFile> = expected
        .iter()
        .map(|file| (file.transfer_id.as_str(), file))
        .collect();
    if hello.transfer_ids.is_empty()
        || hello
            .transfer_ids
            .iter()
            .any(|id| !by_id.contains_key(id.as_str()))
    {
        bail!("数据连接的 transfer_id 未接受");
    }
    let ids = hello.transfer_ids.clone();
    write_frame(
        &mut stream,
        &WireMessage::FileDataReady(FileDataReady {
            transfer_ids: ids.clone(),
        }),
    )
    .await?;

    tokio::fs::create_dir_all(files_dir).await?;
    set_dir_mode(files_dir, 0o700);
    let mut writers: std::collections::HashMap<String, Writer> = std::collections::HashMap::new();
    for id in &ids {
        let meta = by_id[id.as_str()];
        let path = unique_path(files_dir, &meta.name).await?;
        let part = part_path(files_dir, &meta.name, id);
        let file = tokio::fs::File::create(&part).await?;
        set_file_mode(&part, 0o600);
        writers.insert(
            id.clone(),
            Writer {
                meta: meta.clone(),
                part,
                path,
                file: Some(file),
                received: 0,
                next_seq: 0,
            },
        );
    }

    let result = async {
        loop {
            let frame = timeout(DATA_IDLE_TIMEOUT, read_frame(&mut stream))
                .await
                .map_err(|_| anyhow::anyhow!("数据连接超时"))??;
            match frame {
                WireMessage::FileChunk(chunk) => {
                    let Some(writer) = writers.get_mut(&chunk.transfer_id) else {
                        bail!("收到未知 transfer_id");
                    };
                    if chunk.seq != writer.next_seq {
                        bail!("文件块序号不连续");
                    }
                    let data = base64::engine::general_purpose::STANDARD
                        .decode(&chunk.data)
                        .context("文件块编码错误")?;
                    if data.len() > CHUNK_BYTES {
                        bail!("文件块过大");
                    }
                    if writer.received + data.len() as u64 > writer.meta.size {
                        bail!("文件超过声明大小");
                    }
                    if let Some(file) = writer.file.as_mut() {
                        file.write_all(&data).await?;
                    }
                    writer.received += data.len() as u64;
                    writer.next_seq += 1;
                }
                WireMessage::FileDataDone(done) => {
                    if done.transfer_ids.iter().any(|id| !writers.contains_key(id)) {
                        bail!("完成消息包含未知 transfer_id");
                    }
                    break;
                }
                WireMessage::FileCancel(_) | WireMessage::FileError(_) => {
                    bail!("对方取消/文件错误");
                }
                _ => bail!("数据连接收到非法帧"),
            }
        }
        let mut received = Vec::new();
        for (id, mut writer) in writers.drain() {
            if writer.received != writer.meta.size {
                bail!("文件大小不一致");
            }
            if let Some(mut file) = writer.file.take() {
                file.flush().await?;
                file.sync_all().await?;
            }
            if let Err(error) = tokio::fs::rename(&writer.part, &writer.path).await {
                let _ = tokio::fs::remove_file(&writer.part).await;
                return Err(error.into());
            }
            received.push(ReceivedFile {
                transfer_id: id,
                name: writer.meta.name.clone(),
                path: writer.path,
            });
        }
        Ok::<_, anyhow::Error>(received)
    }
    .await;

    if result.is_err() {
        for writer in writers.values() {
            let _ = tokio::fs::remove_file(&writer.part).await;
        }
    }
    result
}

struct Writer {
    meta: IncomingFile,
    part: PathBuf,
    path: PathBuf,
    file: Option<tokio::fs::File>,
    received: u64,
    next_seq: u64,
}

pub async fn unique_path(dir: &Path, name: &str) -> Result<PathBuf> {
    let name = sanitize_name(name)?;
    let candidate = dir.join(&name);
    if !tokio::fs::try_exists(&candidate).await.unwrap_or(false) {
        return Ok(candidate);
    }
    let path = Path::new(&name);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("file");
    let ext = path.extension().and_then(|value| value.to_str());
    for index in 0..10_000 {
        let candidate = match ext {
            Some(ext) => dir.join(format!("{stem}[{index}].{ext}")),
            None => dir.join(format!("{stem}[{index}]")),
        };
        if !tokio::fs::try_exists(&candidate).await.unwrap_or(false) {
            return Ok(candidate);
        }
    }
    bail!("无法生成不重复的文件名")
}

pub fn sanitize_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name == "." || name == ".." {
        bail!("文件名不合法");
    }
    if name.contains('/') || name.contains('\\') || name.contains('\0') {
        bail!("文件名包含路径分隔符");
    }
    let name: String = name.chars().filter(|ch| !ch.is_control()).collect();
    if name.is_empty() {
        bail!("文件名不合法");
    }
    let mut result = String::new();
    for ch in name.chars() {
        if result.len() + ch.len_utf8() > 255 {
            break;
        }
        result.push(ch);
    }
    if result.is_empty() {
        bail!("文件名不合法");
    }
    Ok(result)
}

fn part_path(dir: &Path, name: &str, transfer_id: &str) -> PathBuf {
    let safe = sanitize_name(name).unwrap_or_else(|_| "file".to_string());
    dir.join(format!(".{safe}.{transfer_id}.part"))
}

#[cfg(unix)]
fn set_dir_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
}

#[cfg(not(unix))]
fn set_dir_mode(_path: &Path, _mode: u32) {}

#[cfg(unix)]
fn set_file_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
}

#[cfg(not(unix))]
fn set_file_mode(_path: &Path, _mode: u32) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_names() {
        assert_eq!(sanitize_name("a.png").unwrap(), "a.png");
        assert!(sanitize_name("../a.png").is_err());
        assert!(sanitize_name("").is_err());
        assert!(sanitize_name("a/b").is_err());
    }

    #[tokio::test]
    async fn unique_path_uses_bracket_numbers() {
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::write(dir.path().join("a.png"), b"x")
            .await
            .unwrap();
        let path = unique_path(dir.path(), "a.png").await.unwrap();
        assert_eq!(path.file_name().unwrap(), "a[0].png");
    }
}
