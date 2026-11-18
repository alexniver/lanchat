# 质量、测试与风险（0.1.0 单机）

> 状态：草案；核心交互以 [08-core-interaction.md](08-core-interaction.md) 为准
> 关联：[01-scope.md](01-scope.md) · [04-architecture.md](04-architecture.md)

## 1. 质量目标

0.1.0 以“本地聊天核心交互稳定、可预期、中文可用”为目标：

- 不崩溃：IO/配置/历史/剪贴板/终端错误都有处理路径。
- 不误触：输入 panel 打开时，`j/k/y/Y/i` 只作为文字输入。
- 不静默丢：历史写入失败、复制失败都能被用户感知。
- 不脏终端：任何退出路径都恢复 raw mode / alternate screen。
- 不卡顿：空闲低 CPU；长消息/大量历史下仍可滚动。
- 可回归：栈、选中、复制、历史、键位配置有自动化测试。
- 可恢复：坏 keymap 不会把用户锁死。

## 2. 测试策略

### 2.1 测试金字塔

```text
        手工验收（CJK 字体/resize/tmux/SSH/panic/24h）
              ▲
        TUI 渲染测试（TestBackend）
              ▲
   集成测试（App + 假历史 writer + 假剪贴板）
              ▲
        单元测试（栈/选中/格式化/输入/历史/keymap）
```

### 2.2 核心交互用例（必须自动化）

| 模块 | 用例 |
| --- | --- |
| Layer 栈 | 启动 `[List]`；`i` push Compose；Esc/Enter pop；基础 List 不 pop；Compose 下键分发只看栈顶 |
| List | 默认自动、选最后；空列表无选中；`j/k` 到非最新 → 手动；手动下新消息不跳；`j/k` 到最新 → 自动；clamp；选中可见 |
| Compose | 输入字符；`j/k/y/Y/i` 作为文字输入，不触发列表动作；Esc pop 不产生消息；空 Enter pop 不产生消息；非空 Enter 追加并 pop |
| 消息格式 | `[name][datetime]: message`；datetime 格式；正文保留空格；长消息折行；CJK 宽度 |
| 复制 | `y` 复制正文；`Y` 复制完整行；状态“复制成功”；失败“复制失败”；无消息不崩溃 |
| 输入编辑 | 中文/emoji、光标、Backspace/Delete、水平滚动、粘贴换行转空格、16 KiB 上限 |
| keymap | 默认键位；覆盖/解绑；Compose 保留键；未知 action/按键；坏 TOML 回退；`--no-config` |
| 历史 | 追加/加载/坏行/权限/轮转/flush；加载后默认选最后 |
| 状态 | 在线 1（本机）、消息数、选中 i/N、自动/手动、临时提示 2 秒过期 |
| 终端 | 双 panel 布局；状态 3 行；输入 panel 3 行；resize；最小尺寸 |

### 2.3 栈测试重点

- `push`/`pop` 后 `layers.len()` 正确；基础 `List` 永不 pop。
- `Compose` 下 `j` 输入字符 `j`，`ListState.selected` 不变。
- `Compose` 下 `y` 输入 `y`，不触发复制。
- `Compose` 下 `i` 输入 `i`，不 push 第二个 Compose。
- `Esc`/`Enter` 后回到 List；发送后 `selected = last`；取消后恢复 `saved_selected`。
- `Q` 在 Compose 下作为字符输入；List 下退出。
- 渲染：Compose 下选中高亮消失；List 下恢复。
- 手动模式下发送消息：消息追加、选中不跳、模式仍手动；手动下新消息不改变滚动。

### 2.4 keymap 配置测试

- 默认 `[list]` 绑定可用。
- 用户把 `j` 改为 `cursor_up`，`k` 仍下移。
- `"none"` 解绑后按键无动作。
- 尝试改 Compose `Enter`/`Esc`：警告并忽略。
- 未知 action/按键语法：忽略该行，其他生效。
- 前缀冲突（如 `"g"` 和 `"g g"`）：报告并忽略冲突。
- TOML 语法错误：启动回退默认。
- `--no-config`：完全默认。
- 多键序列（若引擎支持）：`"g g"` 可用；超时清除。

### 2.5 集成测试

- `App` + 假 `HistoryHandle` + 假 `Clipboard`：
  - `i` → 输入 → Enter（自动模式）：消息入列表、历史 writer 收到、Compose pop、选中新消息；
  - `i` → 输入 → Enter（手动模式）：消息入列表、Compose pop、选中保持；
  - Esc/空 Enter：无消息、选中恢复；
  - `y/Y`：剪贴板收到正确文本；状态提示；
  - 历史写入失败：状态 `!`，消息仍在内存；
  - 剪贴板失败：状态“复制失败”。
