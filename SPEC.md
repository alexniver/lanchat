
# lanchat — 局域网文本与文件共享工具
## Claude Code 启动规格文档

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
| 错误处理 | `anyhow` |
| 日志 | `tracing` + `tracing-subscriber` |
| UUID | `uuid` (v4) |
| 时间 | `chrono` |
| 目录路径 | `dirs` |

**Cargo.toml 依赖参考：**

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
ratatui = "0.28"
crossterm = "0.28"
mdns-sd = "0.11"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
dirs = "5"
```

---

## 三、目录结构

```
lanchat/
├── Cargo.toml
├── src/
│   ├── main.rs              # 入口：初始化、启动各任务
│   ├── config.rs            # 节点配置（UUID、hostname、端口）
│   ├── network/
│   │   ├── mod.rs
│   │   ├── chat/mod.rs      # chat 逻辑
│   │   ├── files/mod.rs     # files 逻辑
│   │   └── peer.rs          # 单个 Peer 连接的读写逻辑
│   │   └── discovery/mod.rs # mDNS 广播与发现
│   ├── protocol.rs          # 消息协议定义（枚举 + 序列化）
│   ├── state.rs             # 全局共享状态（AppState）
│   └── tui/
│       ├── mod.rs           # TUI 主循环
│       ├── app.rs           # UI 状态机
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
- `display_name`：默认为 `username@hostname`，用户可手动修改

**TCP 监听端口**：默认 `47731`，可通过 `--port` 命令行参数覆盖。

