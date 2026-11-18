# LanChat 0.1.0

> Helix-inspired、键位可自定义的单机终端聊天工具。
>
> 0.1.0 只做本地聊天内核；局域网联网在 0.2。

## 功能

- 上下双 panel：上方信息列表，下方状态（3 行）。
- 消息格式：`[name][datetime]: message`。
- `i` 打开输入 panel；Enter 发送，Esc/空 Enter 取消。
- 选中分**自动/手动**两种模式：
  - 默认自动选中最后一条；新消息自动选最新。
  - `j/k` 选中非最新消息后进入手动模式；手动模式下新消息（含自己发送的）不自动跳。
  - `j/k` 选回最新后恢复自动。
- `y` 复制正文，`Y` 复制完整 `[name][datetime]: message`；状态显示“复制成功”。
- 列表默认选中最后一条；输入 panel 打开时选中高亮消失，`j/k/y/Y/i` 只作为文字输入。
- `Q`（List 层）退出；Compose 下 `Q` 是普通字符。
- 本地 JSONL 历史：启动加载最近消息，退出 flush；文件权限 `0600`。
- `keymap.toml` 可覆盖 List 层键位；坏配置回退默认。
- 中文/emoji 按显示宽度处理；程序不加载字体。

## 构建与运行

```bash
cargo run
cargo test
```

首次运行会在 XDG 配置目录生成 `identity.json`（node_id + 昵称）。

## 默认键位

| 层 | 键 | 动作 |
| --- | --- | --- |
| List | `i` | 打开输入 panel |
| List | `j` / `k` | 下/上移动选中（并切换自动/手动） |
| List | `y` | 复制正文 |
| List | `Y` | 复制完整行 |
| List | `Q` | 退出 |
| Compose | `Enter` | 非空发送；空/纯空白取消 |
| Compose | `Esc` | 取消，丢弃输入 |
| Compose | 其他字符 | 文本输入（`j/k/y/Y/i/Q` 都是文字） |

Compose 下只响应 `Enter`/`Esc` 两个**应用快捷键**；方向键、Backspace、Delete、Ctrl+w/u/k/a/e 等仍用于文本编辑。

## 配置

Linux（遵循 XDG）：

| 类型 | 路径 |
| --- | --- |
| 配置 | `~/.config/lanchat/config.toml` |
| 键位 | `~/.config/lanchat/keymap.toml` |
| 身份 | `~/.config/lanchat/identity.json` |
| 历史 | `~/.local/share/lanchat/history/` |
| 日志 | `~/.local/state/lanchat/logs/` |

`config.toml` 示例：

```toml
[identity]
name = "alice@arch"

[ui]
show_help_hint = true

[history]
enabled = true
max_messages = 5000
max_file_mb = 10
```

`keymap.toml` 示例：

```toml
[list]
"j" = "cursor_down"
"k" = "cursor_up"
"y" = "copy_body"
"Y" = "copy_line"
"i" = "open_compose"

# 解绑： "j" = "none"
# 多键序列： "g g" = "cursor_top"（0.1.0 可选能力）
```

`Q` 是退出保留键，不能重映射或解绑；Compose 的 `Enter`/`Esc` 固定。

```bash
lanchat --no-config            # 忽略用户配置和键位
lanchat --config ./config.toml # 指定配置文件
lanchat --keymap ./keys.toml   # 指定键位文件
lanchat --debug                # 输出 debug 日志
lanchat --version
lanchat --help
```

## 中文与字体

TUI **不加载字体**。终端需要使用支持 CJK 的等宽字体，例如 Sarasa Mono SC、Noto Sans Mono CJK SC、Maple Mono NF CN 等，并保持 UTF-8 locale。程序用 `unicode-width` 计算 2 列宽的 CJK/emoji；字体缺字会显示 tofu（□），属于终端字体配置问题。

## 已知限制（0.1.0）

- 不联网：没有 peer、mDNS、TCP、在线人数（状态显示“在线 1（本机）”）。
- 没有文件共享、多频道、私聊、多行输入、搜索/命令/帮助、主题自定义。
- 没有鼠标操作。
- 剪贴板优先 `arboard`，失败时尝试 OSC 52；SSH/headless 终端可能不支持。
- Windows 未作为主要验收平台。
- 详细需求与设计见 [docs/requ/0.1.0/README.md](docs/requ/0.1.0/README.md)。

## 开发

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

旧版实现保留在 [`old/`](old/)，不参与编译。
