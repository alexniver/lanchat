# lanchat 0.1.5 用户名追加 node_id

> 状态：**方案已被 0.1.6 取代**（0.1.6 使用短号 no1）
> 版本：0.1.5
> 创建日期：2026-11-19

## 一句话目标

用稳定的 `node_id` 区分用户，不再使用 IP 最后一段。消息和成员栏显示：

```text
alice@arch (19b9b31e)
bob@arch   (c81d46a5)
```

`/send file ... to <node_id>` 和 `/recv file <node_id>` 都支持 node_id。

## 背景

0.1.4 的用户名默认可能重复（例如两台机器都叫 `zhulin@archlinux`）。IP 最后一段虽然能区分，但：

- DHCP 会变；
- 跨网段可能重复；
- IPv6 没有统一的“最后一段”；
- 不是真正的身份。

项目已经有稳定 `node_id`：

- `Identity.node_id`；
- `Hello.node_id`；
- `Chat.from`；
- `App.peers` 以 node_id 为 key。

因此 0.1.5 改用 node_id。

## 推荐方案

**显示名追加 node_id 前缀，不修改 identity/协议。**

```text
基础名：alice@arch
node_id：19b9b31e-c81d-4c6a-8af3-27c16587b238
显示名：alice@arch (19b9b31e)
```

- `identity.name` 保持 `alice@arch`。
- `Hello.name` / `Chat.name` 仍发送基础名，兼容旧版本。
- 显示/历史使用 `基础名 (短 node_id)`。
- `/send file ... to 19b9e31e`、`/recv file 19b9b31e` 可用。
- 完整 node_id 也可以。
- 不再使用 IP 最后一段。

### 为什么短 node_id

完整 UUID 是 36 字符：

```text
alice@arch (19b9b31e-c81d-4c6a-8af3-27c16587b238)
```

在消息列表和 26 列成员栏里太长。取前 8 位：

```text
alice@arch (19b9b31e)
```

- 8 位在局域网规模下足够区分；
- 命令可用 8 位前缀；
- 冲突时用完整 node_id；
- 显示的是 node_id，不是 IP。

## 范围和效果

- 消息列表：`[alice@arch (19b9b31e)][...]: ...`
- 成员栏：`alice@arch (19b9b31e)`，自己仍标 `(you)`
- 文件请求：`alice@arch (19b9b31e) 想发送文件… 输入 /recv file 19b9b31e 接收。`
- 历史保存组合后的显示名。
- `--no-network` 也显示自己的 node_id 后缀（node_id 本地就有）。
- 旧历史/旧 peer 保持基础名可读。

## 范围表

| 能力 | 0.1.5 | 说明 |
| --- | --- | --- |
| 消息显示 node_id 短标识 | ✅ 做 | 8 位前缀 |
| 成员栏显示 | ✅ 做 | 自己/他人 |
| `/send ... to <node_id>` | ✅ 做 | 完整/唯一前缀 |
| `/recv file <node_id>` | ✅ 做 | 完整/唯一前缀 |
| 不再用 IP 末位 | ✅ 做 | 移除 last-octet 匹配 |
| 历史保存显示名 | ✅ 做 | 旧格式兼容 |
| 不修改 identity/协议 | ✅ 做 | 兼容 0.1.4 |
| IP/昵称寻址 | 🟡 保留 | 便利，不是主标识 |
| 完整 node_id 显示 | 🟡 可选 | 当前显示短前缀 |
| 自定义后缀 | ❌ 不做 | 0.1.5 不提供 |

## 文档

- [01-scope.md](01-scope.md)
- [02-naming.md](02-naming.md)
- [03-ui.md](03-ui.md)
- [04-architecture.md](04-architecture.md)
- [05-open-questions.md](05-open-questions.md)
- [06-roadmap.md](06-roadmap.md)
- [decisions.md](decisions.md)
