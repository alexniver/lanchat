# 0.2 网络与协议（预留设计）

> 状态：**0.2 预留设计**；0.1.0 只做单机，不实现本文内容。
> 参见 [decisions.md](decisions.md) D-006。
> 关联：[04-architecture.md](04-architecture.md) · [01-scope.md](01-scope.md)

## 1. 范围与假设

0.2 的联网目标：同一局域网内、可互相 TCP 直连的 lanchat 实例，自动发现并交换文本消息。

假设：

- 节点在同一广播域/子网，mDNS 多播可用；不可用时手动地址兜底。
- 节点之间没有 NAT；防火墙允许入站 TCP 端口。
- 节点数 ≤ 64；单房间；消息以文本为主，单条 ≤ 16 KiB。
- 时钟可能不同步；不保证跨节点严格全序。
- 局域网被视为可信环境；0.2 明文、无鉴权。

## 2. 拓扑

采用**全连接 mesh**：

```text
      A ───────── B
      │ \       / │
      │   \   /   │
      │     X     │
      │   /   \   │
      C ───────── D
```

- 每个节点与所有已知在线节点建立一条 TCP 连接。
- 发送消息时直接发给所有在线 peer；接收方不转发（避免环路和重复）。
- 如果 mesh 不完整（A 连不上 B，但都能连 C），0.2 **不保证** A/B 互通；UI 显示连接状态。中继/转发放后续版本。
- 单房间 `#lobby`；协议保留 `room` 字段，为多频道预留。

## 3. Peer 状态机

```text
Discovered ──connect──► Connecting ──tcp ok──► Handshaking ──hello ok──► Online
     ▲                       │                     │                      │
     │                       │ timeout/error       │ timeout/version      │ idle/error
     │                       ▼                     ▼                      ▼
     └─────────────────── Reconnecting ◄────────── Offline ◄──────────────┘
                              │
                              └── backoff ──► Connecting
```

| 状态 | UI | 行为 |
| --- | --- | --- |
| Discovered | 成员列表灰色 | 等待连接调度 |
| Connecting | `◌ connecting` | TCP connect，3s 超时 |
| Handshaking | `◌ handshake` | 交换 Hello，5s 超时 |
| Online | `● online` | 可收发；心跳 |
| Degraded | `! degraded` | 心跳延迟/写失败，准备重连 |
| Reconnecting | `◌ reconnecting` | 指数退避 |
| Offline | `○ offline` | 手动断开或放弃；保留历史 |

- `:disconnect` 将状态置为 Offline，并取消自动重连；`:reconnect` 清除手动断开标记。
- mDNS `ServiceRemoved` 只标记 offline，不删除历史；重新出现时重新连接。
- 进程退出发送 `Bye`（尽力），随后关闭 socket。

## 4. 发现（mDNS）

### 4.1 服务定义

| 项 | 值 |
| --- | --- |
| 服务类型 | `_lanchat._tcp.local.` |
| 实例名 | `node_id`（UUID，避免重名） |
| 主机名 | `<hostname>.local.` |
| 端口 | TCP 监听端口（默认 47731） |
| TXT | `v=1`、`id=<node_id>`、`name=<display_name>`、`proto=1` |

- 用 **node_id** 过滤自身，不用 IP+端口（旧版方式在多网卡/端口复用时不可靠）。
- TXT 值做长度限制；昵称中的换行/控制字符先清理。
- 地址：注册本机非回环地址；多网卡时优先与默认路由同网段的 IPv4，其次其他 IPv4/IPv6。
- mDNS 线程：`mdns-sd::ServiceDaemon`，注册 + browse；事件通过 `tokio::sync::mpsc::Sender::blocking_send` 送入网络 manager。
- 发现事件：
  ```rust
  NetEvent::PeerDiscovered { node_id, name, addrs, port, source: Mdns }
  NetEvent::PeerVanished { node_id }
  ```
- 同一节点多次 `ServiceResolved` 去重；地址变化时更新候选地址并触发重连。
- 自身服务过滤：`id == local.node_id`；不要只按 IP/端口。

### 4.2 手动发现

- CLI：`--connect host:port`（可重复）。
- 配置：`[network] seeds = ["192.168.1.23:47731"]`。
- 命令：`:connect host:port`。
- 手动节点标记 `source: Manual`，不因 mDNS 消失而移除；是否自动重连由配置决定（默认是）。
- 解析支持 IPv4/IPv6：`host:port`、`[::1]:47731`；不允许裸端口（避免歧义）。

### 4.3 mDNS 失败降级

