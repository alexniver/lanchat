# LanChat 0.1.1

> Helix-inspired、键位可自定义的局域网终端聊天工具。
>
> 0.1.0 的单机聊天内核已冻结；0.1.1 加入 mDNS 发现、TCP 单房间聊天、在线状态、右侧成员侧边栏、心跳/重连。

## 功能

- **双 panel TUI**：上方信息列表，下方 3 行状态。
- **局域网聊天**：mDNS 自动发现 + `--connect` 手动地址兜底；TCP 单房间 `#lobby`。
- **在线状态**：状态显示 `在线 N`（N 含本机）；`● online` / `◌ connecting` / `◌ reconnecting` / `! offline`。
- **成员侧边栏**：`Space u` 打开右侧侧边栏，再次 `Space u` 或 `Esc` 关闭；`j/k` 选择成员，不做私聊。
- **消息格式**：`[name][datetime]: message`；远端消息显示本地接收时间。
- **自动/手动选中**：
  - 默认自动选最新；新消息自动跟随。
  - `j/k` 选中非最新后进入手动模式；手动下新消息（含自己发送）不自动跳。
  - 选回最新后恢复自动。
- **复制**：`y` 复制正文，`Y` 复制完整行；状态显示“复制成功”。
- **本地历史**：JSONL；远端消息记录可选 `from`/`sent_at`；退出 flush；文件权限 `0600`。
- **重连**：Ping/Pong 心跳；断线指数退避重连；`Space r` 重连，`Space d` 断开。
- **可配置 keymap**：`keymap.toml` 覆盖 List 动作；坏配置回退默认。
- **中文**：`unicode-width` 处理 CJK/emoji；程序不加载字体。

## 构建与运行

```bash
cargo run
cargo test
```

两台机器/两个实例：

```bash
# 机器 A
lanchat --port 47731

# 机器 B：优先 mDNS 自动发现；也可手动连接
lanchat --port 47732 --connect 192.168.1.10:47731
```

本机测试两个实例时，使用不同端口和不同配置目录：

```bash
XDG_CONFIG_HOME=/tmp/lanchat-a lanchat --port 47741
XDG_CONFIG_HOME=/tmp/lanchat-b lanchat --port 47742 --connect 127.0.0.1:47741
```

## 默认键位

| 层 | 键 | 动作 |
| --- | --- | --- |
| List | `i` | 打开输入 panel |
| List | `j` / `k` | 下/上移动选中（并切换自动/手动） |
| List | `y` | 复制正文 |
| List | `Y` | 复制完整行 |
| List | `Space u` | 打开/关闭右侧成员侧边栏 |
| List | `Space r` | 重连 |
| List | `Space d` | 手动断开 |
| List | `Q` | 退出 |
| 成员侧边栏 | `j` / `k` | 选择成员 |
| 成员侧边栏 | `Space u` / `Esc` | 关闭 |
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

[network]
enabled = true
port = 47731
mdns = true
reconnect = true
seeds = []                 # 手动地址，如 ["192.168.1.23:47731"]
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
# 多键序列： "g g" = "cursor_top"
```

`Q` 是退出保留键，不能重映射或解绑；Compose 的 `Enter`/`Esc` 固定。

```bash
lanchat --no-config            # 忽略用户配置和键位
lanchat --no-network           # 完全回到本地模式
lanchat --config ./config.toml
lanchat --keymap ./keys.toml
lanchat --port 47731
lanchat --connect host:port    # 可重复
lanchat --version
lanchat --help
```

## 网络与安全

- 服务类型：`_lanchat._tcp.local.`；TXT 含 `id`/`name`/`v`/`proto`。
- 4 字节大端长度 + JSON 帧；最大帧 64 KiB，消息正文 16 KiB。
- 同一 node_id 只保留一条连接；重复连接按 node_id/握手去重。
- 发送失败、协议错误、连接断开都通过状态/toast 提示，不静默丢消息。
- **明文传输，无加密、无鉴权**：只应在可信局域网使用；不要在不可信网络传输敏感内容。
- 不提供文件共享、历史同步、离线消息队列、互联网中继。
- mDNS 不可用时用 `--connect`；IPv4 优先，IPv6 best effort。

## 中文与字体

TUI **不加载字体**。终端需要使用支持 CJK 的等宽字体，例如 Sarasa Mono SC、Noto Sans Mono CJK SC、Maple Mono NF CN 等，并保持 UTF-8 locale。程序用 `unicode-width` 计算 2 列宽的 CJK/emoji；字体缺字会显示 tofu（□），属于终端字体配置问题。

## 已知限制（0.1.1）

- 单房间；无多频道/私聊。
- 无文件共享。
- 无加密/鉴权；无历史同步；无离线队列/ACK。
- 无多行输入、搜索/命令/帮助、主题自定义、鼠标。
- Windows 未作为主要验收平台。
- 详细需求与设计见 [docs/0.1.1/README.md](docs/0.1.1/README.md)；0.1.0 历史需求见 [docs/requ/0.1.0/README.md](docs/requ/0.1.0/README.md)。

## 开发

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

旧版实现保留在 [`old/`](old/)，不参与编译。
