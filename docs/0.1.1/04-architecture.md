# 0.1.1 网络架构

> 状态：已实现
> 基线：0.1.0 的单所有者 `App` + `Vec<Layer>` 架构
> 关联：[02-network.md](02-network.md) · [03-ui.md](03-ui.md) · [01-scope.md](01-scope.md)

## 1. 目标

在不破坏 0.1.0 的前提下，把网络能力接到现有事件循环：

- 网络层只发事件、收命令，不直接修改 `App`。
- 远端消息复用现有 `Message`/历史/选中逻辑。
- 连接/心跳/重连在独立 actor 中，不阻塞 UI。
- 0.1.0 的本地测试继续通过。

## 2. 新增模块

```text
src/
├── net/
│   ├── mod.rs          # NetHandle、NetCommand、NetEvent
│   ├── manager.rs      # 网络 actor：peers、连接生命周期
│   ├── discovery.rs    # mDNS 注册/浏览（独立线程）
│   ├── peer.rs         # 单连接读写、心跳
│   ├── protocol.rs     # 帧、Hello/Ready/Chat/Ping/Pong/Bye/Error
│   └── reconnect.rs    # 退避/抖动
├── app/
│   ├── peers.rs        # PeerView、ConnectionState
│   └── ...             # 现有 0.1.0 结构
└── ui/
    └── members.rs      # 右侧成员侧边栏（新 Layer）
```

## 3. 事件循环集成

0.1.0 的主循环：

```text
EventStream -> App
tick        -> App
history_rx  -> App
```

0.1.1 增加：

```text
net_rx      -> App::handle_net_event
```

`App` 产生 `Effect::Net(NetCommand)`，由主循环或 `NetHandle` 执行：

```rust
loop {
    if app.dirty() { terminal.draw(...)?; app.clear_dirty(); }

    tokio::select! {
        event = events.next() => app.handle_terminal(event),
        _ = tick.tick() => app.on_tick(Instant::now()),
        error = history_error_rx.recv() => app.on_history_error(error),
        event = net_rx.recv() => app.handle_net_event(event),
    }

    for effect in app.take_effects() {
        match effect {
            Effect::Persist(message) => app.persist_message(&message),
            Effect::Copy { text } => app.on_clipboard_result(app.copy_text(&text)),
            Effect::Net(command) => net_handle.send(command)?,
        }
    }
}
```

## 4. 网络 actor

```rust
pub struct NetHandle {
    tx: mpsc::Sender<NetCommand>,
}

pub enum NetCommand {
    SendChat { msg_id: String, body: String },
    Connect { addr: SocketAddr },
    Disconnect { node: Option<String> },
    Reconnect { node: Option<String> },
    Shutdown,
}

pub enum NetEvent {
    PeerUp { node_id: String, name: String, addr: SocketAddr },
    PeerDown { node_id: String, reason: String },
    ChatReceived(ChatMessage),
    SendFailed { msg_id: String, reason: String },
    Status { state: ConnectionState, detail: String },
}
```

- `net::start(config) -> Result<(NetHandle, mpsc::Receiver<NetEvent>)>`。
- manager task 独占 peers/sockets；每 peer 的 reader/writer 是子 task。
- mDNS 线程通过 `blocking_send` 发 discovery 事件到 manager。
- 有界 channel；发送用 `send().await`，队列满时断开慢 peer 并上报，不 `try_send`。
- 退出时 manager 发送 `Bye`、关闭 socket、join。

## 5. App 状态扩展

```rust
pub struct App {
    // 0.1.0 字段
    pub messages: Vec<Message>,
    pub layers: Vec<Layer>,
    pub status: Option<Status>,
    // 0.1.1
    pub peers: BTreeMap<String, PeerView>,
    pub connection: ConnectionState,
    pub unread: usize,           // 手动模式下收到但未跳转的消息数
}
```

- `PeerView`：node_id、name、addr、state、last_seen。
- `ConnectionState`：Local / Connecting / Online / Reconnecting / Offline。
- `Layer` 增加 `Members(MemberState)`：右侧成员侧边栏，`Space u` 开关。
- 不新增全局 `Arc<Mutex>`；网络状态由 manager 发事件，App 单线程更新。

## 6. 消息模型与历史

0.1.0：

```json
{"v":1,"id":"...","room":"lobby","name":"...","timestamp":"...","body":"..."}
```

