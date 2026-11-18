use std::io::Write;

use anyhow::{bail, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;

const MAX_COPY_BYTES: usize = 100 * 1024;

/// Keeps the platform clipboard alive so X11 clipboard ownership is not lost
/// immediately after `set_text`. Falls back to OSC 52 for SSH/headless use.
#[derive(Default)]
pub struct Clipboard {
    inner: Option<arboard::Clipboard>,
}

impl Clipboard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn copy(&mut self, text: &str) -> Result<()> {
        if text.len() > MAX_COPY_BYTES {
            bail!("复制内容过大（{} 字节）", text.len());
        }
        if self.inner.is_none() {
            self.inner = arboard::Clipboard::new().ok();
        }
        if let Some(clipboard) = &mut self.inner {
            if clipboard.set_text(text.to_string()).is_ok() {
                return Ok(());
            }
            self.inner = None;
        }
        write_osc52(text)
    }
}

fn write_osc52(text: &str) -> Result<()> {
    let sequence = osc52_sequence(text);
    let mut stdout = std::io::stdout();
    stdout.write_all(sequence.as_bytes())?;
    stdout.flush()?;
    Ok(())
}

pub fn osc52_sequence(text: &str) -> String {
    let encoded = STANDARD.encode(text.as_bytes());
    format!("\x1b]52;c;{encoded}\x07")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_osc52_sequence() {
        assert_eq!(osc52_sequence("hi"), "\x1b]52;c;aGk=\x07");
    }

    #[test]
    fn rejects_oversized_copy() {
        let text = "x".repeat(MAX_COPY_BYTES + 1);
        assert!(Clipboard::new().copy(&text).is_err());
    }
}
