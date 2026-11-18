use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::app::message::Message;
use crate::config::{HistoryConfig, Paths};

const HISTORY_PREFIX: &str = "lobby-";
const HISTORY_SUFFIX: &str = ".jsonl";

enum Command {
    Append(Message),
    Shutdown(Sender<()>),
}

pub struct HistoryHandle {
    tx: Option<Sender<Command>>,
    join: Option<JoinHandle<()>>,
}

impl HistoryHandle {
    pub fn open(
        config: &HistoryConfig,
        paths: &Paths,
        error_tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<(Self, Vec<Message>, Vec<String>)> {
        if !config.enabled {
            return Ok((Self::disabled(), Vec::new(), Vec::new()));
        }
        let (messages, warnings) = load_recent(&paths.history_dir, config.max_messages)?;
        let dir = paths.history_dir.clone();
        let max_bytes = config.max_file_mb.saturating_mul(1024 * 1024);
        let (tx, rx) = mpsc::channel();
        let join = thread::Builder::new()
            .name("lanchat-history".to_string())
            .spawn(move || writer_loop(dir, max_bytes, rx, error_tx))?;
        Ok((
            Self {
                tx: Some(tx),
                join: Some(join),
            },
            messages,
            warnings,
        ))
    }

    pub fn disabled() -> Self {
        Self {
            tx: None,
            join: None,
        }
    }

    pub fn append(&self, message: &Message) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::Append(message.clone()));
        }
    }

    pub fn shutdown(&mut self) {
        let Some(tx) = self.tx.take() else {
            return;
        };
        let (ack_tx, ack_rx) = mpsc::channel();
        let _ = tx.send(Command::Shutdown(ack_tx));
        let _ = ack_rx.recv_timeout(Duration::from_secs(1));
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for HistoryHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn writer_loop(
    dir: PathBuf,
    max_bytes: u64,
    rx: Receiver<Command>,
    error_tx: tokio::sync::mpsc::UnboundedSender<String>,
) {
    while let Ok(command) = rx.recv() {
        match command {
            Command::Append(message) => {
                if let Err(error) = append_sync(&dir, max_bytes, &message) {
                    let _ = error_tx.send(format!("历史写入失败: {error}"));
                }
            }
            Command::Shutdown(ack) => {
                let _ = ack.send(());
                break;
            }
        }
    }
}

pub fn load_recent(dir: &Path, max_messages: usize) -> Result<(Vec<Message>, Vec<String>)> {
    if !dir.exists() {
        return Ok((Vec::new(), Vec::new()));
    }
    let mut files = Vec::new();
    for entry in
        fs::read_dir(dir).with_context(|| format!("读取历史目录失败: {}", dir.display()))?
    {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(HISTORY_PREFIX) && name.ends_with(HISTORY_SUFFIX) {
            files.push(entry.path());
        }
    }
    files.sort();

    let mut messages = Vec::new();
    let mut warnings = Vec::new();
    for path in files {
        let file = match fs::File::open(&path) {
            Ok(file) => file,
            Err(error) => {
                warnings.push(format!("无法读取历史 {}: {error}", path.display()));
                continue;
            }
        };
        let reader = BufReader::new(file);
        for (line_no, line) in reader.lines().enumerate() {
            let line = match line {
                Ok(line) => line,
                Err(error) => {
                    warnings.push(format!(
                        "历史读取失败 {}:{}: {error}",
                        path.display(),
                        line_no + 1
                    ));
                    continue;
                }
            };
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<HistoryRecord>(&line) {
                Ok(record) => {
                    if let Some(message) = record.into_message() {
                        messages.push(message);
                    } else {
                        warnings.push(format!(
                            "历史记录缺少字段 {}:{}",
                            path.display(),
                            line_no + 1
                        ));
                    }
                }
                Err(error) => {
                    warnings.push(format!(
                        "跳过损坏历史行 {}:{}: {error}",
                        path.display(),
                        line_no + 1
                    ));
                }
            }
        }
    }
    if messages.len() > max_messages {
        messages.drain(..messages.len() - max_messages);
    }
    Ok((messages, warnings))
}

fn append_sync(dir: &Path, max_bytes: u64, message: &Message) -> Result<()> {
    fs::create_dir_all(dir)?;
    set_dir_mode(dir, 0o700);
    let month = Local::now().format("%Y-%m");
    let path = dir.join(format!("{HISTORY_PREFIX}{month}{HISTORY_SUFFIX}"));
    rotate_if_needed(&path, max_bytes)?;

    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .with_context(|| format!("打开历史文件失败: {}", path.display()))?;
    set_file_mode(&path, 0o600);
    let record = HistoryRecord {
        v: 1,
        id: message.id.clone(),
        room: "lobby".to_string(),
        from: message.from.clone(),
        name: message.name.clone(),
        timestamp: message.timestamp.clone(),
        sent_at: message.sent_at.clone(),
        body: message.body.clone(),
    };
    let line = serde_json::to_string(&record)?;
    writeln!(file, "{line}")?;
    file.flush()?;
    Ok(())
}

fn rotate_if_needed(path: &Path, max_bytes: u64) -> Result<()> {
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(());
    };
    if max_bytes == 0 || metadata.len() <= max_bytes {
        return Ok(());
    }
    let stem = path
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "lobby".to_string());
    let backup = path.with_file_name(format!("{stem}.1{HISTORY_SUFFIX}"));
    let _ = fs::remove_file(&backup);
    fs::rename(path, backup)?;
    Ok(())
}

