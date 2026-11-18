use std::fs::OpenOptions;
use std::io::{self, Write};
use std::panic;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use crossterm::cursor::Show;
use crossterm::event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

static RESTORED: AtomicBool = AtomicBool::new(true);
static PANIC_HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

pub struct TerminalGuard {
    active: bool,
}

impl TerminalGuard {
    pub fn enter() -> Result<Self> {
        enable_raw_mode()?;
        RESTORED.store(false, Ordering::SeqCst);
        let mut stdout = io::stdout();
        if let Err(error) = execute!(
            stdout,
            EnterAlternateScreen,
            EnableBracketedPaste,
            DisableMouseCapture
        ) {
            let _ = disable_raw_mode();
            RESTORED.store(true, Ordering::SeqCst);
            return Err(error.into());
        }
        Ok(Self { active: true })
    }

    pub fn restore() {
        if RESTORED.swap(true, Ordering::SeqCst) {
            return;
        }
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen, DisableBracketedPaste, Show);
        let _ = stdout.flush();
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.active {
            Self::restore();
            self.active = false;
        }
    }
}

pub fn new_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    let backend = CrosstermBackend::new(io::stdout());
    Ok(Terminal::new(backend)?)
}

pub fn install_panic_hook(panic_log: PathBuf) {
    if PANIC_HOOK_INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        TerminalGuard::restore();
        write_panic(&panic_log, info);
        default_hook(info);
    }));
}

fn write_panic(path: &Path, info: &panic::PanicHookInfo<'_>) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let now = chrono::Local::now();
        let _ = writeln!(
            file,
            "[{}] PANIC: {info}",
            now.format("%Y-%m-%d %H:%M:%S%.3f")
        );
        if let Some(location) = info.location() {
            let _ = writeln!(
                file,
                "  at {}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            );
        }
        let backtrace = std::backtrace::Backtrace::force_capture();
        let _ = writeln!(file, "--- backtrace ---\n{backtrace}");
    }
}
