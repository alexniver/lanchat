mod app;
mod cli;
mod clipboard;
mod config;
mod key;
mod logging;
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

    let config = config::load(&cli)?;
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

    let mut warnings = config.warnings.clone();
    warnings.extend(keymap.warnings().iter().cloned());
    warnings.extend(history_warnings);

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
        }

        for effect in app.take_effects() {
            match effect {
                Effect::Persist(message) => app.persist_message(&message),
                Effect::Copy { text } => {
                    let result = app.copy_text(&text);
                    app.on_clipboard_result(result);
                }
            }
        }

        if app.should_quit() {
            break;
        }
    }

    app.shutdown();
    Ok(())
}
