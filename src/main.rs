mod app;
mod cli;
mod clipboard;
mod config;
mod key;
mod logging;
mod net;
mod storage;
mod terminal;
mod ui;

use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::EventStream;
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;

use crate::app::{App, Effect};
use crate::net::{NetCommand, NetEvent};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::parse()?;
    if cli.help {
        print!("{}", cli::help_text());
        return Ok(());
    }
    if cli.version {
        println!("lanchat {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let mut config = config::load(&cli)?;
    let _logging = logging::init(&config.paths.log_dir, cli.debug)?;
    tracing::debug!(
        node_id = %config.identity.node_id,
        name = %config.identity.name,
        "身份已加载"
    );
    let keymap = key::load(&cli, &config);

    let (history_error_tx, mut history_error_rx) = mpsc::unbounded_channel::<String>();
    let (history, messages, history_warnings) =
        storage::HistoryHandle::open(&config.history, &config.paths, history_error_tx)?;

    let mut network_error = None;
    let (net_handle, mut net_rx) = if config.network.enabled {
        match net::start(config.network.clone(), config.identity.clone()).await {
            Ok((handle, rx)) => (Some(handle), Some(rx)),
            Err(error) => {
                config.network.enabled = false;
                network_error = Some(error.to_string());
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    let mut warnings = config.warnings.clone();
    warnings.extend(keymap.warnings().iter().cloned());
    warnings.extend(history_warnings);
    if let Some(handle) = &net_handle {
        tracing::info!(port = handle.local_port(), "网络已启动");
    } else if cli.no_network {
        tracing::info!("网络已禁用（--no-network）");
    } else if let Some(error) = network_error {
        tracing::warn!(%error, "网络启动失败");
        warnings.push(format!("网络启动失败，已回退本地模式: {error}"));
    }

    let panic_log = config.paths.panic_log.clone();
    let mut app = App::new(config, keymap, history, messages, warnings);

    let _guard = terminal::TerminalGuard::enter()?;
    terminal::install_panic_hook(panic_log);
    let mut terminal = terminal::new_terminal()?;

    let mut events = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(200));
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        if app.dirty() {
            terminal.draw(|frame| ui::render(frame, &app))?;
            app.clear_dirty();
        }

        tokio::select! {
            event = events.next() => {
                match event {
                    Some(Ok(event)) => app.handle_terminal(event),
                    Some(Err(error)) => return Err(error.into()),
                    None => break,
                }
            }
            _ = tick.tick() => {
                app.on_tick(Instant::now());
            }
            error = history_error_rx.recv() => {
                if let Some(error) = error {
                    app.on_history_error(error);
                }
            }
            event = recv_net(&mut net_rx) => {
                if let Some(event) = event {
                    app.handle_net_event(event);
                }
            }
        }

        for effect in app.take_effects() {
            match effect {
                Effect::Persist(message) => app.persist_message(&message),
                Effect::Copy { text } => {
                    let result = app.copy_text(&text);
                    app.on_clipboard_result(result);
                }
                Effect::SendChat { msg_id, body } => {
                    if let Some(handle) = &net_handle {
                        let _ = handle.send(NetCommand::SendChat { msg_id, body }).await;
                    } else {
                        app.handle_net_event(NetEvent::SendFailed {
                            msg_id,
                            reason: "网络未启用".to_string(),
                        });
                    }
                }
                Effect::Reconnect => {
                    if let Some(handle) = &net_handle {
                        let _ = handle.send(NetCommand::Reconnect).await;
                    }
                }
                Effect::Disconnect => {
                    if let Some(handle) = &net_handle {
                        let _ = handle.send(NetCommand::Disconnect).await;
                    }
                }
            }
        }

        if app.should_quit() {
            break;
        }
    }

    if let Some(handle) = &net_handle {
        let _ = handle.send(NetCommand::Shutdown).await;
    }
    app.shutdown();
    Ok(())
}

async fn recv_net(rx: &mut Option<mpsc::Receiver<NetEvent>>) -> Option<NetEvent> {
    match rx {
        Some(rx) => rx.recv().await,
        None => std::future::pending().await,
    }
}