- 不依赖真实终端/剪贴板/网络。

### 2.6 TUI 测试

- `TestBackend` 渲染：
  - 上下 panel；状态 3 行；
  - 消息 `[name][datetime]: message`；
  - 选中高亮；
  - Compose 输入 panel + 选中高亮消失；
  - 状态 `复制成功`/`输入中`；
  - 40×10 过小提示；80×24 正常。
- 不做整屏像素快照；只断言关键文本/区域。

### 2.7 手工验收

- [ ] 终端使用 CJK 等宽字体，中文/emoji 对齐正常。
- [ ] 输入法输入中文，候选框/提交正常（终端相关）。
- [ ] tmux/screen 下运行，退出后终端正常。
- [ ] SSH 下 OSC 52 复制（终端支持时）。
- [ ] resize 多次，布局/选中/输入光标不崩。
- [ ] `Q` 退出；panic 后终端恢复。
- [ ] 有历史/无历史启动；默认自动、选中最后/无选中。
- [ ] `j/k` 到非最新 → 状态“手动”；新消息不跳；`j` 到最新 → 状态“自动”。
- [ ] 手动模式下自己发送消息不自动跳；按 `j` 到最新后恢复自动。
- [ ] 24h 运行（建议）。

## 3. 性能目标与测量

| 指标 | 目标 | 测量方式 |
| --- | --- | --- |
| 冷启动 | < 500 ms | `hyperfine`/手计时（release） |
| 空闲 CPU | < 1% 单核 | `top`/`pidstat` 60s 无输入 |
| 内存 | < 50 MB（10k 消息） | 压测脚本/`/proc/status` |
| 输入延迟 | < 16 ms | 手工/日志时间戳 |
| 历史加载 | 10 MB < 200 ms | 单元/基准测试 |
| 长消息 | 16 KiB 折行 < 16 ms | 基准/手工 |
| 复制 | < 100 ms（本机） | 手工 |

实现约束：

- 只在 `dirty` 或 tick 时重绘；tick 250 ms。
- 折行结果缓存；只构造可见行的 `Line`/`Span`。
- 历史写入后台线程；剪贴板/配置读取不阻塞 UI。
- 日志异步 appender；不写正文。

## 4. 跨平台

| 平台 | 0.1.0 目标 | 注意事项 |
| --- | --- | --- |
| Linux | 主要验收平台 | XDG；raw mode；arboard X11/Wayland；OSC52 |
| macOS | 尽量可用 | 路径；终端字体；剪贴板 |
| Windows | 尽力，记录限制 | Windows Terminal/ConPTY；字体；路径 |
| WSL | 视同 Linux | 剪贴板/多播可能受限；本地验证不需要网络 |

- 避免 `libc`/Unix-only API；文件权限 `cfg(unix)`。
- CJK 字体由终端提供；程序只保证宽度计算。
- 不在 0.1.0 承诺 Windows 完美体验。

## 5. 可观测性

- 日志：XDG state 下按天轮转；默认 INFO；`LANCHAT_LOG=debug`。
- 日志字段：时间、级别、target、msg_id、错误；**不含 body**。
- `--version`：版本、配置/数据路径、平台、`local` 模式。
- 错误 UI：状态 panel 文本；错误码（`history_write`、`clipboard_failed`、`keymap_parse`）。
- 不在 UI 显示堆栈；堆栈只进日志。

## 6. 旧版问题 → 新版对策