- mDNS 注册失败：记录错误，UI 提示“自动发现不可用”，仍允许手动连接。
- mDNS browse 失败：同上；不 panic。
- 容器/CI 中 mDNS 可能不可用：测试用手动连接，不用 mDNS。

## 5. 连接与握手

### 5.1 TCP 连接

- 默认监听 `0.0.0.0:47731`（IPv4）；可选 `--port 0` 随机（测试）。
- 端口占用：启动失败并给出明确错误（建议不自动顺延，避免 mDNS/防火墙混乱；见 OQ-12）。
- connect 超时 3s；握手超时 5s；写超时 5s（或使用 `tokio::time::timeout` 包裹写）。
- `TCP_NODELAY` 开启，降低聊天延迟。

### 5.2 握手

双方在 TCP 建立后**立即互发** Hello（不等待对方），避免谁先发的歧义：

```json
{
  "type": "hello",
  "magic": "lanchat",
  "protocol": 1,
  "node_id": "uuid",
  "name": "alice@arch",
  "version": "0.2.0",
  "capabilities": ["chat"],
  "nonce": "random-16-bytes-hex"
}
```

- 第一帧必须是 Hello；否则关闭并记录。
- 校验：
  - `magic == "lanchat"`；
  - `protocol` 主版本兼容（protocol 1；0.2 只接受主版本 1）；
  - `node_id` 合法 UUID、非空；
  - `name` 长度/字符合法；
  - `node_id != local.node_id`（相同则拒绝，提示“身份重复”）。
- 不兼容：发送 `Error { code: "version_mismatch" }` 后关闭；UI 显示对端版本。
- 双方能力取交集；protocol 1 只要求 `chat`。
- 握手成功后：

```text
A ── Hello ──► B
A ◄─ Hello ─── B
A ── Ready ──► B   （可选，确认双方已就绪）
```

> 建议加 `Ready`：双方 Hello 校验通过后发送，收到对方 Ready 才标记 Online；避免一方还在校验时收到 Chat。

### 5.3 重复连接处理

同一 node_id 可能出现两条连接（双方同时发起、或 mDNS 重复触发）。规则：

1. **主发起方**：`node_id` 字典序较小的一方负责主动连接；较大方等待。
2. **兜底**：较大方等待 `duplicate_grace`（2s）仍无连接时，也可发起；握手后去重。
3. **去重**：manager 发现同一 node_id 已有一条 Online 连接时：
   - 优先保留“由较小 node_id 发起”的那条；
   - 若无法判定（两条都是同一方发起/同时），比较握手 `nonce` 或 socket 四元组，保留字典序较小者；
   - 另一条发送 `Bye { reason: "duplicate" }` 并关闭；
   - 只向 `App` 发一次 `PeerUp`，避免双份消息。
4. **身份重复**（两个进程用同一 node_id）：拒绝连接，`Error { code: "duplicate_identity" }`；UI 警告“node_id 冲突，请删除/更换身份文件”。

> 旧版用“端口号比较”决定谁连接，端口变化/重连时会失效；新方案以 node_id 为准，端口只用于连接。

## 6. 帧格式

### 6.1 长度前缀 + JSON

```text
┌──────────────┬───────────────────────────┐
│ u32 BE length│ JSON UTF-8 payload        │
└──────────────┴───────────────────────────┘
```

- `length` 只计 payload 字节数，不含 4 字节头。
- 最大帧 64 KiB；Hello 最大 4 KiB；Chat body 最大 16 KiB。
- 用 `tokio_util::codec::LengthDelimitedCodec`：
  - `max_frame_length(64 * 1024)`；
  - 大端 4 字节长度（默认）；
  - 解码失败：记录错误并关闭连接（帧边界已丢失，不能继续）。
- JSON 用 `serde` + `#[serde(tag = "type", rename_all = "snake_case")]`。
- 不使用 JSON lines：消息正文可能包含换行（虽然 JSON 会转义），长度前缀更易设上限、避免“无换行导致内存增长”。

