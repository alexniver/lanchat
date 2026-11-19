# LanChat 0.1.9

LanChat 是一个 **Helix 风格键位、键盘优先的局域网终端聊天工具**。它运行在终端里，不需要中心服务器，同一局域网内的节点通过 mDNS 自动发现，再用 TCP 直连聊天和传递文件。

> **安全提醒**：明文传输，无加密、无鉴权。只应在可信局域网使用。
>
> 版本变化见 [CHANGELOG.md](CHANGELOG.md)。

## 功能

- 上下双 panel：消息列表 + 3 行状态栏。
- `i` 输入、Enter 发送、Esc 取消。
- `j/k` 在消息间移动；自动/手动两种选中模式。
- `y` 复制正文、`Y` 复制完整行。
- mDNS 自动发现；mDNS 不可用时可用 `--connect` 手动连接。
- 右侧成员侧边栏、在线人数、心跳和断线重连。
- `/setname` 修改昵称，显示为 `alice (no1)`。
- `/sendfile` / `/recvfile` 发送文件；`/sendfolder` / `/recvfolder` 发送文件夹。
- 文件夹保留目录结构和空目录，重名自动编号；发送端不设大小/数量限制。
- 接收前检查磁盘空间，要求剩余空间不少于待接收数据的 1.5 倍。
- `?` 打开内置帮助，`j/k` 滚动，`Esc` 返回。
- 中文/emoji 按终端显示宽度处理。
- 本地 JSONL 历史，重启后保留。

## 界面

```text
┌ 信息列表 ──────────────────────────────────────────────────────┐
│ [alice (no1)][2026-11-19 10:30:01]: 你好                        │
│ [bob (no2)][2026-11-19 10:30:05]: 在                            │
│ [alice (no1)][2026-11-19 10:31:12]: 今天先到这里                │
└─────────────────────────────────────────────────────────────────┘
┌ 状态 ──────────────────────────────────────────────────────────┐
│ 本机 alice (no1) · 在线 2 · 消息 3 · 选中 3/3 · 自动 · ● online   [?]帮助 │
└─────────────────────────────────────────────────────────────────┘
```

- 按 `i` 打开输入 panel；发送/取消后消失。
- 按 `/` 打开命令输入，预填 `/`。
- 按 `?` 打开帮助。
- 按 `Space u` 打开成员侧边栏。

## 安装

### 前置条件

