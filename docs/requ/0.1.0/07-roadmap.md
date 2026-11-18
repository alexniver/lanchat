# 0.1.0 实现路线图

> 状态：M0–M3 已实现；P2 可选增强与 0.1.1 网络未做。核心交互见 [08-core-interaction.md](08-core-interaction.md)
> 关联：[01-scope.md](01-scope.md) · [04-architecture.md](04-architecture.md) · [05-network.md](05-network.md)（0.1.1）

## 1. 总体顺序

```text
M0 脚手架/终端/双 panel 骨架
        │
        ▼
M1 核心交互：消息列表 + 选中 + 输入栈 + 复制
        │
        ▼
M2 历史 + keymap.toml + 中文/状态打磨
        │
        ▼
M3 测试/验收/发布 0.1.0
        │
        ▼
0.1.1 网络（独立，不在 0.1.0）
```

**原则**：先把用户明确要求的交互闭环（双 panel、`i` 输入栈、`j/k` 选中、`y/Y` 复制、状态“复制成功”）做成可运行 demo，再做历史和键位配置，最后打磨。

## 2. 里程碑

### M0 — 脚手架与双 panel 骨架

**目标**：能启动、退出、渲染上下两个 panel。

- 根 `Cargo.toml` 加回 0.1.0 最小依赖。
- `TerminalGuard`：raw mode、alternate screen、bracketed paste、panic hook。
- 事件循环：`EventStream` + tick + dirty 渲染。
- 双 panel 布局：上信息列表、下状态（3 行）；最小尺寸/resize。
- `App`/`Layer` 栈骨架：`[List]`；`Q` 退出；`i` 先占位。
- 配置/身份加载骨架；`--no-config`/`--version`/`--help`。

**退出标准**：

- 启动显示双 panel；状态显示占位文本。
- `Q` 退出；panic 后终端恢复。
- 40×10 显示“终端太小”；80×24 正常。

### M1 — 核心交互闭环

**目标**：完全按 08 文档跑通本地聊天。

- 消息模型：`name`/`datetime`/`body`/`id`；格式 `[name][datetime]: message`。
- 信息列表：默认自动、选最后；`j/k` 到非最新转手动、新消息不跳；回到最新恢复自动；选中高亮；折行。
- Compose：`i` push；输入框；Enter 非空发送/空取消；Esc 取消；pop。
- Compose 下忽略 `j/k/y/Y/i` 列表动作（作为文字输入）。
- 复制：`y` 正文、`Y` 完整行；arboard + OSC52 兜底；状态“复制成功/失败”。
- 状态 panel：`在线 1（本机） · 消息 N · 选中 i/N · 自动/手动`；输入提示；临时提示 2 秒。
- 终端 CJK 宽度：`unicode-width`；折行/光标。

**退出标准**：

- 启动/输入/发送/取消/选中/自动-手动切换/复制全流程可用。
- Compose 下 `j/k/y/Y/i` 不触发列表动作。
- 中文/emoji 输入和显示不崩；字体正确时对齐。
- 状态“复制成功”2 秒后消失。

### M2 — 历史、键位配置与打磨

**目标**：重启可回看；List 键位可改；体验完整。

- 历史 JSONL：按月、加载最近 N、坏行跳过、权限 `0600`、轮转、退出 flush。
- 启动加载历史并默认选最后；无历史空列表。
- `keymap.toml`：默认 `[list]`；覆盖/解绑；Compose Enter/Esc 保留；坏配置回退；`--no-config`。
- 配置：昵称、历史开关/上限；原子写入。
- 颜色/`NO_COLOR`；状态文字截断；首次启动提示。
- 错误提示：历史/剪贴板/配置失败。
- 手工验收：CJK 字体、tmux/SSH、resize。

**退出标准**：

- 重启恢复历史；权限/坏行测试通过。
- List 动作可通过 keymap 改；坏配置不锁死。
- 复制/历史/配置失败都有可见提示。
- 核心验收清单通过。

### M3 — 测试、验收、发布 0.1.0

- `cargo fmt`/`clippy`/`test`/`build --release`。
- README/双 panel/键位/历史/字体要求/已知限制。
- 旧身份迁移/升级测试。
- 24h soak（建议）。
- 打 tag `v0.1.0`（用户确认后）。

## 3. 0.1.1 — 网络（独立，不在 0.1.0）

0.1.0 发布后再启动：

- 协议：长度前缀帧、Hello/Ready/Chat/Ping/Pong/Bye/Error。
- 发现：mDNS + 手动连接。
- manager：peer 状态机、连接去重、心跳、重连退避。
- UI：状态 panel 真实在线数、成员面板、未读、连接状态；新 Layer。
- 测试：localhost 两个 manager；两机手工验收。
- 详细设计见 [05-network.md](05-network.md)。

## 4. 任务依赖

| 任务 | 依赖 |
| --- | --- |
| Layer 栈 / 键分发 | App 骨架 |
| 消息模型/格式化 | 无 |
| 列表选中/折行 | 消息模型 + 布局 |
| Compose/输入框 | Layer 栈 + 输入编辑器 |
| 复制 | 剪贴板模块 + 选中 |
| 状态 panel | 消息/选中/提示 |
| 历史 | 消息模型 + 存储 |
| keymap 配置 | 默认键位 + 解析/校验 |
| TUI 测试 | 渲染入口 + App |

## 5. 建议的 PR 序列

1. `chore: scaffold app, terminal guard, dual-panel layout`
2. `feat: layer stack and Q quit`
3. `feat: message model and list selection`
4. `feat: compose layer with Enter/Esc and text input`
5. `feat: y/Y clipboard copy and status toast`
6. `feat: keymap.toml list bindings`
7. `feat: JSONL history`
8. `feat: CJK width, resize, terminal polish`
9. `test: stack/selection/copy/history coverage`
10. `docs: README and font/keymap notes`

> 每个 PR 保持可编译、测试通过；不把大重构留到最后。

## 6. 第一个可运行 demo

M1 结束时即可运行：

- 上下双 panel；10 条占位消息；默认选最后。
- `j/k` 移动选中；到非最新转手动；到最新恢复自动；`y/Y` 复制（先写日志/mock 也可）。
- `i` 弹出输入 panel；输入中文；Enter 追加消息；Esc 取消。
- Compose 下 `j/k/y` 作为文字输入，不移动/复制。
- 状态显示消息数/选中/自动或手动/复制成功。

这个 demo 能提前验证“栈 + 输入 panel + 选中”的核心手感，避免后面返工。

## 7. 待确认

- OQ-25 Compose Enter 语义（已确认）。
- OQ-26 退出键（已确认：List `Q`）。
- OQ-27 datetime 格式（已确认）。
- OQ-28 取消输入后的选中项。
- OQ-29 剪贴板方案。
- OQ-30 状态 panel 高度。
- OQ-31 手动模式下自己发送是否自动跳最新（建议不跳）。
- 是否把搜索/命令/帮助放到 0.1.x 可选。
