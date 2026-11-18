# 0.1.7 待确认

> 状态：P0 已确认

## 已确认

- `/setname <name>`。
- 显示 `name (no1)`。
- 状态栏本机名：
  - List/Compose 放最前；
  - Help 放末尾。
- 持久化 identity.json + config.toml。
- 在线 0.1.7 peer 用 NameUpdate；旧 peer 重连生效。
- 1–64 字符。

## 待确认（P1）

- 是否允许名字包含换行（当前拒绝控制字符）。
- 是否允许重名（当前允许；短号区分）。
- 是否提供 `/setname` 的空格/引号转义（当前取整段）。
- 是否显示完整 name 在帮助页。
- 是否允许手工设置 short_no（当前不做）。
- 是否把名字写进状态栏帮助页（当前写）。