- 较新的 stable Rust（edition 2021）。没有 Rust 时先安装 [rustup](https://rustup.rs/)。
- UTF-8 终端；显示中文需要 CJK 等宽字体。
- 局域网聊天需要同子网，并允许 mDNS（UDP 5353）和 TCP 端口（默认 47731）互通。

### 从源码构建

```bash
cargo build --release
./target/release/lanchat
```

### 安装到 Cargo bin

```bash
cargo install --path . --locked
lanchat --version
```

### 开发模式运行

```bash
cargo run --release
```

## 快速开始

### 两台机器

机器 A：

```bash
lanchat
```

机器 B：

```bash
lanchat
```

同一局域网内会通过 mDNS 自动发现。若自动发现不可用，在 B 上手动连接 A：

```bash
lanchat --connect 192.168.1.10:47731
```

默认端口是 `47731`；A 使用其他端口时，`--connect` 也要写对应端口。

### 本机启动两个实例

```bash
# 实例 A
XDG_CONFIG_HOME=/tmp/lanchat-a/config \
XDG_DATA_HOME=/tmp/lanchat-a/data \
XDG_STATE_HOME=/tmp/lanchat-a/state \
lanchat --port 47741

# 实例 B
XDG_CONFIG_HOME=/tmp/lanchat-b/config \
XDG_DATA_HOME=/tmp/lanchat-b/data \
XDG_STATE_HOME=/tmp/lanchat-b/state \
lanchat --port 47742 --connect 127.0.0.1:47741
```

### 只使用本地模式

```bash
lanchat --no-network
```

不启动 mDNS/TCP，仍可使用输入、历史、复制和键位配置。

## 用法

### 输入与发送

| 操作 | 按键 |
| --- | --- |
| 打开输入 panel | `i` |
| 发送 | `Enter`（内容非空） |
| 关闭/取消 | `Esc`，或空内容按 `Enter` |
| 光标左右 | `←` / `→` |
| 行首/行尾 | `Home` / `End`，或 `Ctrl+a` / `Ctrl+e` |
| 删除字符 | `Backspace` / `Delete` |
| 删除词/到行首/到行尾 | `Ctrl+w` / `Ctrl+u` / `Ctrl+k` |

输入 panel 打开时，只有 `Enter` 和 `Esc` 是应用快捷键；`j`、`k`、`y`、`Y`、`i`、`Q` 都作为文字输入。单条消息上限 16 KiB。

### 选中与复制

默认自动选中最新消息。按 `j` / `k` 选中非最新消息后进入手动模式：

- 新消息（包括自己发送的）不会自动跳转；
- 状态栏显示“手动”和“新消息 N”；
- 按 `j` 选回最新后恢复自动。

- `y`：复制正文；
- `Y`：复制完整格式 `[name][datetime]: message`；
- 成功/失败显示在状态栏。

### 修改名称

```text
/setname alice
```

成功后：

- 显示名变为 `alice (no1)`；
- 状态栏变为 `本机 alice (no1) · ...`；
- 更新 `identity.json` 和 `config.toml` 的 `[identity].name`（如果存在）；
- 在线 peer 收到 `NameUpdate`；旧版本 peer 重连后看到新名字；
- 历史消息不重写。

兼容写法：`/set name alice`。

### 文件传递

发送方：

```text
/sendfile ~/Download/a.png to no1
/sendfile ~/Download/a.png to ip101
```

`<对方>` 支持：

- 短号 `no1`；
- IP 最后一段 `ip101`；
- 完整 node_id 或唯一前缀；
- 完整 IPv4；
- 在线昵称。

接收方会看到：

```text
[系统][10:30:05]: alice (no1) 想发送文件 a.png（1.2 MB）。输入 /recvfile no1 接收。
```

接收方输入：

```text
/recvfile no1
```

行为：

- 接收该发送方全部待接收文件；
- 请求 10 分钟未接收自动取消；
- 同一文件名的未完成请求会被拦截；
- 文件保存到 `lanchat/files`；
- 重名保存为 `a[0].png`、`a[1].png`；
- 发送端不设大小/数量限制；接收端按 1.5 倍磁盘空间检查；
- 文件内容走独立数据 TCP 连接，不阻塞聊天控制消息；
- 文件事件是系统消息，不写入历史。

兼容写法：`/send file`、`/recv file`。

### 文件夹传递

```text
/sendfolder ~/Pictures/holiday to no1
```

接收方：

```text
/recvfolder no1
/recv folder no1   # 兼容写法
```

行为：

- 递归发送，保留相对目录结构和空目录；
- 符号链接默认跳过；
- 根目录重名保存为 `holiday[0]`、`holiday[1]`；
- 文件夹先写临时目录，全部完成后 rename；失败整体清理；
- 发送端不设单文件/文件夹大小、文件数、深度限制；
- 接收前检查 `剩余空间 >= 待接收数据 × 1.5`，不足则双方提示“磁盘空间不足，传送失败”；
- 请求 10 分钟未接收自动取消；
- 文件夹事件是系统消息，不写入历史。

### 内置帮助

在消息列表按 `?`：

- 帮助内容占据主区域；
- `j` / `k` 上下滚动；
- `Esc` 退出帮助；
- 帮助只读，不改变当前消息选中，也不写历史/发网络消息。

### 成员、重连与退出

| 操作 | 按键 |
| --- | --- |
| 成员侧边栏开关 | `Space u` |
| 成员选择 | `j` / `k` |
| 重连 | `Space r` |
| 手动断开 | `Space d` |
| 退出 | `Q`（仅消息列表） |

`Q` 是大写字母。输入 panel 下按 `Q` 只是输入字符 `Q`；先 `Esc` 回消息列表再退出。

### 默认键位

| 层 | 键 | 动作 |
| --- | --- | --- |
| 消息列表 | `i` | 打开输入 panel |
| 消息列表 | `/` | 命令输入 |
| 消息列表 | `?` | 打开帮助 |
| 消息列表 | `j` / `k` | 下/上选择消息 |
| 消息列表 | `y` / `Y` | 复制正文 / 完整行 |
| 消息列表 | `Space u` | 成员侧边栏 |
| 消息列表 | `Space r` / `Space d` | 重连 / 断开 |
| 消息列表 | `Q` | 退出 |
| 成员侧边栏 | `j` / `k` | 选择成员 |
| 成员侧边栏 | `Space u` / `Esc` | 关闭 |
| 帮助 | `j` / `k` | 滚动 |
| 帮助 | `Esc` | 退出帮助 |
| 输入 panel | `Enter` | 非空发送 |
| 输入 panel | `Esc` | 取消 |

键位可改，见 `keymap.toml`。

## 配置

### 文件位置

| 内容 | Linux | macOS | Windows |
| --- | --- | --- | --- |
| 配置 | `~/.config/lanchat/config.toml` | `~/Library/Application Support/lanchat/config.toml` | `%APPDATA%\lanchat\config.toml` |
| 键位 | `~/.config/lanchat/keymap.toml` | `~/Library/Application Support/lanchat/keymap.toml` | `%APPDATA%\lanchat\keymap.toml` |
| 身份 | `~/.config/lanchat/identity.json` | `~/Library/Application Support/lanchat/identity.json` | `%APPDATA%\lanchat\identity.json` |
| 历史 | `~/.local/share/lanchat/history/` | `~/Library/Application Support/lanchat/history/` | `%LOCALAPPDATA%\lanchat\history\` |
| 接收文件 | `~/.local/share/lanchat/files/` | `~/Library/Application Support/lanchat/files/` | `%LOCALAPPDATA%\lanchat\files\` |
| 日志 | `~/.local/state/lanchat/logs/` | `~/Library/Application Support/lanchat/logs/` | `%LOCALAPPDATA%\lanchat\logs\` |

Linux 遵循 XDG；macOS/Windows 使用系统目录。`--config` 只改变配置文件位置，历史/接收文件/日志仍使用平台数据目录。

首次运行生成 `identity.json`：

```json
{
  "node_id": "…",
  "name": "alice@host",
  "short_no": 1
}
```

- `node_id` 是 UUID，用于网络去重/识别，建议不要删除。
- `short_no` 是短号，`1` 显示为 `no1`；冲突时自动顺延。
- `/setname` 会同时更新 `name` 和 `config.toml` 的 `[identity].name`。

### `config.toml`

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
seeds = []                 # 例如 ["192.168.1.23:47731"]
```

| 字段 | 说明 |
| --- | --- |
| `identity.name` | 昵称；覆盖 `identity.json` 里的昵称 |
| `ui.show_help_hint` | 启动时是否显示操作提示 |
| `history.enabled` | 是否启用历史 |
| `history.max_messages` | 启动加载最近多少条 |
| `history.max_file_mb` | 历史轮转阈值 |
| `network.enabled` | 是否启动网络 |
| `network.port` | TCP 端口，默认 47731 |
| `network.mdns` | 是否启用 mDNS |
| `network.reconnect` | 是否自动重连 |
| `network.seeds` | 手动连接地址 |

### `keymap.toml`

只配置消息列表：

```toml
[list]
"j" = "cursor_down"
"k" = "cursor_up"
"y" = "copy_body"
"Y" = "copy_line"
"i" = "open_compose"
"?" = "open_help"

# 解绑
# "j" = "none"

# 多键序列
# "g g" = "cursor_top"
```

可用 action：

| action | 说明 |
| --- | --- |
| `open_compose` | 输入 panel |
| `open_command` | 命令输入 |
| `open_help` | 帮助 |
| `cursor_down` / `cursor_up` | 下/上选择 |
| `copy_body` / `copy_line` | 复制正文 / 完整行 |
| `toggle_members` | 成员侧边栏 |
| `reconnect` / `disconnect` | 重连 / 断开 |
| `quit` | 退出 |
| `noop` | 无动作 |

- `Q` 是退出保留键，不能重映射/解绑。
- 输入 panel 的 `Enter`/`Esc` 固定。
- 未知 action、错误按键、前缀冲突会报告并忽略；TOML 语法错误回退默认。

### 命令行参数

| 参数 | 说明 |
| --- | --- |
| `--config <PATH>` | 指定 `config.toml` |
| `--keymap <PATH>` | 指定 `keymap.toml` |
| `--no-config` | 忽略用户配置和键位 |
| `--no-network` | 完全本地模式 |
| `--port <PORT>` | TCP 端口，默认 47731 |
| `--connect <ADDR>` | 手动连接 `host:port`，可重复 |
| `--debug` | debug 日志 |
| `--version`, `-V` | 显示版本 |
| `--help`, `-h` | 显示帮助 |

## 网络与安全

- mDNS 服务：`_lanchat._tcp.local.`。
- 单房间 `#lobby`，TCP 全连接 mesh。
- 4 字节大端长度 + JSON 帧；最大帧 64 KiB，消息正文 16 KiB。
- 同一 node_id 只保留一条连接；心跳/断线重连。
- 文件控制消息走 peer 连接；文件内容走独立数据 TCP 连接。
- 远端消息显示本地接收时间；历史保留 `from`/`sent_at`。
- 不提供加密、鉴权、文件夹同步、历史同步、离线队列、互联网中继。
- 仅适合可信局域网。

mDNS 失败时用 `--connect` 或在 `[network] seeds` 配置；端口占用时用 `--port` 换端口。

## 中文与字体

TUI 不加载字体；终端使用支持 CJK 的等宽字体（如 Sarasa Mono SC、Noto Sans Mono CJK SC、Maple Mono NF CN），并保持 UTF-8 locale。程序用 `unicode-width` 计算显示宽度；缺字显示 tofu 属于终端字体问题。

## 故障排查

- **看不到对方**：检查同子网、防火墙 mDNS/UDP 5353 和 TCP 47731；用 `--connect`；确认 node_id 不重复。
- **端口占用**：`lanchat --port 47732`；本机双实例需不同端口和配置目录。
- **复制失败**：系统剪贴板不可用时尝试 OSC 52；终端需支持。
- **中文错位**：检查 CJK 字体和 `LANG`/`LC_ALL`。
- **网络启动失败**：应用回退本地模式并提示；可先用 `--no-network` 验证本地功能。

## 开发

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

旧版实现保留在 [`old/`](old/)，不参与编译。

- 当前需求：[docs/0.1.9/README.md](docs/0.1.9/README.md)
- 版本变化：[CHANGELOG.md](CHANGELOG.md)

## 许可证

MIT（见 `Cargo.toml`）。
