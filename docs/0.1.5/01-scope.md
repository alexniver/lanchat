# 0.1.5 范围与验收

> 状态：已实现，验收中
> 目标：用 node_id 区分用户，替代 IP 最后一段
> 关联：[README.md](README.md) · [02-naming.md](02-naming.md)

## 1. 背景

项目已有 `node_id`：

```rust
pub struct Identity {
    pub node_id: String,
    pub name: String,
}
```

- 首次运行生成 UUID；
- `Hello.node_id` 用于握手/去重；
- `Chat.from` 是发送方 node_id；
- `App.peers` 以 node_id 为 key。

它比 IP 稳定，适合做用户短标识和文件传递目标。

## 2. 目标

- 显示名格式：`基础名 (短 node_id)`。
- 消息列表、成员栏、文件提示使用显示名。
- `/send file <路径> to <node_id>` 可用。
- `/recv file <node_id>` 可用。
- 支持完整 node_id 和唯一前缀。
- 不再用 IP 最后一段。
- 不修改 identity/协议，兼容 0.1.4。

## 3. 非目标

| 不做 | 说明 |
| --- | --- |
| 修改 `identity.name` | 基础名不变 |
| 修改 `Hello.name`/`Chat.name` | 协议保持基础名 |
| 用 node_id 完全替换昵称 | 显示名仍保留昵称 |
| 显示完整 36 字符 node_id | 默认短前缀，命令可完整 |
| IP 最后一段寻址 | 移除 |
| 自定义后缀/别名 | 0.1.5 不做 |
| 修改历史旧记录 | 旧记录保持 |
| 同 node_id 多用户 | node_id 本身唯一 |

## 4. 功能需求

### FR-01 显示名

- `[必须]` 格式 `base (node8)`。
- `[必须]` `node8` = 完整 node_id 前 8 个字符。
- `[必须]` 基础名空 -> `用户 (node8)`。
- `[必须]` 基础名已有 ` (8位hex)` -> 替换为当前 node8，避免重复。
- `[必须]` 控制字符清理。
- `[必须]` 不修改配置/协议存储。

### FR-02 消息

- `[必须]` 本地消息 `Message.name` 使用 self display name。
- `[必须]` 远端消息使用 peer display name。
- `[必须]` 历史 JSONL 保存 display name。
- `[必须]` 消息格式仍 `[name][datetime]: body`。
- `[必须]` `Y` 完整行包含 node_id 短标识。

### FR-03 成员

- `[必须]` 成员第一行显示 display name。
- `[必须]` 自己保留 `(you)`。
- `[必须]` 地址行仍显示完整 `IP:port`。
- `[应该]` 长名截断不破布局。

### FR-04 文件传递寻址

- `[必须]` `/send file <路径> to <node_id>`。
- `[必须]` `/recv file <node_id>`。
- `[必须]` 完整 node_id 精确匹配。
- `[必须]` 唯一前缀匹配（如 `19b9b31e`）。
- `[必须]` 多个前缀匹配 -> 报歧义，要求完整 node_id。
- `[必须]` 不再匹配 IP 最后一段（`103`）。
- `[应该]` 保留完整 IP 和昵称作为便利寻址。
- `[必须]` 文件请求提示里的 `/recv file` 使用短 node_id。

### FR-05 兼容

- `[必须]` 0.1.4 peer 可继续聊天。
- `[必须]` 0.1.4 peer 仍显示基础名（没有新渲染）。
- `[必须]` 0.1.5 对 0.1.4 消息仍用 peer node_id 组合显示名。
- `[必须]` 旧历史无后缀，照常加载。
- `[必须]` 协议不新增字段。

## 5. 验收

- [x] 消息显示 `[alice@arch (19b9b31e)]`。
- [x] 成员栏显示 `alice@arch (19b9b31e)`、`bob@arch (c81d46a5)`。
- [x] 自己消息也带 node8。
- [x] `/send file a.txt to 19b9b31e` 成功定位。
- [x] `/recv file 19b9b31e` 接收该发送方文件。
- [x] 完整 node_id 可用。
- [x] 歧义前缀报错。
- [x] `103` 不再作为 IP 末位匹配。
- [x] 历史保存 display name。
- [x] 旧历史不变。
- [x] 0.1.4 的 103 个测试继续通过；当前 111。

## 6. 假设

- node_id 是 UUID 或至少 8 字符的稳定字符串。
- 局域网规模下 8 位前缀冲突概率低；冲突时用完整 ID。
- 昵称仍由用户配置，不作为唯一标识。
