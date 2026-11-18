# 显示名：基础名 + node_id

> 状态：已实现
> 关联：[01-scope.md](01-scope.md) · [03-ui.md](03-ui.md)

## 1. 格式

```text
alice@arch (19b9b31e)
```

```text
display_name = clean(base) + " (" + node_id[..8] + ")"
```

- `base`：`Identity.name` / `Chat.name`。
- `node_id`：UUID 字符串。
- 短标识：前 8 个字符；UUID 前 8 位是十六进制。
- 括号用英文半角，前面一个空格。

## 2. 为什么用短 node_id

完整 UUID：

```text
alice@arch (19b9b31e-c81d-4c6a-8af3-27c16587b238)
```

问题：

- 消息行太长；
- 成员栏 26 列放不下；
- 8 位足够局域网区分。

命令层仍支持完整 node_id，避免前缀冲突。

## 3. 短标识规则

```rust
node8 = node_id.chars().take(8).collect()
```

- UUID：`19b9b31e-...` → `19b9b31e`。
- 短于 8 字符的 node_id：原样使用。
- 空 node_id：不追加后缀。
- 不做大小写转换；UUID 默认小写。
- 不做 hash；直接用前缀，便于用户从 `identity.json`/日志识别。

## 4. 基础名清理

- 去首尾空白。
- 控制字符替换为空格。
- 空名 -> `用户`。
- 如果基础名已以 ` (<8位十六进制>)` 结尾，替换为当前 node8：

```text
alice (deadbeef) → alice (19b9b31e)
```

- 只影响显示，不修改配置/协议。

## 5. 消息/历史

### 本地发送

```text
Message.name = display_name(identity.name, identity.node_id)
```

- `Effect::SendChat` 仍发送基础名（manager 使用 `Identity.name`）。
- 历史保存组合名。

### 远端接收

```text
Message.name = peers[chat.from].display_name
```

- `PeerUp` 先建立 PeerView。
- 找不到 peer 时用 `display_name(chat.name, chat.from)`。
- 历史保存组合名。

## 6. 显示 vs 身份

| 用途 | 用哪个 |
| --- | --- |
| 消息/成员显示 | `display_name` |
| 历史 name | `display_name` |
| 协议 Hello.name | 基础名 |
| 协议 Chat.name | 基础名 |
| 网络唯一标识 | node_id |
| 文件传递目标 | node_id（完整/前缀） |

## 7. 寻址规则

`<对方>` 解析顺序：

1. 完整 node_id 精确匹配；
2. node_id 唯一前缀；
3. 完整 IP；
4. 昵称（大小写不敏感）。

不再匹配 IPv4 最后一段。

- 前缀匹配到多个 → `匹配到多个在线用户，请改用完整 node_id`。
- 找不到 → `请用 node_id、完整 IP 或昵称重试`。

示例：

```text
/send file ~/a.png to 19b9b31e
/send file ~/a.png to 19b9b31e-c81d-4c6a-8af3-27c16587b238
/send file ~/a.png to 192.168.1.103
/recv file 19b9b31e
```

## 8. 边界

- 基础名重复：node_id 后缀区分。
- node_id 前缀冲突：用完整 node_id。
- 旧 peer：基础名显示；0.1.5 仍可根据 `Chat.from` 组合。
- 旧历史：无后缀，保持原样。
- IPv4/IPv6 不影响显示名。
- `--no-network`：自己仍显示 node8（node_id 本地已知）。

## 9. 待确认

- 显示短前缀还是完整 node_id（当前短前缀）。
- 是否保留完整 IP/昵称寻址（当前保留）。
- 前缀最小长度（当前任意唯一前缀）。
- 是否提供“显示完整 node_id”开关。