| # | 旧版问题 | 影响 | 0.1.0 对策 |
| --- | --- | --- | --- |
| 1 | `event::poll(16ms)` + 每帧渲染 | 空闲 CPU 高 | `EventStream` + `dirty`/250ms tick |
| 2 | `Arc<Mutex<AppState>>` 跨任务 | 锁竞争、难测试 | `App` 单所有者；后台任务只发事件 |
| 3 | 滚动按消息条数 | 长消息显示错位 | 显示行映射；`j/k` 按消息、滚动按行 |
| 4 | `try_send` 静默丢消息 | 用户以为已发送 | 0.1.0 本地；历史失败可见；0.2 网络背压 |
| 5 | 无心跳/重连 | 掉线假在线 | 0.2 |
| 6 | 端口比较去重 | 重连失效 | 0.2 |
| 7 | 握手无超时/上限 | 慢速攻击 | 0.2 |
| 8 | JSON 行无上限 | 内存增长 | 消息 16 KiB 上限；0.2 帧上限 |
| 9 | 阻塞文件 IO | 卡 UI | 历史后台线程；剪贴板后台 |
| 10 | 文件路径未校验 | 路径穿越 | 0.1.0 无文件传输 |
| 11 | 整文件 base64 | 内存爆炸 | 0.1.0 无文件传输 |
| 12 | `Ctrl+C` 不处理、`Q` 退出 | 习惯冲突 | Compose 只认 Enter/Esc；List `Q` 退出 |
| 13 | 终端恢复非 panic-safe | 崩溃后终端脏 | `TerminalGuard` + panic hook |
| 14 | 自身过滤 IP+端口 | 多网卡误判 | 0.2 |
| 15 | mDNS 失败 panic | 启动崩溃 | 0.2 |
| 16 | `--port`/`--name` 未生效 | 配置误导 | 0.1.0 CLI 明确生效/文档 |
| 17 | 输入单行无横向滚动 | 长输入不可见 | 水平滚动 + 光标可见 |
| 18 | 退出丢历史 | 无上下文 | JSONL + 退出 flush |
| 19 | 日志写 CWD `log/` | 污染工作目录 | XDG state + 轮转 |
| 20 | 无测试 | 回归困难 | 单元/集成/TUI 测试 |
| 21 | 消息去重无上限 | 内存增长 | LRU 10k |
| 22 | 发送/文件错误只写日志 | 用户无感 | 状态提示 |
| 23 | 单连接多 task 生命周期不清 | 幽灵 peer | 0.2 |
| 24 | 渲染/日志每帧写文件 | IO 噪音 | 降低日志级别 |
| 25 | 键位写死、不可改 | 不符合习惯 | `keymap.toml` + 回退 |
| 26 | 配置错误锁死 | 用户无法启动 | 保留键 + `--no-config` |

## 7. 风险登记

| 风险 | 概率 | 影响 | 应对 |
| --- | --- | --- | --- |
| Compose 下误触列表动作 | 中 | 输入被打断 | 栈顶分发；Compose 不查 List 键位树；测试 |
| 退出键被改坏 | 中 | 无法退出 | `Q` 保留（List）；`--no-config` |
| 中文错位/方块 | 高 | 不可读 | `unicode-width`；文档要求 CJK 等宽字体 |
| 剪贴板在 SSH/headless 不可用 | 高 | 复制失败 | arboard + OSC52；失败提示 |
| 终端/剪贴板差异（OSC52） | 中 | 复制不可用 | arboard + OSC52；失败提示 |
| 范围膨胀（搜索/命令/网络） | 中 | 延期 | 核心验收只按 08；其他 P2/0.2 |
| 历史隐私/磁盘增长 | 低 | 隐私/占用 | `0600`、轮转、可关闭 |
| 大消息/长历史卡顿 | 低 | 体验 | 16 KiB 上限、可见行渲染、缓存 |
| 终端恢复失败 | 低 | 终端脏 | TerminalGuard + panic hook + 测试 |

## 8. 发布检查清单（0.1.0）

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test`（单元 + 集成 + TUI）
- [ ] `cargo build --release`
- [ ] `cargo tree` 无 `mdns-sd`、网络/文件共享依赖
- [ ] 旧 `old/` 不在编译产物中
- [ ] README/双 panel/键位/历史/字体要求文档更新
- [ ] 核心验收清单通过（[01-scope.md](01-scope.md) 第 8 节）
- [ ] 栈/Compose 忽略快捷键/选中/复制测试通过
- [ ] 中文宽度/字体手工验收
- [ ] 剪贴板本机 + SSH/OSC52 手工验收
- [ ] 终端恢复（正常/panic/Q）验证
- [ ] 历史权限/日志正文检查
- [ ] `--version` 输出正确
- [ ] 升级/旧身份迁移验证
- [ ] 已知限制写入 README（字体/裁剪/Windows）

## 9. 待确认

- OQ-25 Compose Enter 语义（已确认）。
- OQ-26 退出键（已确认：List `Q`）。
- OQ-27 datetime 格式（已确认）。
- OQ-28 取消输入后的选中项。
- OQ-29 剪贴板方案。
- OQ-30 状态 panel 高度。
- OQ-31 手动模式下自己发送是否自动跳最新（建议不跳）。
- 是否做 24h soak test。
- 是否在 CI 跑 macOS/Windows。
