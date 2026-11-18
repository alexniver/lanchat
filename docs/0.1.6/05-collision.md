# 短号冲突

> 状态：已实现

## 规则

本地短号与 peer 相同：

```text
local.node_id < peer.node_id  → 本地保留
local.node_id > peer.node_id  → 本地顺延
```

顺延：

```text
candidate = 1
while candidate 已被本地 peers/冲突 peer 使用:
    candidate += 1
local.short_no = candidate
```

- 保存 identity.json。
- 发 ShortNoUpdate 给支持短号的 peer。
- 发 LocalShortNoChanged 给 App。
- 新短号从下一条消息/成员更新开始生效。

## 为什么按 node_id

- node_id 稳定且全局唯一；
- 两端看到同一对 node_id，会做出一致决定；
- 短号只用于显示/命令，不参与协议唯一性。

## 边界

- 两个 peer 互相冲突：较小 node_id 保留，较大顺延。
- 三个及以上短号冲突：逐个顺延到空号。
- 短号 0（老 peer）：不参与冲突。
- 同一 node_id（复制 identity）：无法解决，属于配置错误。
- 短号复用：离线后号码可被新节点使用；历史 `from` 仍准确。
