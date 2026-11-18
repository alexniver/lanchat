# 0.1.5 架构

> 状态：已实现
> 关联：[02-naming.md](02-naming.md) · [03-ui.md](03-ui.md)

## 1. 命名模块

新增 `src/app/naming.rs`：

```rust
pub const SHORT_NODE_ID_LEN: usize = 8;
pub fn short_node_id(node_id: &str) -> String;
pub fn display_name(base: &str, node_id: &str) -> String;
```

- `short_node_id` 取前 8 个字符。
- `display_name` 清理基础名、替换已有 ` (8位hex)`、追加 ` (node8)`。
- 不读写配置/历史/网络。

## 2. PeerView

[src/app/mod.rs](../../src/app/mod.rs)：

```rust
pub struct PeerView {
    pub name: String,          // 基础名，保留给昵称寻址
    pub display_name: String,  // name (node8)
    pub addr: SocketAddr,
}
```

`PeerUp`：

```rust
let display_name = naming::display_name(&name, &node_id);
```

- 成员栏使用 `display_name`。
- 寻址昵称仍用 `name`。
- `PeerDown` 状态也用 display_name。

## 3. 消息

### 本地

```rust
let name = self.self_display_name();
let mut message = Message::new(name, text);
```

`self_display_name()`：

```rust
naming::display_name(&config.identity.name, &config.identity.node_id)
```

### 远端

```rust
let mut message = Message::remote(&chat);
message.name = peers.get(&chat.from).display_name
    .unwrap_or_else(|| naming::display_name(&chat.name, &chat.from));
```

- 历史 Persist 组合后的 Message。
- 协议 Chat.name 仍是基础名。

## 4. 寻址

`App::resolve_peer`：

```text
完整 node_id == text
node_id.starts_with(text)
完整 IP == text
nickname.eq_ignore_ascii_case(text)
```

- 不再用 IPv4 最后一段。
- 唯一匹配返回 `(node_id, display_name, addr)`。
- 多个匹配 → 歧义，要求完整 node_id。
- `/send`、`/recv` 共用。

## 5. 文件提示

`FileOfferReceived`：

```rust
let display = peers[&node_id].display_name;
let short = naming::short_node_id(&node_id);
format!("{display} 想发送文件 ... 输入 /recv file {short} 接收。")
```

- 其他文件事件已走 `peer_display`，改为返回 display_name。
- 网络事件仍携带基础 `peer_name` 作为无 peer 时回退。

## 6. 为什么不改 identity/协议

- node_id 是稳定身份，但完整 UUID 不适合显示。
- 基础昵称仍是用户配置，不写入运行态后缀。
- 0.1.4 协议/客户端不需要改。
- 0.1.4 peer 仍能聊天；只是看不到 0.1.5 的后缀。

## 7. 兼容/历史

- `HistoryRecord` 不变，`name` 存 display_name。
- 旧历史基础名照常显示。
- 旧 peer 的 Chat.name 基础名；0.1.5 用 `chat.from` 组合。
- node_id 空/短：安全回退。

## 8. 测试

- `naming`：短 ID、空名、控制字符、已有后缀替换。
- `App`：PeerUp display、ChatReceived display、self display。
- `resolve_peer`：完整/前缀 node_id、完整 IP、昵称、`103` 不再匹配、歧义。
- `/send` 用 node_id 目标。
- UI 成员/消息显示。
- 0.1.4 的 103 个测试继续通过；当前 111。

## 9. 待确认

- 显示短前缀 vs 完整 node_id（当前短前缀）。
- 是否保留完整 IP/昵称寻址（当前保留）。
- 前缀最小长度。
- 是否提供完整 ID 显示开关。
