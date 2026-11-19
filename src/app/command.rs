use std::path::PathBuf;

/// Result of parsing a Compose submission that starts with `/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedCommand {
    SendFile {
        path: String,
        peer: String,
    },
    RecvFile {
        peer: String,
    },
    SendFolder {
        path: String,
        peer: String,
    },
    RecvFolder {
        peer: String,
    },
    SetName {
        name: String,
    },
    /// Not a registered command: send the original text as a normal message.
    Unknown,
    /// A registered command with bad arguments: show usage, do not send.
    Invalid {
        usage: &'static str,
    },
}

pub const SEND_USAGE: &str = "用法：/sendfile <文件路径> to <对方>";
pub const RECV_USAGE: &str = "用法：/recvfile <对方>";
pub const SEND_FOLDER_USAGE: &str = "用法：/sendfolder <目录> to <对方>";
pub const RECV_FOLDER_USAGE: &str = "用法：/recvfolder <对方>";
pub const SETNAME_USAGE: &str = "用法：/setname <新名字>";

pub fn parse(input: &str) -> ParsedCommand {
    let text = input.trim();
    if !text.starts_with('/') {
        return ParsedCommand::Unknown;
    }
    if text == "/sendfile" || text.starts_with("/sendfile ") {
        return parse_send_file(text, "/sendfile ");
    }
    if text == "/recvfile" || text.starts_with("/recvfile ") {
        return parse_recv_file(text, "/recvfile ");
    }
    if text == "/sendfolder" || text.starts_with("/sendfolder ") {
        return parse_folder_path(text, "/sendfolder ", SEND_FOLDER_USAGE, true);
    }
    if text == "/recvfolder" || text.starts_with("/recvfolder ") {
        return parse_folder_path(text, "/recvfolder ", RECV_FOLDER_USAGE, false);
    }
    if text == "/recv folder" || text.starts_with("/recv folder ") {
        return parse_folder_path(text, "/recv folder ", RECV_FOLDER_USAGE, false);
    }
    // 0.1.7 compatibility aliases; not shown in help.
    if text == "/send" || text.starts_with("/send ") {
        return parse_send_file(text, "/send file ");
    }
    if text == "/recv" || text.starts_with("/recv ") {
        return parse_recv_file(text, "/recv file ");
    }
    if text == "/setname" || text.starts_with("/setname ") {
        return parse_setname(text, "/setname ");
    }
    // 0.1.7 compatibility alias; not shown in help.
    if text == "/set name" || text.starts_with("/set name ") {
        return parse_setname(text, "/set name ");
    }
    ParsedCommand::Unknown
}

fn parse_send_file(text: &str, prefix: &str) -> ParsedCommand {
    let Some(rest) = text.strip_prefix(prefix) else {
        return ParsedCommand::Invalid { usage: SEND_USAGE };
    };
    let rest = rest.trim();
    let Some(index) = rest.rfind(" to ") else {
        return ParsedCommand::Invalid { usage: SEND_USAGE };
    };
    let path = strip_quotes(rest[..index].trim());
    let peer = rest[index + 4..].trim();
    if path.is_empty() || peer.is_empty() {
        return ParsedCommand::Invalid { usage: SEND_USAGE };
    }
    ParsedCommand::SendFile {
        path: path.to_string(),
        peer: peer.to_string(),
    }
}

fn parse_recv_file(text: &str, prefix: &str) -> ParsedCommand {
    let Some(rest) = text.strip_prefix(prefix) else {
        return ParsedCommand::Invalid { usage: RECV_USAGE };
    };
    let peer = strip_quotes(rest.trim());
    if peer.is_empty() || peer.split_whitespace().count() != 1 {
        return ParsedCommand::Invalid { usage: RECV_USAGE };
    }
    ParsedCommand::RecvFile {
        peer: peer.to_string(),
    }
}

fn parse_folder_path(
    text: &str,
    prefix: &str,
    usage: &'static str,
    sending: bool,
) -> ParsedCommand {
    let Some(rest) = text.strip_prefix(prefix) else {
        return ParsedCommand::Invalid { usage };
    };
    let rest = rest.trim();
    if sending {
        let Some(index) = rest.rfind(" to ") else {
            return ParsedCommand::Invalid { usage };
        };
        let path = strip_quotes(rest[..index].trim());
        let peer = rest[index + 4..].trim();
        if path.is_empty() || peer.is_empty() {
            ParsedCommand::Invalid { usage }
        } else {
            ParsedCommand::SendFolder {
                path: path.to_string(),
                peer: peer.to_string(),
            }
        }
    } else {
        let peer = strip_quotes(rest);
        if peer.is_empty() || peer.split_whitespace().count() != 1 {
            ParsedCommand::Invalid { usage }
        } else {
            ParsedCommand::RecvFolder {
                peer: peer.to_string(),
            }
        }
    }
}

fn parse_setname(text: &str, prefix: &str) -> ParsedCommand {
    let Some(rest) = text.strip_prefix(prefix) else {
        return ParsedCommand::Invalid {
            usage: SETNAME_USAGE,
        };
    };
    let name = strip_quotes(rest.trim());
    if name.is_empty() {
        return ParsedCommand::Invalid {
            usage: SETNAME_USAGE,
        };
    }
    ParsedCommand::SetName {
        name: name.to_string(),
    }
}

