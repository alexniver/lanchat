# lanchat 0.1.6 短号 no1 与文件寻址

> 状态：**0.1.6 已实现，验收中**（0.1.5 已冻结）
> 版本：0.1.6
> 创建日期：2026-11-19

## 目标

保留 UUID `node_id` 作为唯一身份，增加短号 `short_no`（`no1`、`no2`），解决 UUID 不好输入的问题：

```text
alice (no1)
bob   (no2)
```

文件传递：

```text
/send file a.png to no1
/send file a.png to ip101
/recv file no1
/recv file ip101
```

## 为什么不是把 node_id 换成自增 u32

局域网没有中心分配器，本地自增会导致两台机器都叫 `no1`。0.1.6 采用：

- **UUID `node_id` 仍是协议/历史/去重的唯一身份**；
- 新增持久化 `short_no` 作为显示/命令短号；
- 连接时交换短号，冲突时按 node_id 字典序决定谁保留，另一个顺延；
- 短号只保证在线节点内唯一，不承诺永久不复用。

## 格式

```text
显示名 = 基础名 + " (no" + short_no + ")"
```

示例：

```text
alice (no1)
bob   (no2)
```

- `node_id` 仍是 UUID，不显示完整 UUID。
- 老 peer 没有 `short_no` 时回退 UUID 前 8 位。
- 文件目标 `no1` 是短号，`ip101` 是 IP 最后一段，二者显式区分。

## 范围

| 能力 | 0.1.6 | 说明 |
| --- | --- | --- |
| 持久化 short_no | ✅ 做 | identity.json |
| 显示 `no1` | ✅ 做 | 消息/成员/文件提示 |
| `/send ... to no1` | ✅ 做 | 短号 |
| `/send ... to ip101` | ✅ 做 | IP 最后一段 |
| `/recv file no1/ip101` | ✅ 做 | 同一解析器 |
| 完整 node_id/IP/昵称 | ✅ 保留 | 兜底 |
| 冲突顺延 | ✅ 做 | 按 node_id 决胜 |
| 短号更新通知 | ✅ 做 | ShortNoUpdate |
| UUID 替换 | ❌ 不做 | 保留内部唯一身份 |
| 短号永久唯一 | ❌ 不承诺 | 在线内唯一 |
| 手工改短号 | ❌ 不做 | 自动分配/顺延 |

## 文档

- [01-identity.md](01-identity.md)
- [02-commands.md](02-commands.md)
- [03-ui.md](03-ui.md)
- [04-architecture.md](04-architecture.md)
- [05-collision.md](05-collision.md)
- [06-testing.md](06-testing.md)
- [07-roadmap.md](07-roadmap.md)
- [decisions.md](decisions.md)
