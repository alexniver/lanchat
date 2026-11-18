use std::path::Path;

use anyhow::Result;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::fmt::layer;
use tracing_subscriber::prelude::*;

pub struct LoggingGuard {
    _guard: WorkerGuard,
}

pub fn init(log_dir: &Path, debug: bool) -> Result<LoggingGuard> {
    std::fs::create_dir_all(log_dir)?;
    let appender = tracing_appender::rolling::daily(log_dir, "lanchat.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(appender);

    let default_level = if debug { "debug" } else { "info" };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

    let file_layer = layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .with_filter(filter);

    tracing_subscriber::registry().with(file_layer).init();
    tracing::info!(?log_dir, "日志系统已初始化");
    Ok(LoggingGuard { _guard: guard })
}
