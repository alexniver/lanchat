# lanchat 0.1.7 修改名称与状态栏本机名

> 状态：**0.1.7 已实现，验收中**（0.1.6 已冻结）
> 版本：0.1.7
> 创建日期：2026-11-19

## 目标

增加：

```text
/setname alice
```

修改后：

- 显示名变成 `alice (no1)`；
- 状态栏显示本机名；
- 在线 peer 尽快看到新名字；
- 重启后名字保持。

## 显示格式

消息/成员：

```text
alice (no1)
```

状态栏（List）：

```text
本机 alice (no1) · 在线 2 · 消息 3 · 选中 3/3 · 自动 · ● online
```

状态栏（Compose）：

```text
本机 alice (no1) · 输入中 · Enter 发送 · Esc 取消 · 消息 3
```

状态栏（Help）：

```text
按 Esc 退出帮助 · j/k 滚动 · 帮助 1/24 · LanChat 0.1.7 · 本机 alice (no1)
```

- 本机名放在状态栏最前面（List/Compose），保证窄终端也优先可见。
- 帮助状态放在末尾，保留 Esc 提示。

## 命令

```text
/setname <新名字>
```

示例：

```text
/setname alice
/setname 张三
/setname alice@arch
```

规则：

- 1–64 个字符；
- 去首尾空白；
- 拒绝空名、控制字符；
- 允许中文/空格/常见符号；
- 显示时自动追加 `(no1)`，基础名不包含短号。

## 持久化

`/setname` 同时更新：

- `identity.json` 的 `name`；
- `config.toml` 的 `[identity].name`（如果文件存在，保留其他配置/注释）。

这样重启后不会被 config 里的旧 name 覆盖。

## 在线传播

- 新增 `NameUpdate { node_id, name }`；
- 仅发给支持 capability `name_update` 的 0.1.7 peer；
- peer 更新显示名，不改变 node_id/short_no；
- 0.1.6 及更早 peer 看不到实时改名，重连后看到新 Hello.name；
- 历史消息不重写。

## 范围

| 能力 | 0.1.7 | 说明 |
| --- | --- | --- |
| `/setname` | ✅ 做 | 1–64 字符 |
| 更新 identity.json | ✅ 做 | 持久化 |
| 更新 config.toml | ✅ 做 | toml_edit 保留注释 |
| 状态栏本机名 | ✅ 做 | List/Compose/Help |
| NameUpdate | ✅ 做 | 0.1.7 peer |
| 旧 peer 实时改名 | ❌ 不做 | 重连后生效 |
| 历史重写 | ❌ 不做 | 保留当时名字 |
| 手工设置 short_no | ❌ 不做 | 自动分配 |

## 文档

- [01-scope.md](01-scope.md)
- [02-command.md](02-command.md)
- [03-status.md](03-status.md)
- [04-architecture.md](04-architecture.md)
- [05-open-questions.md](05-open-questions.md)
- [06-roadmap.md](06-roadmap.md)
- [decisions.md](decisions.md)
