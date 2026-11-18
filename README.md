# LanChat 0.1.4

LanChat 是一个 **Helix 风格键位、键盘优先的局域网终端聊天工具**。它运行在终端里，不需要中心服务器，同一局域网内的节点通过 mDNS 自动发现，再用 TCP 直连聊天。

0.1.4 增加 Windows/macOS 路径与接收文件名校验；0.1.3 已加入内置帮助，0.1.2 已加入文件传递。目标是“聊天、发文件、查看帮助都在终端里完成”：

- 两个 panel：消息列表 + 状态栏。
- `i` 输入、Enter 发送、Esc 取消。
- `j/k` 在消息间移动；自动/手动两种选中模式。
- `y` 复制正文、`Y` 复制完整行。
- mDNS 自动发现；mDNS 不可用时可以手动填地址。
- 右侧成员侧边栏、在线人数、心跳和断线重连。
- 中文/emoji 按终端显示宽度处理。
- `/send file ... to ...` 发文件，对方 `/recv file ...` 接收；文件保存到平台数据目录下的 `lanchat/files`。
- 重名自动保存为 `a[0].png`、`a[1].png`；10 分钟未接收自动取消。
- 在消息列表按 `?` 打开内置帮助，`j/k` 滚动，`Esc` 返回。
- 配置/历史/接收文件使用平台目录；Windows/macOS 路径已适配。
- 本地 JSONL 历史；重启后还能看到之前消息。

> **安全提醒**：0.1.4 是明文局域网聊天/文件传递，没有加密、没有身份认证。请只在可信局域网使用，不要传输敏感信息。

## 界面

```text
┌ 信息列表 ──────────────────────────────────────────────────────┐
│ [alice][2026-11-19 10:30:01]: 你好                             │
│ [bob][2026-11-19 10:30:05]: 在                                 │
│ [alice][2026-11-19 10:31:12]: 今天先到这里                     │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
┌ 状态 ──────────────────────────────────────────────────────────┐
│ 在线 2 · 消息 3 · 选中 3/3 · 自动 · ● online                   │
└─────────────────────────────────────────────────────────────────┘
```

- 上 panel：消息列表。
- 下 panel：在线人数、消息数、选中位置、自动/手动模式、连接状态。
- 按 `i` 后，消息列表下方会出现输入 panel；发送/取消后消失。
- 按 `/` 打开输入 panel，预填 `/`，用于 `/send file`、`/recv file` 等命令。
- 按 `?` 打开内置帮助，`j/k` 滚动，`Esc` 返回。
- 按 `Space u` 后，右侧出现成员侧边栏。

## 安装

### 前置条件

