# lanchat — 局域网文本与文件共享工具
## 项目规格文档

---

## 一、项目概述

`lanchat` 是一个基于 Rust 的终端程序，运行在局域网内的多台机器上，实现：

1. **自动节点发现**：通过 mDNS 广播，自动找到同局域网内的其他 lanchat 实例，无需手动配置 IP。
2. **实时聊天**：所有节点加入同一个聊天室，消息广播给所有在线节点。
3. **文件同步**：每个节点暴露本地 `~/.local/share/lanchat/` 文件夹内的文件，其他节点可以按需下载。
4. **TUI 界面**：使用 `ratatui` 构建终端用户界面，分为聊天视图和文件视图。

---

## 二、技术栈

| 用途 | crate |
|------|-------|
| 异步运行时 | `tokio` (full features) |
| TUI 框架 | `ratatui` + `crossterm` |
| mDNS 发现 | `mdns-sd` |
| 序列化 | `serde` + `serde_json` |
| 文件哈希 | `sha2` |
| Base64 编解码 | `base64` |
| 错误处理 | `anyhow` |
| 日志 | `tracing` + `tracing-subscriber` + `tracing-appender` |
| UUID | `uuid` (v4) |
| 时间 | `chrono` |
| 目录路径 | `dirs` |
| 用户名获取 | `whoami` |
| 主机名获取 | `hostname` |
| 系统调用 | `libc` |

**Cargo.toml 依赖参考：**

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-appender = "0.2"
dirs = "6"
libc = "0.2"
mdns-sd = "0.19"
sha2 = "0.10"
base64 = "0.22"
ratatui = "0.29"
crossterm = "0.28"
whoami = "1"
hostname = "0.4"
```

---

## 三、目录结构

```
lanchat/
├── Cargo.toml
├── src/
│   ├── main.rs              # 入口：日志初始化、配置加载、启动各任务、event_handler
│   ├── config.rs            # 节点身份配置（UUID、hostname）、命令行参数解析
│   ├── network/
│   │   ├── mod.rs           # AppEvent 枚举定义
│   │   ├── connect.rs       # 主动连接逻辑（端口号去重、握手）
│   │   ├── discovery.rs     # mDNS 广播与发现（独立 std::thread）
│   │   ├── files.rs         # 本地文件扫描、SHA256 计算
│   │   ├── peer.rs          # 单个 Peer 连接的读写 task（read_loop / write_loop）
│   │   └── server.rs        # TCP Server：监听随机端口、接受入站连接
│   ├── protocol.rs          # 消息协议定义（枚举 + 序列化结构体）
│   ├── state.rs             # 全局共享状态（AppState、PeerState、ChatMessage）
│   └── tui/
│       ├── mod.rs           # TUI 主循环、键盘事件处理、crossterm 集成
│       ├── app.rs           # UI 状态机（Tab、UiState）
│       ├── chat_view.rs     # 聊天界面渲染
│       └── files_view.rs    # 文件同步界面渲染
```

---

## 四、节点配置（config.rs）

程序首次启动时，在 `~/.config/lanchat/identity.json` 生成并持久化节点身份：

```json
{
  "node_id": "550e8400-e29b-41d4-a716-446655440000",
  "display_name": "lin@archlinux"
}
```

- `node_id`：随机 UUIDv4，唯一标识一个节点
- `display_name`：默认为 `username@hostname`，用户可手动修改 identity.json

### 命令行参数

| 参数 | 说明 |
|------|------|
| `--port PORT` | 指定端口号（已解析但当前版本服务端使用随机端口，暂未生效） |
| `--name NAME` | 覆盖 display_name（已解析但当前版本暂未生效） |
| `--identity PATH` | 指定自定义身份文件路径 |

**共享目录**：`~/.local/share/lanchat/`（通过 `dirs::data_local_dir()` 获取，跨平台兼容）。

---

## 五、消息协议（protocol.rs）

所有消息均为 **JSON 换行符分隔**（每条消息占一行，末尾 `\n`），通过 TCP 传输。

### 5.1 消息枚举

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "payload")]
pub enum Message {
    /// 握手：连接建立后，双方互发此消息
    Hello(HelloPayload),

    /// 聊天消息广播
    Chat(ChatPayload),

    /// 请求对方的文件清单
    FileListRequest,

    /// 返回文件清单
    FileListResponse(FileListPayload),

    /// 请求下载某个文件
    FileRequest(FileRequestPayload),

    /// 文件数据响应（base64 编码内容）
    FileResponse(FileResponsePayload),

    /// 节点主动断开通知
    Goodbye,
}
```

