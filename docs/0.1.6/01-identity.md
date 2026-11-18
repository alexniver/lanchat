# 身份与短号

> 状态：已实现

## 1. identity.json

```json
{
  "node_id": "19b9b31e-c81d-4c6a-8af3-27c16587b238",
  "name": "alice",
  "short_no": 1
}
```

- `node_id`：UUID，协议/历史/去重的唯一身份。
- `name`：基础昵称。
- `short_no`：短号，`1` 显示为 `no1`。
- 旧 identity 没有 `short_no`：加载时补 `1` 并保存。

## 2. 分配

- 首次运行：`short_no = 1`。
- 连接时交换 Hello.short_no。
- 与在线 peer 冲突：
  - 比较 node_id 字典序；
  - 较小者保留；
  - 较大者顺延到下一个未使用正整数；
  - 保存 identity.json；
  - 通过 ShortNoUpdate 通知 peer；
  - 本地 App 收到 LocalShortNoChanged 更新自己显示名。
- 短号只保证当前在线节点内唯一；离线后可能复用。

## 3. 兼容

- 0.1.5 及更早：Hello 没有 short_no，默认 0。
- 0.1.6 对 short_no=0 的 peer 回退 UUID 前 8 位显示。
- 0.1.6 新增 capability `short_no`；只向支持者发送 ShortNoUpdate。
- 协议 node_id/Chat.from 不变。

## 4. 显示优先级

```text
short_no > 0  -> (no1)
short_no == 0 -> (node8)
```

- 基础名空 -> `用户 (no1)`。
- 已有 ` (noN)` / ` (node8)` 后缀 -> 替换。
- 不修改基础名本身。

## 5. 验收

- [x] identity.json 持久化 short_no。
- [x] 旧 identity 自动补 1。
- [x] Hello 交换 short_no。
- [x] 冲突顺延并保存。
- [x] 旧 peer 回退 node8。
- [x] 不修改 node_id/name。