- 较新的 stable Rust 工具链（edition 2021）。没装 Rust 的话先安装 [rustup](https://rustup.rs/)。
- 一个支持 UTF-8 的终端；要显示中文，终端还需要 CJK 等宽字体。
- 局域网聊天需要节点在同一子网，并允许 mDNS（UDP 5353）和 TCP 端口（默认 47731）互通。

### 从源码构建

在项目目录执行：

```bash
cargo build --release
```

生成的二进制在：

```text
target/release/lanchat
```

直接运行：

```bash
./target/release/lanchat
```

### 安装到 `~/.cargo/bin`

```bash
cargo install --path . --locked
```

之后可以直接运行：

```bash
lanchat --version
lanchat
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

同一局域网内，双方会通过 mDNS 自动发现。如果自动发现不可用，在 B 上手动连接 A：

```bash
lanchat --connect 192.168.1.10:47731
```

A 的地址换成实际局域网 IP。默认 TCP 端口是 `47731`；如果 A 用了别的端口，手动连接时也要写对。

### 本机启动两个实例

同一台机器需要不同端口，并建议使用不同的 XDG 目录，避免身份/历史混在一起：

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

完全等同 0.1.0 的单机聊天：不启动 mDNS/TCP，不显示连接状态，仍可使用输入、历史、复制、键位配置。

## 用法

### 输入与发送

| 操作 | 按键 |
| --- | --- |
| 打开输入 panel | `i` |
| 发送 | `Enter`（输入非空时） |
| 关闭输入 panel / 取消 | `Esc`，或空内容按 `Enter` |
| 光标左右 | `←` / `→` |
| 行首 / 行尾 | `Home` / `End`，或 `Ctrl+a` / `Ctrl+e` |
| 删除字符 | `Backspace` / `Delete` |
| 删除词 / 到行首 / 到行尾 | `Ctrl+w` / `Ctrl+u` / `Ctrl+k` |

输入 panel 打开时，只有 `Enter` 和 `Esc` 是**应用快捷键**；`j`、`k`、`y`、`Y`、`i`、`Q` 都会作为普通文字输入。方向键、Backspace 等仍用于文本编辑。

发送空消息不会产生消息；单条消息上限 16 KiB。

### 文件传递（0.1.2）

发送方：

```text
/send file ~/Download/a.png to 103
```

- `/` 打开输入框并预填 `/`。
- `<对方>` 支持完整 IP、IP 最后一段（如 `103`）、在线昵称。
- 接收方会看到：

```text
[系统][10:30:05]: 192.168.1.101 想发送文件 a.png（1.2 MB）。输入 /recv file 101 接收。
```

接收方输入：

```text
/recv file 101
```

- 接收该发送方全部待接收文件。
- 文件保存到平台数据目录下的 `lanchat/files`（Linux 为 `~/.local/share/lanchat/files`）；重名自动改为 `a[0].png`、`a[1].png`。
- 请求 10 分钟未接收自动取消；同一文件重复请求会被拦截。
- 文件传输走独立数据 TCP 连接，控制消息走聊天连接，避免大文件阻塞聊天。
- 文件请求/成功/取消是系统消息，不写入聊天历史。

文件保存目录（Linux）：

```text
~/.local/share/lanchat/files
```

### 内置帮助

在消息列表按 `?`：

- 上方显示帮助内容，下方保留状态栏；
- `j` / `k` 上下滚动；
- `Esc` 退出帮助，回到消息列表；
- 帮助打开时不会改变当前选中消息，也不会写历史/发网络消息。

帮助状态栏示例：

```text
按 Esc 退出帮助 · j/k 滚动 · 帮助 1/24 · LanChat 0.1.4
```

### 选中消息

默认是**自动模式**：始终选中最新消息，新消息到达时自动跟到底部。

按 `j` / `k` 选中非最新消息后，进入**手动模式**：

- 新消息（包括自己发送的）不会自动改变当前选中/滚动位置。
- 状态栏显示“手动”和“新消息 N”。
- 按 `j` 选回最新消息后，自动模式恢复。

手动模式下自己发送消息也不会自动跳到最新，这是 0.1.1 的明确行为。

### 复制

- `y`：复制当前消息正文。
- `Y`：复制完整格式 `[name][datetime]: message`。
- 成功后状态栏显示“复制成功”；失败显示“复制失败”。

优先使用系统剪贴板（`arboard`）；不可用时尝试 OSC 52，适合部分 SSH/远程终端。

### 成员侧边栏

- `Space u`：打开右侧成员侧边栏；再按 `Space u` 或 `Esc` 关闭。
- `j` / `k`：在成员之间选择。
- 本机标注 `(you)`。
- 成员面板不做私聊，只显示昵称、地址和连接状态。

### 重连与断开

| 操作 | 按键 |
| --- | --- |
| 重连 | `Space r` |
| 手动断开 | `Space d` |
| 退出 | `Q`（仅消息列表层） |

`Q` 是大写字母。Compose 下按 `Q` 只会输入字符 `Q`；先 `Esc` 回消息列表，再按 `Q` 退出。

`Space r` / `Space d` 是两键序列：先按空格，再按 `r` 或 `d`。

### 默认键位

| 层 | 键 | 动作 |
| --- | --- | --- |
| 消息列表 | `i` | 打开输入 panel |
| 消息列表 | `/` | 打开命令输入，预填 `/` |
| 消息列表 | `?` | 打开内置帮助 |
| 消息列表 | `j` / `k` | 下/上移动选中 |
| 消息列表 | `y` | 复制正文 |
| 消息列表 | `Y` | 复制完整行 |
| 消息列表 | `Space u` | 打开/关闭成员侧边栏 |
| 消息列表 | `Space r` | 重连 |
| 消息列表 | `Space d` | 手动断开 |
| 消息列表 | `Q` | 退出 |
| 成员侧边栏 | `j` / `k` | 选择成员 |
| 成员侧边栏 | `Space u` / `Esc` | 关闭 |
| 帮助 | `j` / `k` | 上下滚动帮助 |
| 帮助 | `Esc` | 退出帮助 |
| 输入 panel | `Enter` | 非空发送；空内容取消 |
| 输入 panel | `Esc` | 取消 |
| 输入 panel | 其他字符 | 文本输入 |

键位可以改；见下面的 `keymap.toml`。

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

Linux 遵循 XDG；macOS/Windows 使用 `dirs` 库的系统目录。`--config` 只改变配置文件位置，历史/接收文件/日志仍使用平台数据目录。

首次运行会自动生成 `identity.json`：

```json
{
  "node_id": "…",
  "name": "alice@host"
}
```

`node_id` 用于网络去重/识别，建议不要删除；昵称可以通过配置修改。

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

字段说明：

| 字段 | 说明 |
| --- | --- |
| `identity.name` | 显示名；会覆盖 `identity.json` 里的昵称 |
| `ui.show_help_hint` | 启动时是否显示操作提示 |
| `history.enabled` | 是否启用本地历史 |
| `history.max_messages` | 启动加载最近多少条 |
| `history.max_file_mb` | 历史文件轮转阈值 |
| `network.enabled` | 是否启动网络 |
| `network.port` | TCP 监听端口，默认 47731 |
| `network.mdns` | 是否启用 mDNS 自动发现 |
| `network.reconnect` | 断线后是否自动重连 |
| `network.seeds` | 手动连接地址列表 |

### `keymap.toml`

只配置消息列表（List 层）：

```toml
[list]
"j" = "cursor_down"
"k" = "cursor_up"
"y" = "copy_body"
"Y" = "copy_line"
"i" = "open_compose"

# 解绑
# "j" = "none"

# 多键序列
# "g g" = "cursor_top"
```

可用 action：

| action | 说明 |
| --- | --- |
| `open_compose` | 打开输入 panel |
| `open_command` | 打开命令输入（预填 `/`） |
| `cursor_down` / `cursor_up` | 下/上移动选中 |
| `copy_body` / `copy_line` | 复制正文 / 完整行 |
| `toggle_members` | 打开/关闭成员侧边栏 |
| `reconnect` / `disconnect` | 重连 / 断开 |
| `quit` | 退出 |
| `noop` | 无动作 |

注意：

- `Q` 是退出保留键，不能重映射或解绑。
- Compose 的 `Enter`/`Esc` 固定，不能改。
- 未知 action、错误按键语法、前缀冲突会报告并忽略；整个 TOML 语法错误会回退默认键位。

### 命令行参数

```bash
lanchat [选项]
```

| 参数 | 说明 |
| --- | --- |
| `--config <PATH>` | 指定 `config.toml` |
| `--keymap <PATH>` | 指定 `keymap.toml` |
| `--no-config` | 忽略用户配置和键位，使用默认 |
| `--no-network` | 不启动网络，完全本地模式 |
| `--port <PORT>` | TCP 监听端口，默认 47731 |
| `--connect <ADDR>` | 手动连接 `host:port`，可重复 |
| `--debug` | 输出 debug 日志到文件 |
| `--version`, `-V` | 显示版本 |
| `--help`, `-h` | 显示帮助 |

## 网络

- mDNS 服务类型：`_lanchat._tcp.local.`。
- 单房间 `#lobby`；TCP 全连接 mesh。
- 帧格式：4 字节大端长度 + JSON；最大帧 64 KiB，单条消息正文 16 KiB。
- 同一 `node_id` 只保留一条连接；重复连接在握手后去重。
- 心跳：Ping/Pong；断线使用指数退避重连。
- 文件控制消息走 peer 连接；文件内容走独立数据 TCP 连接，复用监听端口。
- 状态栏在线数包含本机；`在线 1` 表示只有自己。
- 节点加入/离开只显示状态提示，不插入消息列表，避免打断选中。
- 远端消息按本地接收时间显示；历史会保留发送方的 `sent_at` 和可选 `from`。

mDNS 自动发现失败时：

```bash
lanchat --connect 192.168.1.10:47731
```

或在 `config.toml` 的 `[network] seeds` 中配置。端口被占用时应用会回退本地模式并给出提示，可以用 `--port` 换端口。

### 安全边界

0.1.4 **不提供**：

- 加密、身份认证、前向保密；
- 文件夹同步/自动共享；
- 历史同步；
- 离线消息队列/ACK；
- 互联网中继/NAT 穿透。

因此只适合可信局域网。日志不记录消息正文；Unix 下历史文件权限为 `0600`。

## 中文与字体

TUI **不加载字体**。终端字体由终端模拟器提供，程序只输出 UTF-8 和 ANSI。

要正确显示中文：

1. 终端使用支持 CJK 的等宽字体，例如 Sarasa Mono SC、Noto Sans Mono CJK SC、Maple Mono NF CN 等。
2. 终端 locale/编码为 UTF-8。
3. 程序用 `unicode-width` 计算显示宽度；中文/全角字符占 2 列，ASCII 占 1 列。

字体缺字会显示 tofu（□），这是终端字体问题，不是程序能修复的。

## 已知限制

- 单房间；没有多频道、私聊。
- 没有文件夹同步；文件传递需要发送方 `/send`、接收方 `/recv`。
- 没有断点续传；传输中断后需要重发。
- 没有加密/鉴权；没有历史同步；没有离线队列/ACK。
- 单行输入；没有多行编辑。
- 命令目前只有 `/send file`、`/recv file`；没有 `/help`、搜索、交互式教程、主题、鼠标。
- Windows/macOS 路径已适配，但真机验收仍待完成。
- mDNS 依赖局域网多播；在容器/受限网络里可能需要 `--connect`。

## 故障排查

### 看不到对方

- 确认两台机器在同一局域网/子网。
- 确认防火墙允许 mDNS（UDP 5353）和 TCP 47731。
- 检查状态栏是否显示 `◌ connecting` / `! offline`。
- 用 `--connect <对方IP>:47731` 手动连接。
- 确认两端没有用同一个 `node_id`（复制配置目录可能导致身份重复）。

### 端口被占用

```bash
lanchat --port 47732
```

同一台机器跑两个实例时，两个实例必须用不同端口和不同配置目录。

### 复制失败

- 本机优先系统剪贴板（X11/Wayland/macOS/Windows）。
- SSH/headless 下会尝试 OSC 52；需要终端支持。
- 如果终端不支持，状态会显示“复制失败”。

### 中文错位 / 显示方块

- 检查终端字体是否有 CJK 字形。
- 检查 `LANG` / `LC_ALL` 是否为 UTF-8。
- 换用等宽 CJK 字体或配置 fallback。

### 网络启动失败

应用会回退到本地模式并在状态栏提示。可以先用 `--no-network` 确认本地功能正常，再检查端口/mDNS。

## 开发

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

旧版实现保留在 [`old/`](old/)，不参与编译。0.1.0 的历史需求在 [docs/requ/0.1.0/README.md](docs/requ/0.1.0/README.md)，0.1.1 联网设计与决策在 [docs/0.1.1/README.md](docs/0.1.1/README.md)，0.1.2 文件传递需求在 [docs/0.1.2/README.md](docs/0.1.2/README.md)，0.1.3 帮助页面需求在 [docs/0.1.3/README.md](docs/0.1.3/README.md)，0.1.4 跨平台路径适配在 [docs/0.1.4/README.md](docs/0.1.4/README.md)。

## 许可证

MIT（见 `Cargo.toml`）。
