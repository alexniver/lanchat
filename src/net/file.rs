use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use base64::Engine;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::net::protocol::{
    read_frame, write_frame, FileChunk, FileDataDone, FileDataHello, FileDataReady, FolderMeta,
    WireMessage,
};

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
    pub folder: Option<FolderMeta>,
}

#[derive(Debug, Clone)]
pub struct ReceivedFile {
    pub transfer_id: String,
    pub name: String,
    pub path: PathBuf,
    pub is_folder: bool,
}

#[derive(Debug, Clone)]
pub struct FolderArchive {
    pub path: PathBuf,
    pub archive_size: u64,
    pub total_size: u64,
    pub file_count: u32,
    pub empty_dirs: u32,
    pub skipped_symlinks: u32,
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
        let (path, part, is_folder) = if meta.folder.is_some() {
            let archive = files_dir.join(format!(".lanchat-folder-{id}.zip"));
            (archive.clone(), archive, true)
        } else {
            let path = unique_path(files_dir, &meta.name).await?;
            let part = part_path(files_dir, &meta.name, id);
            (path, part, false)
        };
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
                is_folder,
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
            if writer.is_folder {
                let root = writer
                    .meta
                    .folder
                    .as_ref()
                    .map(|folder| folder.root_name.clone())
                    .unwrap_or_else(|| writer.meta.name.clone());
                let archive = writer.part.clone();
                let dir = files_dir.to_path_buf();
                let extract_root = root.clone();
                let final_path = tokio::task::spawn_blocking(move || {
                    extract_folder(&archive, &dir, &extract_root)
                })
                .await??;
                received.push(ReceivedFile {
                    transfer_id: id,
                    name: root,
                    path: final_path,
                    is_folder: true,
                });
                continue;
            }
            if let Err(error) = tokio::fs::rename(&writer.part, &writer.path).await {
                let _ = tokio::fs::remove_file(&writer.part).await;
                return Err(error.into());
            }
            received.push(ReceivedFile {
                transfer_id: id,
                name: writer.meta.name.clone(),
                path: writer.path,
                is_folder: false,
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
    is_folder: bool,
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

    // Use the Windows-invalid set on every platform so a received file is
    // portable and cannot create NTFS alternate data streams.
    let mut cleaned = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*') {
            cleaned.push('_');
        } else {
            cleaned.push(ch);
        }
    }
    let cleaned = cleaned.trim_end_matches(['.', ' ']).trim();
    let mut result = if cleaned.is_empty() {
        "file".to_string()
    } else {
        cleaned.to_string()
    };

    if is_windows_reserved(&result) {
        result.insert(0, '_');
    }
    result = truncate_name(&result);
    if result.is_empty() {
        bail!("文件名不合法");
    }
    Ok(result)
}

fn is_windows_reserved(name: &str) -> bool {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(name);
    let stem = stem.trim_end_matches(['.', ' ']).to_ascii_uppercase();
    if matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$" | "CONIN$" | "CONOUT$"
    ) {
        return true;
    }
    let bytes = stem.as_bytes();
    if bytes.len() == 4 && (bytes[0..3] == *b"COM" || bytes[0..3] == *b"LPT") {
        return bytes[3].is_ascii_digit() && bytes[3] != b'0';
    }
    false
}

fn truncate_name(name: &str) -> String {
    const MAX_BYTES: usize = 255;
    if name.len() <= MAX_BYTES {
        return name.to_string();
    }
    let path = Path::new(name);
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| format!(".{extension}"))
        .unwrap_or_default();
    let extension = if extension.len() < MAX_BYTES {
        extension
    } else {
        String::new()
    };
    let stem = name.strip_suffix(&extension).unwrap_or(name);
    let budget = MAX_BYTES.saturating_sub(extension.len());
    let mut end = 0;
    for (index, ch) in stem.char_indices() {
        if index + ch.len_utf8() > budget {
            break;
        }
        end = index + ch.len_utf8();
    }
    let stem = if end == 0 { "file" } else { &stem[..end] };
    format!("{stem}{extension}")
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

