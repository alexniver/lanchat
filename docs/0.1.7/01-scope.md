# 0.1.7 范围与验收

> 状态：已实现

## 目标

- `/setname <名字>` 修改基础昵称。
- 显示名立即变成 `<名字> (noN)`。
- 状态栏显示本机显示名。
- 修改持久化到 identity.json 和 config.toml。
- 在线 0.1.7 peer 收到 NameUpdate。
- 不改变 node_id/short_no/历史。

## 非目标

| 不做 | 说明 |
| --- | --- |
| 修改 short_no | 自动分配/冲突顺延 |
| 修改 node_id | UUID 唯一身份 |
| 重写历史 name | 历史是当时事实 |
| 旧 peer 实时改名 | 重连后新 Hello.name |
| 名字唯一性 | 短号才用于区分 |
| 名字颜色/格式 | 仅文本 |
| 名字长度无限 | 1–64 字符 |

## 规则

- 名字 trim；
- 1–64 字符；
- 不能有控制字符；
- 允许中文、空格、常见符号；
- 基础名可以包含 `(no1)`？显示时 `naming::display_name` 会替换已有后缀，避免重复。

## 持久化

- `identity.json` 的 `name` 更新。
- `config.toml` 的 `[identity].name` 更新（存在时）。
- 配置写入使用 `toml_edit`，保留其他段落/注释。
- 写入失败时显示错误，不静默。

## 在线传播

- 新增 `NameUpdate`，capability `name_update`。
- 收到 NameUpdate 校验 node_id；
- 更新 PeerView.name/display_name；
- 不改 short_no；
- 旧 peer 不发送 NameUpdate。

## 验收

- [x] `/setname alice` 后显示 `alice (no1)`。
- [x] 状态栏显示 `本机 alice (no1)`。
- [x] 重启后名字仍是 alice。
- [x] config.toml 其他配置/注释保留。
- [x] 在线 0.1.7 peer 成员栏更新名字。
- [x] 旧 peer 不崩溃（不发送 NameUpdate）。
- [x] 空名/超长/控制字符报错。
- [x] 0.1.6 的 111 个测试回归；当前 116。
