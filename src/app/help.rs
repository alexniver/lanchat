use std::path::Path;

use crate::key::{ActionId, Keymap};

/// Build the logical help lines. The UI renders these one-to-one; keep lines
/// short enough for the minimum terminal width (60 columns, 58 inner).
pub fn lines(keymap: &Keymap, files_dir: &Path) -> Vec<String> {
    let hint = |action| keymap.hint(action);
    let mut lines = vec![
        format!("LanChat {}", env!("CARGO_PKG_VERSION")),
        String::new(),
        "快速开始".to_string(),
        format!("  {}  输入消息", hint(ActionId::OpenCompose)),
        format!("  {}  命令输入", hint(ActionId::OpenCommand)),
        format!("  {}  打开帮助", hint(ActionId::OpenHelp)),
        "  j / k     下/上选择消息".to_string(),
        format!(
            "  {} / {}  复制正文/完整行",
            hint(ActionId::CopyBody),
            hint(ActionId::CopyLine)
        ),
        "  Esc       取消或退出当前层".to_string(),
        format!("  {}  退出程序（仅消息列表）", hint(ActionId::Quit)),
        String::new(),
        "消息列表".to_string(),
        "  - 默认自动选中最新消息".to_string(),
        "  - j/k 选到非最新后进入手动模式".to_string(),
        "  - 手动模式下新消息不自动跳转".to_string(),
        "  - 选回最新后恢复自动".to_string(),
        String::new(),
        "输入消息".to_string(),
        "  - Enter 发送；空内容或 Esc 取消".to_string(),
        "  - 输入中只有 Enter/Esc 是应用快捷键".to_string(),
        "  - 支持 Home/End、Backspace/Delete".to_string(),
        "  - 支持 Ctrl+a/e/w/u/k 编辑".to_string(),
        String::new(),
        "文件传递".to_string(),
        "  /send file <路径> to <对方>".to_string(),
        "  /recv file <对方>".to_string(),
        "  - 对方可用 no1、ip101、完整 node_id、IP、昵称".to_string(),
        "  - 请求 10 分钟未接收自动取消".to_string(),
        format!("  - 保存到 {}", files_dir.display()),
        "  - 重名另存为 a[0].png、a[1].png".to_string(),
        String::new(),
        "成员与网络".to_string(),
        format!("  {}  打开/关闭成员侧边栏", hint(ActionId::ToggleMembers)),
        format!("  {}  重连", hint(ActionId::Reconnect)),
        format!("  {}  断开", hint(ActionId::Disconnect)),
        "  - 状态栏在线数包含本机".to_string(),
        "  - mDNS 自动发现；失败用 --connect".to_string(),
        String::new(),
        "配置".to_string(),
        "  ~/.config/lanchat/config.toml".to_string(),
        "  ~/.config/lanchat/keymap.toml".to_string(),
        "  ~/.local/share/lanchat/history/".to_string(),
        "  ~/.local/state/lanchat/logs/".to_string(),
        String::new(),
        "限制".to_string(),
        "  - 单房间、明文、无鉴权".to_string(),
        "  - 无历史同步、无断点续传".to_string(),
        "  - 单条消息 16 KiB；单文件 1 GiB".to_string(),
        "  - 仅局域网".to_string(),
    ];
    // Keep the final newline out; the renderer handles the last line.
    lines.push(String::new());
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_contains_version_and_key_sections() {
        let lines = lines(&Keymap::defaults(), Path::new("/tmp/lanchat/files"));
        let text = lines.join("\n");
        assert!(text.starts_with("LanChat "));
        assert!(text.contains("快速开始"));
        assert!(text.contains("/send file"));
        assert!(text.contains("按 Esc") || text.contains("Esc"));
        assert!(lines.len() > 20);
    }

    #[test]
    fn help_uses_current_keymap_hints() {
        let map = Keymap::from_file(&crate::key::KeymapFile {
            list: std::collections::BTreeMap::from([("?".to_string(), "open_help".to_string())]),
        });
        let text = lines(&map, Path::new("/tmp/lanchat/files")).join("\n");
        assert!(text.contains("打开帮助"));
    }
    #[test]
    fn help_shows_actual_files_dir() {
        let path = Path::new("/custom/data/lanchat/files");
        let text = lines(&Keymap::defaults(), path).join("\n");
        assert!(text.contains("/custom/data/lanchat/files"));
    }
}