pub fn create_folder_archive(source: &Path) -> Result<FolderArchive> {
    if !source.is_dir() {
        bail!("不是文件夹: {}", source.display());
    }
    let path = std::env::temp_dir().join(format!("lanchat-folder-{}.zip", uuid::Uuid::new_v4()));
    let file = fs::File::create(&path)
        .with_context(|| format!("创建文件夹压缩包失败: {}", path.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let mut archive = FolderArchive {
        path: path.clone(),
        archive_size: 0,
        total_size: 0,
        file_count: 0,
        empty_dirs: 0,
        skipped_symlinks: 0,
    };
    if let Err(error) = add_dir_to_archive(source, source, &mut zip, options, &mut archive) {
        let _ = fs::remove_file(&path);
        return Err(error);
    }
    let file = match zip.finish() {
        Ok(file) => file,
        Err(error) => {
            let _ = fs::remove_file(&path);
            return Err(error.into());
        }
    };
    file.sync_all().ok();
    drop(file);
    archive.archive_size = fs::metadata(&path)?.len();
    Ok(archive)
}

fn add_dir_to_archive(
    root: &Path,
    dir: &Path,
    zip: &mut zip::ZipWriter<fs::File>,
    options: zip::write::SimpleFileOptions,
    archive: &mut FolderArchive,
) -> Result<()> {
    let mut entries = fs::read_dir(dir)
        .with_context(|| format!("读取目录失败: {}", dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if metadata.file_type().is_symlink() {
            archive.skipped_symlinks += 1;
            continue;
        }
        if metadata.is_dir() {
            let name = format!("{relative}/");
            zip.add_directory(&name, options)?;
            let before = archive.empty_dirs;
            add_dir_to_archive(root, &path, zip, options, archive)?;
            let has_children = fs::read_dir(&path)?.next().is_some();
            if !has_children {
                archive.empty_dirs += 1;
            } else {
                archive.empty_dirs = before;
            }
        } else if metadata.is_file() {
            zip.start_file(&relative, options)?;
            let mut input = fs::File::open(&path)
                .with_context(|| format!("读取文件失败: {}", path.display()))?;
            std::io::copy(&mut input, zip)?;
            archive.total_size = archive.total_size.saturating_add(metadata.len());
            archive.file_count = archive.file_count.saturating_add(1);
        }
    }
    Ok(())
}

fn extract_folder(archive_path: &Path, files_dir: &Path, root_name: &str) -> Result<PathBuf> {
    let root = sanitize_name(root_name)?;
    let final_path = unique_path_sync(files_dir, &root)?;
    let staging = files_dir.join(format!(".lanchat-folder-{}.tmp", uuid::Uuid::new_v4()));
    fs::create_dir_all(&staging)?;
    let result = (|| -> Result<()> {
        let file = fs::File::open(archive_path)?;
        let mut zip = zip::ZipArchive::new(file)?;
        for index in 0..zip.len() {
            let mut entry = zip.by_index(index)?;
            let Some(path) = entry.enclosed_name() else {
                bail!("压缩包包含非法路径");
            };
            if entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
            {
                continue;
            }
            let mut target = staging.clone();
            for component in path.components() {
                let Component::Normal(value) = component else {
                    bail!("压缩包包含非法路径");
                };
                let value = value.to_string_lossy();
                target.push(sanitize_name(&value)?);
            }
            if entry.is_dir() {
                fs::create_dir_all(&target)?;
            } else {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut output = fs::File::create(&target)?;
                std::io::copy(&mut entry, &mut output)?;
                output.sync_all().ok();
            }
        }
        fs::rename(&staging, &final_path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
        let _ = fs::remove_file(archive_path);
        return result.map(|_| final_path);
    }
    let _ = fs::remove_file(archive_path);
    Ok(final_path)
}

fn unique_path_sync(dir: &Path, name: &str) -> Result<PathBuf> {
    let name = sanitize_name(name)?;
    let candidate = dir.join(&name);
    if !candidate.exists() {
        return Ok(candidate);
    }
    let path = Path::new(&name);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("folder");
    let ext = path.extension().and_then(|value| value.to_str());
    for index in 0..10_000 {
        let candidate = match ext {
            Some(ext) => dir.join(format!("{stem}[{index}].{ext}")),
            None => dir.join(format!("{stem}[{index}]")),
        };
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    bail!("无法生成不重复的文件夹名")
}

/// Free bytes available on the filesystem containing `path`.
pub fn available_space(path: &Path) -> Result<u64> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let path = CString::new(path.as_os_str().as_bytes())?;
        let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(path.as_ptr(), &mut stat) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(as_u64(stat.f_bavail).saturating_mul(as_u64(stat.f_frsize)))
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use std::ptr::null_mut;
        #[link(name = "kernel32")]
        extern "system" {
            fn GetDiskFreeSpaceExW(
                directory: *const u16,
                free_to_caller: *mut u64,
                total: *mut u64,
                free_total: *mut u64,
            ) -> i32;
        }
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut free = 0u64;
        let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, null_mut(), null_mut()) };
        if ok == 0 {
            Err(std::io::Error::last_os_error().into())
        } else {
            Ok(free)
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        bail!("当前平台不支持磁盘空间检测")
    }
}

#[cfg(unix)]
fn as_u64(value: impl Into<u64>) -> u64 {
    value.into()
}

pub fn required_for_transfer(bytes: u64) -> u64 {
    bytes.saturating_mul(3).saturating_add(1) / 2
}

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
    #[test]
    fn sanitizes_windows_invalid_characters_and_trailing_dots() {
        assert_eq!(sanitize_name("a:b?.png").unwrap(), "a_b_.png");
        assert_eq!(sanitize_name("name. ").unwrap(), "name");
        assert_eq!(sanitize_name("... ").unwrap(), "file");
        assert_eq!(sanitize_name("中文.png").unwrap(), "中文.png");
        assert!(sanitize_name("a/b.png").is_err());
        assert!(sanitize_name("a\\b.png").is_err());
    }

    #[test]
    fn avoids_windows_reserved_device_names() {
        assert_eq!(sanitize_name("CON").unwrap(), "_CON");
        assert_eq!(sanitize_name("con.txt").unwrap(), "_con.txt");
        assert_eq!(sanitize_name("LPT1.log").unwrap(), "_LPT1.log");
        assert_eq!(sanitize_name("COM0.txt").unwrap(), "COM0.txt");
    }

    #[test]
    fn truncates_long_names_preserving_extension() {
        let name = format!("{}.png", "a".repeat(300));
        let safe = sanitize_name(&name).unwrap();
        assert!(safe.len() <= 255);
        assert!(safe.ends_with(".png"));
    }
    #[tokio::test]
    async fn creates_and_extracts_a_folder_archive() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("holiday");
        std::fs::create_dir_all(source.join("sub")).unwrap();
        std::fs::write(source.join("a.txt"), b"one").unwrap();
        std::fs::write(source.join("sub/b.txt"), b"two").unwrap();
        std::fs::create_dir_all(source.join("empty")).unwrap();
        let archive = create_folder_archive(&source).unwrap();
        assert!(archive.file_count >= 2);
        let files = tempfile::tempdir().unwrap();
        let final_path = tokio::task::spawn_blocking({
            let archive = archive.path.clone();
            let files = files.path().to_path_buf();
            move || extract_folder(&archive, &files, "holiday")
        })
        .await
        .unwrap()
        .unwrap();
        assert_eq!(std::fs::read(final_path.join("a.txt")).unwrap(), b"one");
        assert_eq!(std::fs::read(final_path.join("sub/b.txt")).unwrap(), b"two");
        assert!(final_path.join("empty").is_dir());
    }

    #[test]
    fn disk_space_checks_use_a_margin() {
        assert_eq!(required_for_transfer(100), 150);
        let dir = tempfile::tempdir().unwrap();
        assert!(available_space(dir.path()).is_ok());
    }
    #[tokio::test]
    async fn folder_archive_skips_symlinks_and_renames_collision() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("holiday");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("a.txt"), b"one").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(source.join("a.txt"), source.join("link.txt")).unwrap();
        let archive = create_folder_archive(&source).unwrap();
        assert!(archive.file_count >= 1);
        #[cfg(unix)]
        assert_eq!(archive.skipped_symlinks, 1);
        let files = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(files.path().join("holiday")).unwrap();
        let final_path = tokio::task::spawn_blocking({
            let archive = archive.path.clone();
            let files = files.path().to_path_buf();
            move || extract_folder(&archive, &files, "holiday")
        })
        .await
        .unwrap()
        .unwrap();
        assert_eq!(final_path.file_name().unwrap(), "holiday[0]");
        assert_eq!(std::fs::read(final_path.join("a.txt")).unwrap(), b"one");
    }
}