`.lanchat/` 文件夹：默认在**程序运行目录**下创建（`./lanchat/`），不污染 home 目录。

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
    pub version: String,         // "0.1.0"
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
    // 若文件不存在，data 为空字符串，且 not_found = true
    pub not_found: bool,
}
```

> **注意**：文件传输使用 base64 而非二进制流，简化 JSON 帧边界处理。对于大文件（> 50MB），
> 可在后续版本改为独立的二进制传输通道，当前版本不做处理（超大文件传输会很慢，但功能正确）。

### 5.3 传输格式

- 每条消息序列化为 JSON，后跟 `\n`
- 接收端按行读取，每行反序列化为一条消息
- 使用 `tokio::io::BufReader` + `lines()` 处理

---

## 六、节点发现（discovery.rs）

使用 `mdns-sd` 进行局域网服务广播与发现。

- **服务类型**：`_lanchat._tcp.local.`
- **服务实例名**：`{node_id}._lanchat._tcp.local.`
- **TXT 记录**：`node_id={uuid}`, `name={display_name}`
- **端口**：与 TCP 监听端口相同

**行为：**

1. 程序启动时立刻广播自身服务
2. 同时注册 Browse，监听其他 `_lanchat._tcp.local.` 服务出现/消失事件
3. 发现新节点时，主动发起 TCP 连接（若该节点 node_id 尚未连接）
4. 节点离线时（mDNS remove 事件），从 peers 列表中标记为离线

**去重规则**：以 `node_id` 为唯一键，避免重复连接。自身的 `node_id` 不发起连接。

---

## 七、网络层（network/）

### 7.1 连接管理

- 每个 Peer 连接启动两个 tokio task：**读取 task** 和 **写入 task**
- 读取 task 将收到的消息发送到全局的 `mpsc` channel
- 写入 task 从各 Peer 专属的 `mpsc` channel 接收，向对端发送消息

### 7.2 消息广播

- 聊天消息由发送方广播给**所有已连接 peer**
- 消息用 `msg_id`（UUID）去重：如果本节点已处理过某 `msg_id`，忽略（为未来 P2P 转发预留）

### 7.3 握手流程

```
A ──────── Hello ──────────► B
A ◄─────── Hello ────────── B
（握手完成，可开始正常通信）
```

握手完成后，双方互相请求文件清单：

```
A ──── FileListRequest ────► B
A ◄─── FileListResponse ─── B
```

---

## 八、文件系统（files.rs）

### 8.1 .lanchat 文件夹

- 路径：`~/.local/share/lanchat/`（启动时自动创建，使用 `dirs` crate 获取 data_local_dir）
- lanchat 只能**读取**该文件夹，不自动写入（下载到此文件夹由用户触发）
- 下载的文件保存到同一个 `~/.local/share/lanchat/` 文件夹

### 8.2 本地 Manifest

扫描 `~/.local/share/lanchat/` 文件夹，为每个文件生成 `FileEntry`：

```rust
pub fn scan_local_files(dir: &Path) -> anyhow::Result<Vec<FileEntry>>
```

SHA256 计算在首次扫描时进行，结果缓存在内存中（不持久化到磁盘）。

### 8.3 "未同步"判断

对于远端节点的每个 `FileEntry`，判断本地是否已有同步版本：

- 本地 `~/.local/share/lanchat/` 中**不存在同名文件** → 未同步
- 存在同名文件但 **sha256 不同** → 已修改（显示为"有更新"状态）
- 存在同名文件且 **sha256 相同** → 已同步

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
- 消息内容支持自动换行
- 消息列表支持上下滚动（`↑`/`↓` 或 `Page Up`/`Page Down`）
- 新消息到来时，若用户当前在最底部，自动滚动到最新；若用户在滚动历史，不自动跳转

**输入框：**

- 支持中文输入（crossterm 处理 Unicode）
- `Enter` 发送消息
- `Esc` 清空输入框

### 9.3 文件视图（files_view.rs）

**布局：左右分栏**

```
┌──────────────────────┬──────────────────────────────────────┐
│  节点列表             │  文件列表                              │
│  ● lin@arch (本机)   │  filename.txt    1.2KB   [已同步]      │
│  ● remote1           │  photo.png      45.3KB   [同步]        │
│  ○ remote2 (离线)    │  data.zip      102.0KB   [有更新]      │
│                      │                                        │
└──────────────────────┴──────────────────────────────────────┘
```

- 左栏：节点列表，`●` 在线，`○` 离线，上下方向键选择节点
- 右栏：所选节点暴露的文件列表（来自最近一次 FileListResponse）
- 右栏每行显示：文件名、大小、同步状态
- 同步状态颜色：
  - `[已同步]` → 绿色
  - `[同步]` → 蓝色（可同步）
  - `[有更新]` → 黄色
- 在右栏选中文件后按 `Enter` 或 `s`，触发下载
- 下载中显示 `[下载中...]`，完成后更新为 `[已同步]`
- 按 `r` 刷新所选节点的文件列表（重新发送 FileListRequest）

### 9.4 键盘快捷键（全局）

| 按键 | 功能 |
|------|------|
| `Tab` | 在聊天视图和文件视图之间切换 |
| `q` 或 `Ctrl+C` | 退出程序（发送 Goodbye，优雅关闭） |
| `↑` / `↓` | 滚动消息 / 选择文件或节点 |
| `Page Up` / `Page Down` | 消息列表快速滚动 |
| `s` | 文件视图：同步选中文件 |
| `r` | 文件视图：刷新文件列表 |
| `Enter` | 聊天视图发送消息 / 文件视图触发同步 |
| `Esc` | 聊天视图清空输入框 |

---

## 十、全局状态（state.rs）

```rust
pub struct AppState {
    /// 本机节点信息
    pub local_node: NodeInfo,

    /// 所有已知 Peer（含离线）
    pub peers: HashMap<String, PeerState>,   // key = node_id

    /// 聊天消息列表（按时间顺序）
    pub messages: Vec<ChatPayload>,

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
    /// 向该 peer 发送消息的通道
    pub tx: Option<mpsc::Sender<Message>>,
}
```

`AppState` 用 `Arc<Mutex<AppState>>` 在各 tokio task 和 TUI 主线程间共享。

---

## 十一、并发架构

程序启动后，运行以下并发组件：

```
main()
 ├─ tokio::spawn  → TCP Server task（监听传入连接）
 ├─ tokio::spawn  → mDNS Discovery task（广播 + 监听新节点）
 ├─ tokio::spawn  → Event Handler task（处理所有传入的 Message）
 └─ TUI 主线程   → ratatui 渲染循环（处理键盘事件，每 ~16ms 刷新）
