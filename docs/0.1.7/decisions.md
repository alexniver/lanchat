# 0.1.7 已确认决策

## D-046 `/setname <名字>`

- **状态**：已确认（2026-11-19）
- **决策**：新增 `/setname`；基础名 1–64 字符；显示为 `name (no1)`。
- **影响**：命令解析、持久化、在线传播。

## D-047 持久化 identity + config

- **状态**：已确认
- **决策**：更新 identity.json 和 config.toml 的 `[identity].name`（存在时），使用 toml_edit 保留其他内容。
- **理由**：config 的 name 会覆盖 identity，必须一起改。

## D-048 状态栏格式

- **状态**：已确认
- **决策**：
  - List/Compose：`本机 alice (no1) · ...`
  - Help：`... · LanChat 0.1.7 · 本机 alice (no1)`
- **影响**：状态栏优先级/截断。

## D-049 NameUpdate

- **状态**：已确认
- **决策**：向支持 `name_update` 的 peer 广播；旧 peer 重连后生效；不改 node_id/short_no。
- **影响**：在线名字更新。

## D-050 版本 0.1.7

- **状态**：已确认
- **决策**：本次为 0.1.7；保留 0.1.6 短号/文件/帮助/路径行为。
