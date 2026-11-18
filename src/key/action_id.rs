use std::fmt;

use anyhow::{bail, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ActionId {
    OpenCompose,
    CursorDown,
    CursorUp,
    CopyBody,
    CopyLine,
    Quit,
    Noop,
}

impl ActionId {
    pub fn parse(value: &str) -> Result<Self> {
        Ok(match value {
            "open_compose" => Self::OpenCompose,
            "cursor_down" => Self::CursorDown,
            "cursor_up" => Self::CursorUp,
            "copy_body" => Self::CopyBody,
            "copy_line" => Self::CopyLine,
            "quit" => Self::Quit,
            "noop" => Self::Noop,
            other => bail!("未知 action: {other}"),
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenCompose => "open_compose",
            Self::CursorDown => "cursor_down",
            Self::CursorUp => "cursor_up",
            Self::CopyBody => "copy_body",
            Self::CopyLine => "copy_line",
            Self::Quit => "quit",
            Self::Noop => "noop",
        }
    }
}

impl fmt::Display for ActionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_action_names() {
        for action in [
            ActionId::OpenCompose,
            ActionId::CursorDown,
            ActionId::CursorUp,
            ActionId::CopyBody,
            ActionId::CopyLine,
            ActionId::Quit,
            ActionId::Noop,
        ] {
            assert_eq!(ActionId::parse(action.as_str()).unwrap(), action);
        }
    }

    #[test]
    fn rejects_unknown_action() {
        assert!(ActionId::parse("bogus").is_err());
    }
}
