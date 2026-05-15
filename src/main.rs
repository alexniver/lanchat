mod config;
mod protocol;
mod state;
mod tui;

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::panic;
use std::sync::{Arc, Mutex};

use chrono::Local;
use tokio::sync::mpsc;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::fmt::layer;
use tracing_subscriber::prelude::*;
use tracing_appender::non_blocking::WorkerGuard;

use crate::protocol::Message;
use crate::state::{AppState, NodeInfo};

/// 初始化日志系统：文件日志写入 log/ 文件夹，当前日期时间命名；同时输出到 stderr。
/// 返回的 `WorkerGuard` 必须在程序结束前保持存活，否则会丢日志。
fn init_logging() -> (WorkerGuard, WorkerGuard) {
    // 确保 log 目录存在
    std::fs::create_dir_all("log").ok();

    let now = Local::now();
    let log_file = format!("log/{}.log", now.format("%Y-%m-%d_%H-%M-%S"));
    let file = std::fs::File::create(&log_file).expect("failed to create log file");

    let (non_blocking, file_guard) = tracing_appender::non_blocking(file);

    // 文件层：INFO 及以上写入文件
    let file_layer = layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .with_filter(LevelFilter::INFO);

    // 控制台层：WARN 及以上输出到 stderr
    let (stderr_writer, stderr_guard) = tracing_appender::non_blocking(std::io::stderr());
    let stderr_layer = layer()
        .with_writer(stderr_writer)
        .with_ansi(true)
        .with_target(false)
        .with_filter(LevelFilter::WARN);

    tracing_subscriber::registry()
        .with(file_layer)
        .with(stderr_layer)
        .init();

    // 设置 panic hook，崩溃信息也写入日志
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("log/panic.log")
        {
            let now = Local::now();
            let _ = writeln!(
                f,
                "[{}] PANIC: {}",
                now.format("%Y-%m-%d %H:%M:%S%.3f"),
                info
            );
            if let Some(loc) = info.location() {
                let _ = writeln!(f, "  at {}:{}:{}", loc.file(), loc.line(), loc.column());
            }
            let backtrace = std::backtrace::Backtrace::force_capture();
            let _ = writeln!(f, "--- backtrace ---\n{backtrace}");
        }
        default_hook(info);
    }));

    tracing::info!("日志系统已初始化，日志文件: {}", log_file);

    (file_guard, stderr_guard)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Phase 2：初始化日志系统（guard 在 main 结束前一直存活）
    let (_file_guard, _stderr_guard) = init_logging();

    tracing::info!("lanchat 启动");

    // Phase 3：解析命令行参数 + 加载/创建 identity
    let app_config = config::parse_args();

    // Phase 3：确保本地文件共享目录存在
    let _share_dir = config::ensure_local_share_dir()?;

    let local_node = NodeInfo {
        node_id: app_config.identity.node_id.clone(),
        display_name: app_config.identity.display_name.clone(),
    };

    let app_state = Arc::new(Mutex::new(AppState {
        local_node,
        peers: HashMap::new(),
        messages: Vec::new(),
        seen_msg_ids: HashSet::new(),
        peer_files: HashMap::new(),
        local_files: Vec::new(),
        downloading: HashSet::new(),
    }));

    let (app_tx, ui_rx) = mpsc::channel::<Message>(256);

    tui::run(app_state, app_tx, ui_rx).await
}