0.1.1 增加可选 `from` 与 `sent_at`：

```json
{"v":1,"id":"...","room":"lobby","from":"node-uuid","name":"...","timestamp":"...","sent_at":"...","body":"..."}
```

- `from` 可选；旧记录缺省为空，仍能加载。
- `timestamp` 使用本地接收时间（显示时间）；`sent_at` 保留发送方时间（可选）。
- 本地消息 `from = local.node_id` 或空；远端消息写真实 node_id。
- 远端消息 `timestamp = received_at`，`sent_at = chat.sent_at`；显示用 `timestamp`。
- 历史加载后仍按原顺序显示；缺 `from` 不报错。
- 不改变消息格式 `[name][datetime]: message`。

## 7. 远端消息处理

```text
NetEvent::ChatReceived(chat)
  → 校验/去重（manager 或 App）
  → MessageStore.push(chat)
  → Effect::Persist(message)
  → if list.follow_latest { selected = last; scroll bottom }
    else { selected 不变; unread += 1; status "有新消息" }
```

- 去重可在 manager 做（10k LRU），也可在 App 用 `HashSet/LRU`；建议 manager 做，App 不重复实现。
- 本地发送仍先插入本地消息，再发 `NetCommand::SendChat`；失败 `SendFailed` 标记。
- 手工选择模式语义与 0.1.0 一致；不因网络改变。

## 8. 发现与重连

- `discovery.rs`：`ServiceDaemon` 注册/browse；事件转 `PeerDiscovered/PeerVanished`；自身按 node_id 过滤。
- `manager.rs`：peer 状态机 Discovered → Connecting → Handshaking → Online → Reconnecting/Offline。
- `reconnect.rs`：1s→30s 退避 + 抖动；成功重置；手动断开停止。
- 重复连接去重按 [02-network.md](02-network.md) 第 6 节。
- mDNS 线程不碰 `App`；只发 manager 事件。

## 9. 配置

```toml
[network]
enabled = true
port = 47731
mdns = true
reconnect = true

[network]
seeds = ["192.168.1.23:47731"]
```

- CLI：`--port`、`--connect`、`--no-network`（建议）、`--config`。
- 0.1.0 配置缺 `[network]` → 使用默认。
- `--no-network` 可完全退回 0.1.0 本地模式，便于回归/排障。

## 10. 依赖

新增（建议）：

| crate | 用途 |
| --- | --- |
| `mdns-sd` | mDNS 注册/浏览 |
| `uuid` | 已有；msg_id/node_id |
| `serde_json` | 已有；协议/历史 |

不引入：文件传输、sha2、加密库、HTTP/WebSocket、NAT 穿透。

## 11. 测试

| 层级 | 内容 |
| --- | --- |
| 单元 | 协议帧 roundtrip/坏帧/超限；握手版本/重复身份；去重；退避 |
| 集成 | 两个 `NetManager` 通过 localhost 连接、聊天、断开、重连、重复连接 |
| App | 假 `NetEvent`：自动/手动选中、远端消息入历史、SendFailed、PeerUp/Down |
| 回归 | 0.1.0 的 57 个测试全部通过 |
| 手工 | 两机 mDNS、手动 `--connect`、断网恢复、tmux/SSH |

- CI 不依赖 mDNS；用 localhost/手动连接。
- 协议模糊测试可后续加。

## 12. 风险

| 风险 | 应对 |
| --- | --- |
| 网络 actor 死锁/泄漏 | 有界 channel、超时、shutdown、集成测试 |
| mDNS 环境不可用 | 手动连接兜底；UI 提示 |
| 重复连接造成双份消息 | node_id 去重 + 握手 nonce + 测试 |
| 远端消息打断手动浏览 | 严格 0.1.0 自动/手动规则 |
| 历史格式不兼容 | `from` 可选 + 旧记录默认 |
| 范围膨胀（文件/频道/私聊） | 0.1.1 范围冻结；新需求进后续版本 |
| 明文安全 | README/帮助明确“仅可信局域网” |

## 13. 已确认

- 成员面板作为右侧 `Layer::Members`，`Space u` 开关，`j/k` 选择，`Esc` 关闭。
- 系统加入/离开只走状态/toast。
- 远端消息显示本地接收时间；历史保存 `timestamp` + 可选 `sent_at`/`from`。
- `--no-network`、`Space r`/`Space d` 入口。
- 0.1.1 坚持全 mesh、不做中继。
