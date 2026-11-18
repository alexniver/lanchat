# 架构

> 状态：已实现

## Identity

`config::Identity` 增加：

```rust
pub short_no: u32
```

`IdentityFile` 持久化 `short_no`；旧文件默认补 1。

## 协议

`Hello` 增加：

```rust
#[serde(default)]
pub short_no: u32
```

新增：

```rust
ShortNoUpdate { node_id, short_no }
```

- 仅向 capability `short_no` 的 peer 发送。
- 旧客户端忽略/不会收到。
- `node_id`、`Chat.from` 不变。

## Manager

`PeerEntry` 增加 `short_no`、`supports_short_no`。

`Connected`：

1. 读取 peer short_no/capability；
2. 若与本地冲突：
   - 本地 node_id 较大 → 顺延；
   - 保存 identity；
   - 发送 ShortNoUpdate；
   - 发 LocalShortNoChanged；
3. 插入 PeerEntry；
4. PeerUp 携带 short_no。

收到 ShortNoUpdate：

- 校验 node_id；
- 更新 PeerEntry.short_no；
- 发 PeerShortNoChanged。

## App

- `PeerView { name, display_name, short_no, addr }`。
- `self_display_name()` 用 config.identity.short_no。
- `ChatReceived` 用 peer display。
- `resolve_peer` 支持 `no`/`ip`/node_id/IP/昵称。
- `LocalShortNoChanged` 更新 config.identity.short_no。
- `PeerShortNoChanged` 更新 PeerView/display。
- 文件提示用短号。

## 历史

- 不改 HistoryRecord。
- name 存 display name。
- from 仍是 UUID。
