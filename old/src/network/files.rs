//! 本地文件扫描与哈希计算。
//!
//! 扫描 `~/.local/share/lanchat/` 目录，为每个文件生成 FileEntry。

use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};

use crate::protocol::FileEntry;

/// 扫描本地共享目录，返回所有文件的 FileEntry 列表。
///
/// 只扫描目录下的直接文件（不递归子目录）。
/// SHA256 在每次调用时重新计算（不缓存到磁盘）。
pub fn scan_local_files(dir: &Path) -> anyhow::Result<Vec<FileEntry>> {
    let mut entries = Vec::new();

    if !dir.exists() {
        return Ok(entries);
    }

    let read_dir = match fs::read_dir(dir) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("无法读取共享目录 {}: {}", dir.display(), e);
            return Ok(entries);
        }
    };

    for entry in read_dir {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("读取目录项失败: {}", e);
                continue;
            }
        };

        let path = entry.path();

        // 只处理普通文件
        if !path.is_file() {
            continue;
        }

        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // 跳过隐藏文件
        if name.starts_with('.') {
            continue;
        }

        let metadata = match fs::metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("无法读取文件元数据 {}: {}", path.display(), e);
                continue;
            }
        };

        let size = metadata.len();

        let modified = match metadata.modified() {
            Ok(t) => {
                match t.duration_since(UNIX_EPOCH) {
                    Ok(d) => {
                        // 转换为 RFC3339 格式
                        let secs = d.as_secs();
                        // 用 chrono 格式化
                        match chrono::DateTime::from_timestamp(secs as i64, 0) {
                            Some(dt) => dt.to_rfc3339(),
                            None => String::new(),
                        }
                    }
                    Err(_) => String::new(),
                }
            }
            Err(_) => String::new(),
        };

        let sha256 = match compute_sha256(&path) {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!("无法计算文件哈希 {}: {}", path.display(), e);
                continue;
            }
        };

        entries.push(FileEntry {
            name,
            size,
            sha256,
            modified,
        });
    }

    // 按文件名排序，保证显示稳定
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    tracing::info!("扫描本地文件完成，共 {} 个文件", entries.len());
    Ok(entries)
}

/// 计算文件的 SHA256 哈希，返回 hex 字符串。
fn compute_sha256(path: &Path) -> anyhow::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];

    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    let hash = hasher.finalize();
    Ok(format!("{:x}", hash))
}
