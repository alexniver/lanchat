use std::fmt;

use anyhow::{bail, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Key {
    Char(char),
    Ctrl(char),
    Alt(char),
    Esc,
    Enter,
    Tab,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Space,
    F(u8),
}

impl Key {
    pub fn from_event(event: KeyEvent) -> Option<Self> {
        if event.kind == KeyEventKind::Release {
            return None;
        }
        let code = match event.code {
            KeyCode::Char(ch) => {
                if event.modifiers.contains(KeyModifiers::CONTROL) {
                    return Some(Key::Ctrl(ch.to_ascii_lowercase()));
                }
                if event.modifiers.contains(KeyModifiers::ALT) {
                    return Some(Key::Alt(ch));
                }
                if ch == ' ' {
                    Key::Space
                } else {
                    Key::Char(ch)
                }
            }
            KeyCode::Esc => Key::Esc,
            KeyCode::Enter => Key::Enter,
            KeyCode::Tab => Key::Tab,
            KeyCode::Backspace => Key::Backspace,
            KeyCode::Delete => Key::Delete,
            KeyCode::Left => Key::Left,
            KeyCode::Right => Key::Right,
            KeyCode::Up => Key::Up,
            KeyCode::Down => Key::Down,
            KeyCode::Home => Key::Home,
            KeyCode::End => Key::End,
            KeyCode::PageUp => Key::PageUp,
            KeyCode::PageDown => Key::PageDown,
            KeyCode::F(n) => Key::F(n),
            _ => return None,
        };
        Some(code)
    }

    pub fn parse(token: &str) -> Result<Self> {
        let token = token.trim();
        if token.is_empty() {
            bail!("空按键");
        }
        let lower = token.to_ascii_lowercase();
        if let Some(special) = special_key(&lower) {
            return Ok(special);
        }
        if token.chars().count() == 1 && !token.contains('+') {
            let ch = token.chars().next().unwrap();
            return Ok(if ch == ' ' { Key::Space } else { Key::Char(ch) });
        }

        let parts: Vec<&str> = lower.split('+').collect();
        if parts.len() < 2 {
            bail!("无法识别的按键: {token}");
        }
        let key_part = parts.last().copied().unwrap();
        let mut ctrl = false;
        let mut alt = false;
        let mut shift = false;
        for modifier in &parts[..parts.len() - 1] {
            match *modifier {
                "ctrl" | "control" => ctrl = true,
                "alt" | "meta" => alt = true,
                "shift" => shift = true,
                other => bail!("未知修饰键: {other}"),
            }
        }

        if let Some(special) = special_key(key_part) {
            if ctrl || alt {
                bail!("ctrl/alt 暂不支持组合特殊键: {token}");
            }
            return Ok(special);
        }
        if key_part.chars().count() != 1 {
            bail!("无法识别的按键: {token}");
        }
        let mut ch = key_part.chars().next().unwrap();
        if shift {
            ch = ch.to_ascii_uppercase();
        }
        if ctrl {
            return Ok(Key::Ctrl(ch.to_ascii_lowercase()));
        }
        if alt {
            return Ok(Key::Alt(ch));
        }
        Ok(if ch == ' ' { Key::Space } else { Key::Char(ch) })
    }

    pub fn parse_sequence(text: &str) -> Result<Vec<Key>> {
        let mut keys = Vec::new();
        for token in text.split_whitespace() {
            keys.push(Key::parse(token)?);
        }
        if keys.is_empty() {
            bail!("空按键序列");
        }
        Ok(keys)
    }

    pub fn is_reserved_list_key(self) -> bool {
        matches!(self, Key::Char('Q'))
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Key::Char(' ') | Key::Space => write!(f, "space"),
            Key::Char(ch) => write!(f, "{ch}"),
            Key::Ctrl(ch) => write!(f, "ctrl+{ch}"),
            Key::Alt(ch) => write!(f, "alt+{ch}"),
            Key::Esc => write!(f, "esc"),
            Key::Enter => write!(f, "enter"),
            Key::Tab => write!(f, "tab"),
            Key::Backspace => write!(f, "backspace"),
            Key::Delete => write!(f, "delete"),
            Key::Left => write!(f, "left"),
            Key::Right => write!(f, "right"),
            Key::Up => write!(f, "up"),
            Key::Down => write!(f, "down"),
            Key::Home => write!(f, "home"),
            Key::End => write!(f, "end"),
            Key::PageUp => write!(f, "pageup"),
            Key::PageDown => write!(f, "pagedown"),
            Key::F(n) => write!(f, "f{n}"),
        }
    }
}

fn special_key(name: &str) -> Option<Key> {
    Some(match name {
        "esc" | "escape" => Key::Esc,
        "enter" | "return" => Key::Enter,
        "tab" => Key::Tab,
        "backspace" => Key::Backspace,
        "delete" | "del" => Key::Delete,
        "left" => Key::Left,
        "right" => Key::Right,
        "up" => Key::Up,
        "down" => Key::Down,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" | "page-up" | "pgup" => Key::PageUp,
        "pagedown" | "page-down" | "pgdn" => Key::PageDown,
        "space" => Key::Space,
        "f1" => Key::F(1),
        "f2" => Key::F(2),
        "f3" => Key::F(3),
        "f4" => Key::F(4),
        "f5" => Key::F(5),
        "f6" => Key::F(6),
        "f7" => Key::F(7),
        "f8" => Key::F(8),
        "f9" => Key::F(9),
        "f10" => Key::F(10),
        "f11" => Key::F(11),
        "f12" => Key::F(12),
        _ => return None,
    })
}

pub type KeySequence = Vec<Key>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_keys_and_sequences() {
        assert_eq!(Key::parse("j").unwrap(), Key::Char('j'));
        assert_eq!(Key::parse("Q").unwrap(), Key::Char('Q'));
        assert_eq!(Key::parse("space").unwrap(), Key::Space);
        assert_eq!(Key::parse("ctrl+d").unwrap(), Key::Ctrl('d'));
        assert_eq!(Key::parse("esc").unwrap(), Key::Esc);
        assert_eq!(
            Key::parse_sequence("g g").unwrap(),
            vec![Key::Char('g'), Key::Char('g')]
        );
    }

    #[test]
    fn rejects_bad_keys() {
        assert!(Key::parse("").is_err());
        assert!(Key::parse("ctrl+f13").is_err());
        assert!(Key::parse("nope").is_err());
    }

    #[test]
    fn display_roundtrips_common_keys() {
        assert_eq!(Key::Char('Q').to_string(), "Q");
        assert_eq!(Key::Space.to_string(), "space");
        assert_eq!(Key::Ctrl('d').to_string(), "ctrl+d");
    }
}