#[cfg(unix)]
fn set_dir_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
}

#[cfg(not(unix))]
fn set_dir_mode(_path: &Path, _mode: u32) {}

#[cfg(unix)]
fn set_file_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
}

#[cfg(not(unix))]
fn set_file_mode(_path: &Path, _mode: u32) {}

#[derive(Debug, Serialize, Deserialize)]
struct HistoryRecord {
    #[serde(default = "default_version")]
    v: u8,
    #[serde(default)]
    id: String,
    #[serde(default = "default_room")]
    room: String,
    #[serde(default)]
    from: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    timestamp: String,
    #[serde(default)]
    sent_at: String,
    #[serde(default)]
    body: String,
}

impl HistoryRecord {
    fn into_message(self) -> Option<Message> {
        if self.id.is_empty() || self.timestamp.is_empty() || self.name.is_empty() {
            return None;
        }
        Some(Message {
            id: self.id,
            from: self.from,
            name: self.name,
            timestamp: self.timestamp,
            sent_at: self.sent_at,
            body: self.body,
        })
    }
}

fn default_version() -> u8 {
    1
}

fn default_room() -> String {
    "lobby".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(body: &str) -> Message {
        Message::new("alice@arch", body)
    }

    #[test]
    fn appends_and_loads_messages() {
        let dir = tempfile::tempdir().unwrap();
        append_sync(dir.path(), 10 * 1024 * 1024, &message("one")).unwrap();
        append_sync(dir.path(), 10 * 1024 * 1024, &message("two")).unwrap();
        let (messages, warnings) = load_recent(dir.path(), 100).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].body, "one");
        assert_eq!(messages[1].body, "two");
    }

    #[test]
    fn skips_corrupt_lines() {
        let dir = tempfile::tempdir().unwrap();
        append_sync(dir.path(), 10 * 1024 * 1024, &message("ok")).unwrap();
        let path = fs::read_dir(dir.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let mut file = OpenOptions::new().append(true).open(path).unwrap();
        writeln!(file, "{{not json}}").unwrap();
        let (messages, warnings) = load_recent(dir.path(), 100).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].body, "ok");
        assert!(warnings.iter().any(|warning| warning.contains("损坏")));
    }

    #[test]
    fn caps_loaded_messages() {
        let dir = tempfile::tempdir().unwrap();
        for index in 0..10 {
            append_sync(dir.path(), 10 * 1024 * 1024, &message(&index.to_string())).unwrap();
        }
        let (messages, _) = load_recent(dir.path(), 3).unwrap();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].body, "7");
        assert_eq!(messages[2].body, "9");
    }

    #[test]
    fn writer_thread_appends_and_shuts_down() {
        use crate::config::Paths;
        use tokio::sync::mpsc::unbounded_channel;

        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_dir: dir.path().join("config"),
            config_file: dir.path().join("config/config.toml"),
            keymap_file: dir.path().join("config/keymap.toml"),
            identity_file: dir.path().join("config/identity.json"),
            data_dir: dir.path().join("data"),
            history_dir: dir.path().join("data/history"),
            state_dir: dir.path().join("state"),
            log_dir: dir.path().join("state/logs"),
            panic_log: dir.path().join("state/panic.log"),
        };
        let config = HistoryConfig {
            enabled: true,
            max_messages: 100,
            max_file_mb: 10,
        };
        let (error_tx, _error_rx) = unbounded_channel();
        let (mut history, messages, warnings) =
            HistoryHandle::open(&config, &paths, error_tx).unwrap();
        assert!(messages.is_empty());
        assert!(warnings.is_empty());
        history.append(&message("thread"));
        history.shutdown();

        let (loaded, _) = load_recent(&paths.history_dir, 100).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].body, "thread");
    }

    #[test]
    fn persists_remote_source_and_sent_at() {
        let dir = tempfile::tempdir().unwrap();
        let mut remote = message("remote");
        remote.from = "peer-1".into();
        remote.sent_at = "2026-11-19T10:30:00+08:00".into();
        append_sync(dir.path(), 10 * 1024 * 1024, &remote).unwrap();
        let (loaded, _) = load_recent(dir.path(), 100).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].from, "peer-1");
        assert_eq!(loaded[0].sent_at, "2026-11-19T10:30:00+08:00");
    }

    #[cfg(unix)]
    #[test]
    fn history_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        append_sync(dir.path(), 10 * 1024 * 1024, &message("secret")).unwrap();
        let path = fs::read_dir(dir.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let mode = fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
