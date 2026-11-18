use std::fmt;

use anyhow::{bail, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ActionId {
    OpenCompose,
    OpenCommand,
    CursorDown,
    CursorUp,
    CopyBody,
    CopyLine,
    ToggleMembers,
    Reconnect,
    Disconnect,
    Quit,
    Noop,
}

impl ActionId {
    pub fn parse(value: &str) -> Result<Self> {
        Ok(match value {
            "open_compose" => Self::OpenCompose,
            "open_command" => Self::OpenCommand,
            "cursor_down" => Self::CursorDown,
            "cursor_up" => Self::CursorUp,
            "copy_body" => Self::CopyBody,
            "copy_line" => Self::CopyLine,
            "toggle_members" => Self::ToggleMembers,
            "reconnect" => Self::Reconnect,
            "disconnect" => Self::Disconnect,
            "quit" => Self::Quit,
            "noop" => Self::Noop,
            other => bail!("未知 action: {other}"),
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenCompose => "open_compose",
            Self::OpenCommand => "open_command",
            Self::CursorDown => "cursor_down",
            Self::CursorUp => "cursor_up",
            Self::CopyBody => "copy_body",
            Self::CopyLine => "copy_line",
            Self::ToggleMembers => "toggle_members",
            Self::Reconnect => "reconnect",
            Self::Disconnect => "disconnect",
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
            ActionId::OpenCommand,
            ActionId::CursorDown,
            ActionId::CursorUp,
            ActionId::CopyBody,
            ActionId::CopyLine,
            ActionId::ToggleMembers,
            ActionId::Reconnect,
            ActionId::Disconnect,
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