fn strip_quotes(value: &str) -> &str {
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

/// Expand a leading `~` and make relative paths absolute.
pub fn resolve_path(raw: &str) -> Result<PathBuf, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("文件路径不能为空".to_string());
    }
    let expanded = if raw == "~" {
        home_dir()?
    } else if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\")) {
        home_dir()?.join(rest)
    } else {
        PathBuf::from(raw)
    };
    let absolute = if expanded.is_absolute() {
        expanded
    } else {
        std::env::current_dir()
            .map_err(|error| format!("无法读取当前目录：{error}"))?
            .join(expanded)
    };
    dunce::canonicalize(&absolute).map_err(|error| format!("找不到文件 {raw}：{error}"))
}

fn home_dir() -> Result<PathBuf, String> {
    dirs::home_dir()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| "无法确定用户主目录".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_send_file() {
        assert_eq!(
            parse("/sendfile ~/Download/a.png to 103"),
            ParsedCommand::SendFile {
                path: "~/Download/a.png".into(),
                peer: "103".into()
            }
        );
        assert_eq!(
            parse("/sendfile \"./my photo.png\" to 192.168.1.103"),
            ParsedCommand::SendFile {
                path: "./my photo.png".into(),
                peer: "192.168.1.103".into()
            }
        );
        assert_eq!(
            parse("/sendfile a to b to 101"),
            ParsedCommand::SendFile {
                path: "a to b".into(),
                peer: "101".into()
            }
        );
    }

    #[test]
    fn parses_recv_file() {
        assert_eq!(
            parse("/recvfile 101"),
            ParsedCommand::RecvFile { peer: "101".into() }
        );
    }

    #[test]
    fn parses_legacy_set_name_alias() {
        assert_eq!(
            parse("/set name alice"),
            ParsedCommand::SetName {
                name: "alice".into()
            }
        );
        assert!(matches!(
            parse("/set name"),
            ParsedCommand::Invalid {
                usage: SETNAME_USAGE
            }
        ));
    }

    #[test]
    fn parses_folder_commands() {
        assert_eq!(
            parse("/sendfolder ~/Photos to no1"),
            ParsedCommand::SendFolder {
                path: "~/Photos".into(),
                peer: "no1".into()
            }
        );
        assert_eq!(
            parse("/recvfolder no1"),
            ParsedCommand::RecvFolder { peer: "no1".into() }
        );
        assert_eq!(
            parse("/recv folder no1"),
            ParsedCommand::RecvFolder { peer: "no1".into() }
        );
        assert!(matches!(
            parse("/sendfolder ~/Photos"),
            ParsedCommand::Invalid {
                usage: SEND_FOLDER_USAGE
            }
        ));
    }

    #[test]
    fn parses_setname() {
        assert_eq!(
            parse("/setname alice"),
            ParsedCommand::SetName {
                name: "alice".into()
            }
        );
        assert_eq!(
            parse("/setname 张三"),
            ParsedCommand::SetName {
                name: "张三".into()
            }
        );
        assert!(matches!(
            parse("/setname"),
            ParsedCommand::Invalid {
                usage: SETNAME_USAGE
            }
        ));
    }

    #[test]
    fn legacy_commands_remain_aliases() {
        assert_eq!(
            parse("/send file ~/a.png to no1"),
            ParsedCommand::SendFile {
                path: "~/a.png".into(),
                peer: "no1".into()
            }
        );
        assert_eq!(
            parse("/recv file no1"),
            ParsedCommand::RecvFile { peer: "no1".into() }
        );
    }

    #[test]
    fn unknown_commands_fall_back_to_chat() {
        assert_eq!(parse("/hello"), ParsedCommand::Unknown);
        assert_eq!(parse("/"), ParsedCommand::Unknown);
        assert_eq!(parse("/sendfilex x"), ParsedCommand::Unknown);
    }

    #[test]
    fn malformed_registered_commands_are_invalid() {
        assert!(matches!(
            parse("/sendfile"),
            ParsedCommand::Invalid { usage: SEND_USAGE }
        ));
        assert!(matches!(
            parse("/sendfile a.png"),
            ParsedCommand::Invalid { usage: SEND_USAGE }
        ));
        assert!(matches!(
            parse("/recvfile"),
            ParsedCommand::Invalid { usage: RECV_USAGE }
        ));
        assert!(matches!(
            parse("/recvfile 101 102"),
            ParsedCommand::Invalid { usage: RECV_USAGE }
        ));
    }

    #[test]
    fn resolves_absolute_paths() {
        let path = resolve_path(".").unwrap();
        assert!(path.is_absolute());
        assert!(path.is_dir());
    }
    #[test]
    fn expands_tilde_with_home_dir() {
        let home = dirs::home_dir().expect("home dir");
        let canonical = dunce::canonicalize(&home).unwrap();
        assert_eq!(resolve_path("~").unwrap(), canonical);
        assert_eq!(resolve_path("~/").unwrap(), canonical);
        assert_eq!(resolve_path("~\\").unwrap(), canonical);
    }
}