### 5.2 各 Payload 结构

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HelloPayload {
    pub node_id: String,         // UUID
    pub display_name: String,    // "lin@archlinux"
    pub version: String,         // 来自 CARGO_PKG_VERSION
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatPayload {
    pub msg_id: String,          // UUID，用于去重
    pub from_node_id: String,
    pub from_name: String,
    pub content: String,
    pub timestamp: String,       // RFC3339，如 "2025-05-14T10:30:00+08:00"
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileListPayload {
    pub node_id: String,
    pub files: Vec<FileEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileEntry {
    pub name: String,            // 文件名（不含路径）
    pub size: u64,               // 字节数
    pub sha256: String,          // 文件 SHA256 hex 字符串，用于去重判断
    pub modified: String,        // RFC3339 修改时间
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileRequestPayload {
    pub file_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileResponsePayload {
    pub file_name: String,
    pub size: u64,
    pub sha256: String,
    pub data: String,            // base64 编码的文件内容
    /// 若文件不存在，data 为空字符串，且 not_found = true
    pub not_found: bool,
}
```

### 5.3 传输格式

- 每条消息序列化为 JSON，后跟 `\n`
- 接收端按行读取，每行反序列化为一条消息
- 握手阶段逐字节读取首条 Hello（避免 BufReader 内部缓冲问题）
- 后续消息使用 `tokio::io::BufReader` + `read_line()` 处理

---

## 六、节点发现（discovery.rs）

使用 `mdns-sd` 进行局域网服务广播与发现。

- **服务类型**：`_lanchat._tcp.local.`
- **服务实例名**：`{node_id}._lanchat._tcp.local.`
- **TXT 记录**：`node_id={uuid}`, `name={display_name}`
- **端口**：与 TCP 监听端口相同（随机端口，由 server 启动后获取）
- **主机名**：通过 `hostname` crate 获取，附加 `.local.` 后缀

**实现方式：**

1. 创建一个独立的标准库线程（`std::thread::spawn`），线程内部维护一个单线程 `tokio::runtime` 用于向 tokio channel 发送事件
2. 注册本机 mDNS 服务，地址为通过 UDP socket 探测到的首选非回环 IPv4
3. 启动 mDNS browse 监听，收到 `ServiceResolved` 事件后发送 `AppEvent::PeerDiscovered`
4. 收到 `ServiceRemoved` 事件后发送 `AppEvent::PeerVanished`
5. 忽略自身服务（IP 和端口同时匹配时跳过）
6. `recv_timeout` 1 秒循环，非阻塞

---

## 七、网络层（network/）

### 7.1 AppEvent 内部事件

区别于线上的 `Message`，`AppEvent` 是网络层与 Event Handler 之间的内部通信协议：

```rust
pub enum AppEvent {
    Message { msg: Message, from: SocketAddr },
    PeerDiscovered { node_id, display_name, addr },
    PeerVanished { node_id },
    PeerConnected { node_id, display_name, addr, tx },
    PeerDisconnected { node_id },
    SendChat { content },
    RequestFileList { node_id },
    DownloadFile { node_id, file_name },
}
```

### 7.2 连接管理

- 每个 Peer 连接启动两个 tokio task：**read_loop** 和 **write_loop**
- read_loop 使用 `BufReader::read_line()` 按行读取，反序列化为 `Message`，通过 `app_tx` 发送 `AppEvent::Message`
- write_loop 从 Peer 专属的 `mpsc::Receiver<Message>` 接收消息，序列化后写入 TCP stream
- 当任一 task 退出时，通过 `tokio::select!` 检测，随后发送 `AppEvent::PeerDisconnected`

### 7.3 去重连接规则

使用**端口号比较**作为连接发起依据：

- 本机 TCP 监听端口 < 对端端口 → 本机主动发起连接
- 本机 TCP 监听端口 > 对端端口 → 等待对端发起连接
- 使用端口号而非 node_id 字典序，避免两台机器使用相同 node_id 时双方都不发起连接

### 7.4 握手流程

```
A ──────── Hello ──────────► B
A ◄─────── Hello ────────── B
（握手完成，可开始正常通信）
```

- 主动方：`connect_and_handshake()` — TCP 连接 → 发送 Hello → 逐字节读取对端 Hello → 发送 `PeerConnected` → 将 stream 交给 `handle_connection_already_hello`
- 被动方：`handle_connection()` — 逐字节读取对端 Hello → 回复本机 Hello → 发送 `PeerConnected` → split stream 并 spawn 读写 task

握手完成后，双方自动互相请求文件清单：

```
A ──── FileListRequest ────► B
A ◄─── FileListResponse ─── B
```

### 7.5 消息广播

- 聊天消息由发送方广播给**所有已连接 peer**（通过各 peer 专属 `tx` channel）
- 消息用 `msg_id`（UUID）去重：`AppState.seen_msg_ids` 记录已处理的 msg_id
- 系统消息（节点连接/断开通知）仅在本机 `messages` 列表中添加，不广播

---

## 八、文件系统（files.rs）

### 8.1 共享目录

- 路径：`~/.local/share/lanchat/`（启动时自动创建，使用 `dirs::data_local_dir()`）
- lanchat 读取该目录下的普通文件，跳过隐藏文件和子目录
- 下载的文件保存到同一目录

### 8.2 本地文件扫描

```rust
pub fn scan_local_files(dir: &Path) -> anyhow::Result<Vec<FileEntry>>
```

- 只扫描目录的直接文件（不递归子目录）
- 跳过隐藏文件（`.` 开头）
- 为每个文件计算 SHA256（每次调用重新计算，不缓存到磁盘）
- 文件修改时间转换为 RFC3339 格式
- 结果按文件名排序

### 8.3 同步状态判断

对于远端节点的每个 `FileEntry`，判断本地是否已有同步版本：

- 本地不存在同名文件 → **可用**（显示蓝色 `[同步]`）
- 存在同名文件且 sha256 相同 → **已同步**（显示绿色 `[已同步]`）
- 存在同名文件但 sha256 不同 → **有更新**（显示黄色 `[有更新]`）

### 8.4 文件下载

- 用户选中文件按 `Enter` 或 `s` 触发下载，发送 `FileRequest`
- 收到 `FileResponse` 后进行 base64 解码并写入磁盘
- 文件名冲突处理：若本地已存在同名文件且 sha256 不同，保存为 `filename.conflict.ext`
- 下载完成后重新扫描本地文件列表，更新 TUI 显示
- 大文件（> 50MB）base64 传输会很慢，但功能正确

---

## 九、TUI 布局（tui/）

### 9.1 整体布局

```
┌─────────────────────────────────────────────────────────────┐
│  lanchat  [聊天] [文件]          节点：lin@arch | 在线: 3    │  ← 顶部状态栏
├─────────────────────────────────────────────────────────────┤
│                                                              │
│                      内容区域                                 │  ← 主内容（聊天 or 文件）
│                                                              │
├─────────────────────────────────────────────────────────────┤
│  > 输入框（仅聊天视图显示）                                    │  ← 底部输入栏
└─────────────────────────────────────────────────────────────┘
```

- 顶部状态栏固定 1 行
- 底部输入栏聊天视图 3 行（含边框），文件视图隐藏
- 内容区域占剩余所有空间

### 9.2 聊天视图（chat_view.rs）

**消息列表区域：**

每条消息渲染为：

```
[10:30:22] lin@arch
Hello world, 这是一条消息
```

- 时间戳用灰色，发送者名称用青色（本机）或黄色（他人）
- 系统消息（连接/断开通知）全部灰色显示
- 消息内容支持自动换行
- 消息列表支持上下滚动（`↑`/`↓` 或 `Page Up`/`Page Down`）
- 新消息到来时，若用户在底部，自动跟随最新；若用户在翻历史，保持位置并累加偏移

**输入框：**

- 支持中文输入（crossterm 处理 Unicode）
- 光标操作：`←`/`→` 移动、UTF-8 字符边界安全
- `Enter` 发送消息
- 发送后自动清空输入框

### 9.3 文件视图（files_view.rs）

**布局：左右分栏**

```
┌──────────────────────┬──────────────────────────────────────┐
│  节点列表             │  文件列表                              │
│  ● lin@arch (本机)   │  filename.txt    1.2KB   [已同步]      │
│  ● remote1           │  photo.png      45.3KB   [同步]        │
│                      │  data.zip      102.0KB   [有更新]      │
│                      │                                        │
└──────────────────────┴──────────────────────────────────────┘
```

- **左栏**：节点列表，仅显示本机和在线 peer（离线节点不显示），`●` 在线，上下方向键选择节点
- **右栏**：所选节点暴露的文件列表
- 同步状态颜色：
  - `[已同步]` → 绿色
  - `[同步]` → 蓝色（可同步）
  - `[有更新]` → 黄色
- `←`/`→` 切换左右栏焦点
- 在右栏选中文件后按 `Enter` 或 `s`，触发下载
- 按 `r` 刷新所选节点的文件列表（重新发送 `FileListRequest`）

### 9.4 键盘快捷键（全局）

| 按键 | 功能 |
|------|------|
| `Tab` | 在聊天视图和文件视图之间切换 |
| `Ctrl+C` | 退出程序 |
| `↑` / `↓` | 聊天：滚动消息 / 文件：选择节点或文件 |
| `←` / `→` | 文件视图：切换左右栏焦点 |
| `Page Up` / `Page Down` | 聊天：快速滚动（10行）/ 滚到底部 |
| `s` | 文件视图（右栏焦点）：同步选中文件 |
| `r` | 文件视图：刷新当前节点文件列表 |
| `Enter` | 聊天视图发送消息 / 文件视图（右栏焦点）触发同步 |
| `Backspace` / `Delete` | 聊天输入框：删除字符 |

---

## 十、全局状态（state.rs）

```rust
pub struct AppState {
    /// 本机节点信息
    pub local_node: NodeInfo,

    /// 所有已知 Peer
    pub peers: HashMap<String, PeerState>,   // key = node_id

    /// 聊天消息列表（系统通知 + 用户消息，按时间顺序）
    pub messages: Vec<ChatMessage>,

    /// 已处理的消息 ID（用于去重）
    pub seen_msg_ids: HashSet<String>,

    /// 每个 peer 的文件清单
    pub peer_files: HashMap<String, Vec<FileEntry>>,  // key = node_id

    /// 本地文件清单
    pub local_files: Vec<FileEntry>,

    /// 下载中的文件（文件名 set）
    pub downloading: HashSet<String>,
}

pub struct NodeInfo {
    pub node_id: String,
    pub display_name: String,
}

pub struct PeerState {
    pub node_id: String,
    pub display_name: String,
    pub online: bool,
    /// 该 peer 的 socket 地址
    pub addr: Option<SocketAddr>,
    /// 向该 peer 发送消息的通道
    pub tx: Option<mpsc::Sender<Message>>,
}

/// 聊天消息类型
pub enum ChatMessage {
    /// 系统通知，如 "lin@arch 已连接" / "lin@arch 已断开"
    System { content: String, timestamp: String },
    /// 用户聊天消息
    User(ChatPayload),
}
```

`AppState` 用 `Arc<Mutex<AppState>>` 在各 tokio task 和 TUI 主线程间共享。

### 关键方法

- `online_count()`：当前在线节点总数（本机 + online peers）
- `register_discovered_peer()`：登记新发现但尚未连接的 peer
- `mark_peer_connected()`：标记 peer 已连接，设置 tx channel
- `remove_peer()`：彻底移除 peer（含 peer_files），用于节点离线清理
- `find_node_by_addr()`：根据 socket 地址查找 peer 的 node_id

---

## 十一、并发架构

程序启动后，运行以下并发组件：

```
main()
 ├─ 日志系统初始化（tracing + panic hook → log/panic.log）
 ├─ tokio::spawn  → TCP Server task（监听随机端口，accept 循环）
 ├─ std::thread   → mDNS Discovery 线程（广播 + 监听新节点）
 ├─ tokio::spawn  → Event Handler task（处理所有 AppEvent）
 └─ TUI 主线程    → ratatui 渲染循环（处理键盘事件，每 ~16ms 刷新）
```

**Channel 设计：**

```
discovery      → app_tx: mpsc   传递"发现新节点"、"节点离线"事件
peer reader    → app_tx: mpsc   传递收到的 Message
TUI input      → app_tx: mpsc   传递"用户操作"事件（SendChat、RequestFileList、DownloadFile）
                     ↓
              Event Handler task
                     ↓
             修改 AppState（加锁）
             触发向 peer 发送消息（通过 peer 专属 tx channel）
```

**每连接内部结构：**

```
Peer 连接
 ├─ tokio::spawn → read_loop  (BufReader → 反序列化 → app_tx 发送 Message)
 └─ tokio::spawn → write_loop (peer_rx 接收 → 序列化 → TCP write)
 tokio::select! 任一退出 → 发送 PeerDisconnected
```

---

## 十二、程序启动流程

```
1. 初始化日志系统（tracing-subscriber，文件日志输出到 log/YYYY-MM-DD_HH-MM-SS.log）
2. 注册 panic hook（崩溃信息写入 log/panic.log，含 backtrace）
3. 解析命令行参数（--port, --name, --identity）
4. 加载或创建 ~/.config/lanchat/identity.json
5. 创建 ~/.local/share/lanchat/ 文件夹（如不存在）
6. 扫描本地文件列表，缓存到 AppState
7. 初始化 AppState
8. 启动 TCP Server（随机端口），获取实际端口号
9. 启动 mDNS 广播 + 发现线程，广播实际端口
10. 启动 Event Handler task
11. 启动 TUI 主循环（进入 raw mode + alternate screen）
```

---

## 十三、日志系统

- **文件日志**：输出到运行目录下 `log/` 文件夹，文件名为 `YYYY-MM-DD_HH-MM-SS.log`
- **日志级别**：INFO
- **过滤规则**：`mdns_sd` target 的日志完全静默（避免 mDNS 库噪音）
- **Panic 日志**：panic 时写入 `log/panic.log`，包含时间戳、panic 信息、位置和 backtrace
- 文件日志使用 `tracing-appender` 的非阻塞 writer

---

## 十四、错误处理原则

- 网络错误（连接断开、读写失败）：记录 tracing 日志，将 peer 标记为离线并从 AppState 中彻底移除，不崩溃
- 文件 IO 错误（读取失败、下载失败）：记录日志，跳过该文件
- JSON 解析错误：记录日志，忽略该帧，继续处理
- 程序退出时：TUI 恢复终端原始状态（disable raw mode，leave alternate screen）
- mDNS 注册失败：panic（ServiceInfo 创建失败）或 return（注册失败）

---

## 十五、注意事项

- **跨平台**：主要目标平台为 Linux 和 Windows；`crossterm` 均支持两者；`dirs::data_local_dir()` 在 Linux 返回 `~/.local/share`，在 Windows 返回 `%LOCALAPPDATA%`
- **mDNS on Linux**：`mdns-sd` 使用纯 Rust 实现，无需 Avahi
- **Windows 防火墙**：首次运行 Windows 可能提示防火墙权限，需用户允许
- **随机端口**：TCP Server 使用随机端口（`0.0.0.0:0`），通过 mDNS 广播实际端口号
- **大文件**：当前版本 base64 编码传输，不适合超过 50MB 的文件
- **多网卡**：mDNS 通过 UDP socket 探测首选 IPv4 地址进行注册
- **文件名冲突**：下载时若本地已存在同名文件（sha256 不同），保存为 `filename.conflict.ext`
- **去重连接**：使用端口号比较决定谁发起连接，避免双方同时连接
