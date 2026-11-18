# 0.1.1 网络与协议

> 状态：已实现，验收中
> 关联：[01-scope.md](01-scope.md) · [03-ui.md](03-ui.md) · [04-architecture.md](04-architecture.md)
> 历史参考：[0.1.0 网络预留草案](../requ/0.1.0/05-network.md)（以本文为准）

## 1. 目标与假设

0.1.1 在局域网内把文本消息从“本机”扩展到“多节点”：

- 同局域网自动发现（mDNS）；手动地址兜底。
- 单房间 `#lobby`；全连接 mesh。
- 明文 TCP；不做文件传输、历史同步、离线补发。
- 节点数 ≤ 64；消息以文本为主，单条 ≤ 16 KiB。

假设：

- 节点在同一广播域/子网，mDNS 多播可用；不可用时手动连接。
- 节点间 TCP 可直连；无 NAT/中继。
- 局域网被视为可信环境；0.1.1 无加密/鉴权。
- 节点时钟可能不一致；显示时间已确认使用本地接收时间（`received_at`）。

## 2. 拓扑

全连接 mesh：

```text
      A ───────── B
      │ \       / │
      │   \   /   │
      │     X     │
      │   /   \   │
      C ───────── D
```

- 每个节点与所有已知在线节点建立一条 TCP 连接。
- 发送方直接发给所有 Online peer；接收方不转发。
- mesh 不完整时（A 连不上 B，但都能连 C），0.1.1 **不保证** A/B 互通；状态/成员面板显示连接情况。
- 中继/转发/离线队列不在 0.1.1。

## 3. 发现

### 3.1 mDNS

| 项 | 值 |
| --- | --- |
| 服务类型 | `_lanchat._tcp.local.` |
| 实例名 | `node_id`（UUID） |
| TXT | `v=1`、`id=<node_id>`、`name=<display_name>`、`proto=1` |
| 端口 | TCP 监听端口（默认 47731） |

- 自身过滤用 **node_id**，不用 IP+端口。
- 解析 `ServiceResolved` → `PeerDiscovered { node_id, name, addrs, port }`。
- `ServiceRemoved` → 节点离线，保留历史；重新出现时重连。
- 多地址（IPv4/IPv6）时优先尝试 IPv4；同网段优先。
- mDNS 注册/浏览失败不 panic；UI 提示“自动发现不可用”，仍可手动连接。

### 3.2 手动连接

- CLI：`--connect host:port`（可重复）。
- 配置：`[network] seeds = ["192.168.1.23:47731"]`。
- 手动节点不因 mDNS 消失而删除；是否自动重连由配置决定（默认是）。
- 支持 IPv4；IPv6 为 best effort（`[::1]:47731`）。

## 4. 传输与帧

### 4.1 长度前缀

```text
┌──────────────┬───────────────────────────┐
│ u32 BE length│ JSON UTF-8 payload        │
└──────────────┴───────────────────────────┘
```

- 最大帧 64 KiB；Hello 最大 4 KiB；Chat body 最大 16 KiB。
- 用 4 字节大端长度 + `read_exact`/`write_all`（也可用 `LengthDelimitedCodec`）。
- 解码失败/超长：记录错误并关闭连接（帧边界已丢失，不能继续）。
- 不用 JSON lines：长度前缀更容易限制内存、处理粘包/半包。

### 4.2 帧类型