```

**Channel 设计：**

```
discovery      → app_tx: mpsc   传递"发现新节点"事件
peer reader    → app_tx: mpsc   传递收到的 Message
TUI input      → app_tx: mpsc   传递"用户操作"事件
                     ↓
              Event Handler task
                     ↓
             修改 AppState（加锁）
             触发向 peer 发送消息（通过 peer 专属 tx）
```

---

## 十二、程序启动流程

```
1. 解析命令行参数（--port, --name）
2. 加载或创建 ~/.config/lanchat/identity.json
3. 创建 ~/.local/share/lanchat/ 文件夹（如不存在）
4. 扫描本地 ~/.local/share/lanchat/ 文件列表，缓存到内存
5. 初始化 AppState
6. 启动 TCP Server，开始监听
7. 启动 mDNS 广播 + 发现
8. 启动 Event Handler task
9. 启动 TUI 主循环（进入 raw mode）
```

---

## 十三、错误处理原则

- 网络错误（连接断开、读写失败）：记录 tracing 日志，将 peer 标记为离线，不崩溃
- 文件 IO 错误（读取失败、下载失败）：在 TUI 底部状态栏显示短暂的错误提示
- JSON 解析错误：记录日志，忽略该帧，继续处理
- 程序退出时：先向所有在线 peer 发送 `Goodbye`，等待 200ms，再关闭连接

---

## 十四、实现顺序建议（分阶段）
**Phase 1 - Tui**
1. `tui/app.rs`：UI 状态机（视图切换、选中状态）
2. `tui/chat_view.rs`：聊天界面渲染
3. `tui/files_view.rs`：文件界面渲染
4. `tui/mod.rs`：主循环、键盘事件、crossterm 集成
5. `main.rs`：main.rs启动时，加载Tui

**Phase 2 - 日志**
所有日志以及崩溃信息，写入程序运行目录的 log文件夹下，文件名为 当前日期时间.log

**Phase 3 - config**
`config.rs`：identity 加载/生成

**Phase 4 - 广播 发现 连接**
`network/discovery`：mDNS 广播 + 发现 + 自动连接, 并更新tui界面的连接数
程序启动之后，新启动一个线程，间隔1秒循环，广播 + 发现 + 建立连接
`network/discovery/broadcast.rs` 广播
`network/discovery/discover.rs` 发现
`network/discovery/do_connect.rs` 连接逻辑

`network/server.rs` 本地server, 启动时, 启动server, 使用随机端口, 同时广播此server + 端口
`network/client.rs` 本地client, 当发现有新的server时，连接到这个server, 连接成功之后，新server对应的client也要与本地server连接

当某个chat下线之后，清理 client server app_state中所有相关的信息


**Phase 5 - 聊天实现**
`network/chat`：聊天实现 
当发现新的连接时，在聊天框显示：xxx已连接， 当有连接退出时，显示：xxx已退出

**Phase 6 - 文件功能**
1. `network/files`：扫描、sha256、FileListRequest/Response 处理
2. 文件下载：FileRequest/Response 处理 + base64 encode/decode


---

## 十五、注意事项

- **跨平台**：主要目标平台为 Linux（Arch）和 Windows；`crossterm` 均支持两者；`dirs::data_local_dir()` 在 Linux 返回 `~/.local/share`，在 Windows 返回 `%LOCALAPPDATA%`，天然跨平台
- **mDNS on Linux**：需要确认 Avahi 不冲突；`mdns-sd` 使用纯 Rust 实现，无需 Avahi
- **Windows 防火墙**：首次运行 Windows 可能提示防火墙权限，需用户允许
- **大文件**：当前版本 base64 编码传输，不适合超过 50MB 的文件，超大文件显示警告
- **多网卡**：mDNS 默认绑定所有接口，TCP Server 监听 `0.0.0.0`
- **文件名冲突**：下载时若本地已存在同名文件（sha256 不同），保存为 `filename.conflict.ext`
