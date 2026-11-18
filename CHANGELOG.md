# Changelog

本文件记录 LanChat 的版本变化。README 只保留当前版本的介绍、安装和使用方法。

## [0.1.8] - 2026-11-19

### Changed

- 文件命令改为 `/sendfile <路径> to <对方>`、`/recvfile <对方>`。
- 文件请求提示、帮助和 README 使用新命令。
- 版本号更新为 0.1.8。

### Compatibility

- 保留 `/send file`、`/recv file` 作为兼容别名。
- 保留 `/set name` 作为 `/setname` 的兼容别名。
- 参数、寻址、超时、保存和重名规则不变。

## [0.1.7] - 2026-11-19

### Added

- `/setname <名字>` 修改昵称，显示为 `名字 (no1)`。
- 状态栏显示本机名：`本机 alice (no1) · ...`。
- `NameUpdate` 能力，在线 peer 可实时看到改名。
- `/set name` 兼容写法。

### Changed

- `/setname` 同时更新 `identity.json` 和 `config.toml` 的 `[identity].name`（存在时），使用 `toml_edit` 保留其他配置。
- 帮助和状态栏更新本机名。

### Compatibility

- 0.1.6 及更早 peer 不会收到实时 `NameUpdate`，重连后看到新名字。
- 历史消息不重写。

## [0.1.6] - 2026-11-19

### Added

- 持久化 `short_no`，显示为 `no1`、`no2`。
- Hello 交换短号；冲突时按 node_id 字典序顺延，并广播 `ShortNoUpdate`。
- 文件寻址支持 `no1` 和 `ip101`。
- 保留完整 node_id、node_id 前缀、完整 IP、昵称作为兜底。

### Changed

- 文件命令仍为 `/send file`、`/recv file`（0.1.8 改名）。
- UUID `node_id` 继续作为协议/历史/去重的唯一身份。
- 裸 `103` 不再作为 IP 最后一段匹配，必须写 `ip103`。

## [0.1.5] - 未发布

- 尝试用 node_id 前缀作为用户名后缀；该方案被 0.1.6 的短号方案取代，未单独发布。
- 仅保留 `docs/0.1.5/` 作为设计记录。

## [0.1.4] - 2026-11-19

### Added

- 跨平台路径适配：
  - 配置走 `dirs::config_dir()`；
  - 历史/接收文件走 `dirs::data_local_dir()`；
  - 日志优先 `state_dir()`，否则回退 data。
- `Paths.files_dir` 统一接收文件目录。
- `~` 使用 `dirs::home_dir()`，支持 `~/` 和 `~\`。
- 接收文件名跨平台清理：Windows 非法字符、保留设备名、结尾点/空格、长度截断。
- `dunce::canonicalize` 避免 Windows `\\?\` 路径显示。

### Changed

- 帮助页显示实际 files 路径。
- README 增加 Linux/macOS/Windows 路径表。

## [0.1.3] - 2026-11-19

### Added

- `?` 打开内置帮助页面。
- 帮助内容可滚动，`j/k` 上下滚动，`Esc` 返回。
- 帮助状态栏显示滚动位置和版本。

### Changed

- 帮助打开时不改变消息列表选中，不写历史/发网络消息。
- `Q` 在帮助内忽略；成员栏内 `?` 忽略。

## [0.1.2] - 2026-11-19

### Added

- `/send file <路径> to <对方>`、`/recv file <对方>` 文件传递。
- 10 分钟未接收自动取消；重复请求拦截。
- `/recv` 接收指定发送方全部待接收文件。
- 文件保存到 `lanchat/files`，重名自动 `[0]`/`[1]`。
- 文件内容独立数据 TCP 连接；控制消息走 peer 连接。
- 文件请求/成功/取消作为系统消息显示，不写历史。

### Security

- 明文传输，无加密/鉴权；仅可信局域网。

## [0.1.1] - 2026-11-19

### Added

- mDNS 自动发现 + `--connect` 手动连接。
- 单房间 `#lobby` TCP 聊天。
- 在线人数、右侧成员侧边栏、`Space u`。
- 心跳、断线重连、`Space r` / `Space d`。
- 历史记录远端消息的 `from`/`sent_at`，显示本地接收时间。
- 自动/手动选中规则扩展到远端消息。

### Changed

- 版本号从 0.1.0 更新到 0.1.1。
- `--no-network` 保留纯本地模式。

## [0.1.0] - 2026-11-19

### Added

- 单机终端聊天 TUI：消息列表 + 3 行状态栏。
- `i` 输入、Enter 发送、Esc 取消。
- 自动/手动选中、`j/k`、`y`/`Y` 复制。
- JSONL 本地历史。
- Helix-inspired、可配置 `keymap.toml`。
- CJK/emoji 宽度处理、终端恢复。