```rust
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
| `hello` | 双向首帧 | 版本、身份、能力、nonce |
| `ready` | 双向 | 握手完成确认 |
| `chat` | 双向 | 聊天消息 |
| `ping`/`pong` | 双向 | 心跳 |
| `bye` | 双向 | 主动断开原因 |
| `error` | 双向 | 协议错误 code/message |

## 5. 握手

TCP 建立后双方立即互发 Hello：

```json
{
  "type": "hello",
  "magic": "lanchat",
  "protocol": 1,
  "node_id": "uuid",
  "name": "alice@arch",
  "version": "0.1.1",
  "capabilities": ["chat"],
  "nonce": "random-hex"
}
```

- 第一帧必须是 Hello；否则关闭。
- 校验 magic、protocol 主版本、UUID、昵称长度、`node_id != local`。
- 不兼容：发送 `Error { code: "version_mismatch" }` 后关闭；UI 提示对端版本。
- 重复身份：拒绝连接，提示“node_id 冲突”。
- 双方 Hello 校验通过后发送 `Ready`；收到对方 Ready 才标记 Online。

## 6. 重复连接

同一 node_id 可能出现两条连接（双方同时发起、mDNS 重复触发）。规则：

1. `node_id` 字典序较小的一方负责主动连接；较大方等待。
2. 较大方等待 2s 仍无连接时也可发起（兜底）。
3. handshake 后 manager 发现同一 node_id 已 Online：
   - 优先保留“较小 node_id 发起”的连接；
   - 无法判定时按握手 nonce/socket 四元组确定；
   - 另一条发送 `Bye { reason: "duplicate" }` 并关闭；
   - 只向 App 发一次 `PeerUp`。
4. 两个进程使用同一 node_id：拒绝并提示；不合并身份。

> 0.1.0 旧版用端口号比较，端口变化/重连会失效；0.1.1 以 node_id 为准。

## 7. 聊天语义

### 7.1 消息

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

- `msg_id` 发送方生成 UUID；去重。
- `room` 固定 `lobby`；未知 room 丢弃并记录。
- `from` 必须与握手 node_id 一致。
- `name` 长度受限；可与握手昵称不同（改名后），但限制字符。
- `seq` 每发送方单调递增；0.1.1 只记录，不补发。
- `sent_at` 发送方时钟；显示用本地 `received_at`，历史可保留 `sent_at`。

### 7.2 发送

1. 用户在 Compose 按 Enter（非空）。
2. App 生成 msg_id，插入本地消息（保持 0.1.0 行为）。
3. 网络层将 Chat 写入所有 Online peer 的 writer channel。
4. 写入失败 → `SendFailed { msg_id, reason }`，UI 标记 `!`。
5. 没有在线 peer：本地消息保留；状态提示“无在线节点”。

### 7.3 接收

1. reader 解码 Chat；校验 from/room/size/msg_id。
2. 去重（10,000 条 LRU）；重复则丢弃。
3. 插入消息；记录 received_at；发 `ChatReceived` 给 App。
4. App 按 List 选中模式处理：
   - 自动 → 选中最新、滚到底；
   - 手动 → 保持选中/滚动，状态提示“有新消息”。
5. 不转发（全 mesh）。

### 7.4 顺序与去重

- 同一发送方：TCP 保序 + seq。
- 不同发送方：按本地 received_at 显示；不保证全局严格顺序。
- 去重表容量 10,000；不持久化；重启后极端重复可接受。
- 不做应用层 ACK/重传；TCP 写入成功不等于对端显示。

## 8. 心跳、超时、重连

| 项 | 值 |
| --- | --- |
| Ping 间隔 | 10s |
| Pong 超时 | 5s |
| Idle 超时 | 30s |
| Connect 超时 | 3s |
| Handshake 超时 | 5s |
| 写超时 | 5s |
| 关闭等待 | 1s |

- 重连退避：1s → 2s → 4s → 8s → 16s → 30s，±20% 抖动。
- 握手成功重置退避；手动断开不自动重连。
- 重连成功后不重发旧消息；不交换历史。
- UI 状态：`connecting` / `online` / `reconnecting` / `offline`。

## 9. 端口与多实例

- 默认 TCP 端口 `47731`；`--port` 覆盖；`--port 0` 随机（测试）。
- 端口占用：启动失败并给可读错误，不自动顺延。
- 本机测试多实例：不同 `--port` + 不同配置目录（`--config` 指向不同文件）。
- 同一 node_id 多实例：拒绝连接；测试用不同配置目录生成不同 node_id。

## 10. 限制与安全

| 项 | 限制 |
| --- | --- |
| 帧 | 64 KiB |
| Chat body | 16 KiB |
| 昵称 | 64 字符/256 字节 |
| room | 32 字节 |
| peers | 64 |
| 消息速率 | 建议 20 条/秒/peer |
| 去重 | 10,000 条 LRU |

0.1.1 **不提供**：

- 加密、身份认证、前向保密、访问控制；
- 文件传输、历史同步、离线消息、中继、NAT 穿透。

缓解：

- README/`:help security` 说明“仅限可信局域网”；
- 不记录消息正文到日志；历史 `0600`；
- 校验 `from == handshake node_id`；
- 帧/消息大小上限，解析失败关闭连接不崩溃；
- 单 peer 速率限制（建议）。

## 11. 测试

| 测试 | 内容 |
| --- | --- |
| 帧 | 每种帧 roundtrip；坏 JSON；超长/截断/半包 |
| 握手 | 版本不匹配、magic 错、重复 node_id、超时、Ready 缺失 |
| 去重 | 同一 msg_id 多次接收；本地回显；重连重复 |
| 心跳 | Ping/Pong、丢 Pong、Idle 超时 |
| 重连 | 退避序列、抖动范围、成功重置、手动断开不重连 |
| 重复连接 | 双方同时连接、保留规则、只发一次 PeerUp |
| 多 peer | 3–5 个 manager 互连、广播、断开 |
| 大消息 | 16 KiB 边界、超限拒绝 |
| 安全 | 伪造 from、超长 name/body、控制字符 |

- 集成测试用 `127.0.0.1:0` 和手动连接，不依赖 mDNS。
- mDNS 手工/可选集成测试，不阻塞 CI。
- 0.1.0 的 57 个本地测试必须继续通过。

## 12. 已确认

协议细节（帧格式/端口/重复连接/心跳/时间显示/限制）已确认，见 [05-open-questions.md](05-open-questions.md) 与 [decisions.md](decisions.md)。