### 6.2 帧类型

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WireMessage {
    Hello(Hello),
    Ready(Ready),
    Chat(Chat),
    Ping(Ping),
    Pong(Pong),
    Bye(Bye),
    Error(ProtocolError),
}
```

| 类型 | 方向 | 说明 |
| --- | --- | --- |
| `hello` | 双向，首帧 | 版本、身份、能力、nonce |
| `ready` | 双向 | 握手完成确认 |
| `chat` | 双向 | 聊天消息 |
| `ping` | 双向 | 心跳，带 nonce |
| `pong` | 双向 | 心跳响应，回显 nonce |
| `bye` | 双向 | 主动断开，带原因 |
| `error` | 双向 | 协议错误，带 code/message |

### 6.3 Chat 载荷

```json
{
  "type": "chat",
  "msg_id": "uuid",
  "room": "lobby",
  "from": "uuid",
  "name": "alice@arch",
  "seq": 42,
  "sent_at": "2026-11-19T10:30:00+08:00",
  "body": "hello"
}
```

- `msg_id`：发送方生成 UUID v4；用于去重。
- `room`：0.2 固定 `"lobby"`；未知 room 丢弃并记录。
- `from`：发送方 node_id；接收方校验与握手 node_id 一致（防止伪造）。
- `name`：发送方昵称；可与握手名不同（`:nick` 后更新），但长度/字符受限。
- `seq`：发送方每连接/每进程单调递增；用于检测乱序/缺失（0.2 只记录，不重传）。
- `sent_at`：发送方本地时间 RFC3339；接收方另存 `received_at`。
- `body`：纯文本，长度按 UTF-8 字节 ≤ 16 KiB；禁止 NUL/控制字符（换行/制表符允许）。

## 7. 聊天语义

### 7.1 发送

1. 用户在 Compose 层按 Enter 发送。
2. `App` 生成 `msg_id`，插入本地消息（状态 `Sending`），产生 `Effect::SendChat`。
3. 网络 manager 将 Chat 帧写入所有 Online peer 的 writer channel。
4. 写入成功：可选标记 `Sent`；写入失败：`NetEvent::SendFailed { msg_id, reason }`，UI 标记 `!` 并提示。
5. 没有在线 peer：提示“当前无在线节点”，本地消息仍保留并标记未送达（不静默）。

### 7.2 接收

1. reader 解码 Chat 帧。
2. 校验 `from == peer.node_id`、`room == "lobby"`、`body` 长度、`msg_id` 格式。
3. 去重：`msg_id` 已在 10,000 条 LRU 中 → 丢弃。
4. 插入消息；记录 `received_at`、`peer_id`；按 List 选择模式处理：自动模式选中最新/滚到底，手动模式保持选中/滚动。
5. 发 `NetEvent::ChatReceived` 给 `App`；自动/手动模式规则与 0.1.0 本地消息一致。
6. 不转发（全 mesh）。

### 7.3 去重

- 每个节点维护 `LruCache<MsgId, ()>`，容量 10,000；覆盖重连后的重复投递。
- 本地发送的消息也插入去重表，避免回环时重复显示。
- 去重表不持久化；重启后极端重复（对端重发）可能显示两次，0.2 接受。若做离线队列/ACK，再持久化。

### 7.4 顺序

- 同一发送方：TCP 保序 + `seq`；正常按序。
- 不同发送方：按本地 `received_at` 排序显示；不保证与发送方时钟一致。
- `seq` 缺口：记录 warning，不阻塞后续消息；后续可请求补发。
- 时间显示：默认 `received_at`（见 OQ-09）；`sent_at` 存历史。

### 7.5 系统消息

- 本地生成：`PeerUp` → “alice 加入”；`PeerDown` → “alice 离开/连接断开”；不进网络、不广播。
- 不写入历史？建议写入本地系统消息（与聊天同文件，`kind: "system"`），便于回看；0.2 可选。

## 8. 心跳与超时

| 项 | 值 | 说明 |
| --- | --- | --- |
| Ping 间隔 | 10s | 每个 Online peer |
| Pong 超时 | 5s | 未回则标记 Degraded |
| Idle 超时 | 30s | 无任何帧 → 关闭并重连 |
| Connect 超时 | 3s | TCP connect |
| Handshake 超时 | 5s | 首帧 Hello + Ready |
| 写超时 | 5s | 单帧写入 |
| 关闭等待 | 1s | 发送 Bye 后等待，避免 RST |

- 心跳由 manager/peer task 驱动；不在 UI 线程。
- `Ping/Pong` 不进入 `App`，只在网络层处理；UI 只看到状态变化。
- 网络切换/休眠恢复：心跳失败 → 进入 Reconnecting；恢复后按退避重连。

## 9. 重连

- 退避：`base=1s`，`factor=2`，`max=30s`，`jitter=±20%`。
- 触发：连接错误、握手失败、心跳超时、写失败、mDNS 地址更新。
- 重置：握手成功（Online）后重置退避。
- 停止：`:disconnect` 或退出；手动断开不自动重连。
- 重连期间：
  - `App` 保留 peer 和历史，状态显示 `◌ reconnecting`；
  - 不重发旧消息（0.2 无离线队列）；
  - 新消息发送时若无 Online peer，标记未送达。
- 重连成功后：
  - 重新握手、去重；
  - 不交换历史（OQ-15）；
  - 更新地址/昵称；
  - UI 显示“已重连”。

## 10. 手动连接与多实例

- `--connect host:port` 可重复；`:connect` 运行时添加。
- 本机测试多实例：
  - 不同 `--port`（如 47731/47732）；
  - 不同配置目录 `--config-dir`（避免身份/历史冲突）；0.2 建议支持；
  - 手动 `--connect 127.0.0.1:47732`。
- 同一 node_id 多实例：拒绝连接并提示；测试时必须用不同配置目录。

## 11. 限制与滥用防护

| 项 | 限制 | 处理 |
| --- | --- | --- |
| 帧大小 | 64 KiB | 解码失败/超限 → 关闭连接 |
| Chat body | 16 KiB | 发送端拒绝；接收端丢弃并记录 |
| 昵称 | 64 字符/256 字节 | 截断/拒绝 |
| room | 32 字节 | 未知 → 丢弃 |
| peers | 64 | 超出拒绝/忽略新节点并提示 |
| 消息速率 | 20 条/秒/peer（建议） | 超出丢弃并警告；持续滥用断开 |
| 历史/去重 | 10,000 条 | LRU |
| 连接数 | 每节点 ≤ 64 | 超出拒绝 |

- 所有限制在协议文档和代码中集中定义，避免散落。
- 解析失败只影响当前连接；不允许 panic/无限内存增长。

## 12. 安全边界

0.2 **不提供**：

- 加密（消息明文）；
- 身份认证（node_id/name 可伪造）；
- 前向保密；
- 访问控制（同网段任何人都能连）。

风险与缓解：

- 文档/`:help security` 明确“仅限可信局域网”；
- 不记录消息正文到日志；
- 历史文件 `0600`；
- 校验 `from` 与握手 node_id 一致，减少简单伪造；
- 限制帧/消息大小，避免内存 DoS；
- 未来（0.3+）：预共享密钥握手、Noise/TLS、签名身份。

## 13. 不做

- 文件传输（0.1.0 已删除，0.2 也不做）；
- 中继/转发；
- 离线消息队列/ACK；
- 历史同步；
- NAT 穿透；
- 多频道/私聊；
- 消息编辑/撤回；
- 端到端加密。

## 14. 测试

| 测试 | 内容 |
| --- | --- |
| 协议 roundtrip | 每种帧序列化/反序列化；未知字段忽略；坏 JSON |
| 帧边界 | 超长帧、截断帧、长度 0、负长度（u32 溢出）、粘包/半包 |
| 握手 | 版本不匹配、magic 错、重复 node_id、超时、Ready 缺失 |
| 去重 | 同一 msg_id 多次接收；本地回显；重连重复 |
| 心跳 | Ping/Pong、丢 Pong、Idle 超时 |
| 重连 | 退避序列、jitter 范围、成功重置、手动断开不重连 |
| 重复连接 | 双方同时连接、保留规则、只发一次 PeerUp |
| 多 peer | 3–5 个 manager 互相连接、广播、断开 |
| 大消息 | 16 KiB 边界、超限拒绝 |
| 安全 | 伪造 from、超长 name/body、控制字符 |

- 集成测试用 `127.0.0.1:0` 和手动连接，不依赖 mDNS。
- mDNS 用手工/可选集成测试；CI 不阻塞。
- 建议后续加 `proptest`/fuzz对帧解析做随机输入。

## 15. 待确认

| ID | 问题 | 推荐 |
| --- | --- | --- |
| NET-01 | 帧格式：长度前缀 vs JSON lines | 长度前缀（64 KiB 上限） |
| NET-02 | 是否 0.2 做 mDNS | 做；手动连接兜底 |
| NET-03 | 是否允许中继 | 不做；全 mesh |
| NET-04 | 离线消息/ACK | 不做；只保证在线 |
| NET-05 | 历史同步 | 不做 |
| NET-06 | 端口固定还是随机 | 固定 47731，`--port 0` 测试 |
| NET-07 | 重复连接判定 | node_id 小者主发起 + 2s 兜底 + 握手去重 |
| NET-08 | IPv6 | 尽力；IPv4 优先 |
| NET-09 | 协议版本 | 0.2 = protocol 1；主版本不兼容拒绝 |
| NET-10 | 心跳参数 | 10s/5s/30s；可配置 |
