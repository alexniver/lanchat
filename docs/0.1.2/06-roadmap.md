# 0.1.2 实现路线图

> 状态：M0–M2 已实现；M3 部分；M4 验收中
> 基线：0.1.1 已实现并冻结
> 关联：[01-scope.md](01-scope.md) · [02-commands.md](02-commands.md) · [03-transfer.md](03-transfer.md)

## 1. 总体顺序

```text
M0 命令输入/系统消息基础
        │
        ▼
M1 /send + /recv 控制流程（Offer/Accept/Timeout/Duplicate）
        │
        ▼
M2 文件数据分块传输 + 保存/重名/原子写
        │
        ▼
M3 多文件、提示语、进度/取消、错误处理
        │
        ▼
M4 安全审计、测试、验收、发布 0.1.2
```

**原则**：先让“命令能发出、请求能显示、超时能取消”，再传文件内容。文件传输失败不能影响聊天。

## 2. 里程碑

### M0 — 命令输入与系统消息基础

- `/` 在 List 层打开 Compose，预填 `/`。
- 命令解析器：已注册命令/未知命令/参数错误三种路径。
- 未知 `/...` 按普通消息发送。
- 系统消息显示（文件请求/成功/取消）与用户消息区分。
- 系统消息可选中/复制；不写历史（D-010/D-013）。
- 单元测试：命令解析、未知 fallback、系统消息显示。

**退出标准**：

- `/hello` 作为普通消息发送。
- `/send file` 参数缺失显示用法，不发送消息。
- 系统消息能与用户消息区分，`y/Y` 行为明确。

### M1 — 请求/接收控制

- `FileOffer` / `FileAccept` / `FileCancel` / `FileError` 控制消息。
- `/send file <path> to <peer>`：路径校验、对方解析、Offer 发送。
- `/recv file <peer>`：接受该对方全部 Pending。
- 10 分钟 Pending 超时；双方提示。
- 重复请求去重（对方 + basename）。
- 对方 capability 检查。
- 单元/集成测试：Offer/Accept/Timeout/Duplicate、对方离线/不支持。

**退出标准**：

- A 发请求，B 看到提示；B `/recv` 后 A/B 状态正确。
- 10 分钟超时（测试可用缩短常量）双方提示。
- 重复 a.png 被拦截。
- 旧版本 peer 不会收到文件帧。

### M2 — 文件数据与保存

- `FileChunk` / `FileComplete` 数据流；16 KiB 块。
- 发送端流式读取；接收端流式写入 `.part`。
- 保存目录 `data_local_dir/lanchat/files`。
- basename 安全过滤；重名 `[0]`/`[1]`。
- 临时文件 + rename；失败清理。
- 文件大小/数量限制。
- 成功/失败提示（含最终路径）。
- 测试：小文件、多块文件、重名、路径穿越、磁盘失败模拟。

**退出标准**：

- 1 MB/100 MB 文件可传（测试上限内）。
- 重名生成正确；不覆盖已有文件。
- 失败不留下半文件。
- 聊天在传输中仍可用（允许变慢）。

### M3 — 多文件与体验

- `/recv` 接受多个文件，逐文件传输。
- 多文件汇总提示/逐文件结果。
- 进度（可选）或开始/完成提示。
- 手动取消（可选）。
- 断线/退出时取消未完成请求，通知可见方。
- 提示语全面检查：短、清楚、可操作。

**退出标准**：

- A 发 3 个文件，B 一次 `/recv` 全部接收。
- 一个失败不影响其他（若实现）。
- 断线/超时不留悬挂请求。

### M4 — 安全、测试、发布

- 路径穿越/恶意文件名/超大文件/重复 Offer 测试。
- 大文件/多文件 soak。
- 0.1.1 的 73 个测试全部通过。
- `cargo fmt/clippy/test/build --release`。
- README/命令说明/已知限制更新。
- 版本号改为 0.1.2（用户确认后）。

## 3. 任务依赖

| 任务 | 依赖 |
| --- | --- |
| `/` 输入 | Compose |
| 命令解析 | 消息发送 |
| 系统消息 | MessageKind/历史策略 |
| Offer/Accept | 命令解析 + 协议 |
| 超时/去重 | Offer 状态 |
| 文件块 | 协议 + 文件 IO |
| 保存/重名 | 文件块 + 文件系统 |
| 多文件 | Accept + 队列 |
| 进度 | 文件块回调 |
| 测试 | 各层 |

## 4. 建议 PR 序列

1. `feat: slash command parser and system messages`
2. `feat: file offer/accept/cancel protocol`
3. `feat: pending timeout and duplicate guard`
4. `feat: file chunk transfer and atomic save`
5. `feat: collision naming and success/failure messages`
6. `feat: multi-file accept and progress`
7. `test: file transfer integration and security`
8. `docs: 0.1.2 usage/limitations`

## 5. 发布门槛

- [ ] `/` 输入、未知命令 fallback、参数错误提示正确。
- [ ] `/send`/`/recv` 全流程可用。
- [ ] 10 分钟超时（测试缩短）双方提示。
- [ ] 重复请求拦截。
- [ ] 多文件一次接收。
- [ ] 重名 `[n]`、不覆盖、原子写。
- [ ] 路径穿越/恶意文件名防护。
- [ ] 旧版本 capability 不兼容处理。
- [ ] 0.1.1 的 73 个测试通过。
- [ ] 提示语通俗、无内部错误码。
- [ ] fmt/clippy/test/build 通过。

## 6. 待确认

- OQ-12-07 传输阶段超时。
- OQ-12-08 `/recv file` 无参数行为。
- OQ-12-09 手动取消。
- OQ-12-11 SHA256。
- OQ-12-13 进度显示。
