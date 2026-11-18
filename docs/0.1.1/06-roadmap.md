# 0.1.1 实现路线图

> 状态：M0–M3 已实现；M4 验收中
> 基线：0.1.0 已实现并冻结
> 关联：[01-scope.md](01-scope.md) · [02-network.md](02-network.md) · [04-architecture.md](04-architecture.md)

## 1. 总体顺序

```text
M0 版本/文档/回归基线
        │
        ▼
M1 协议与传输（帧/握手/聊天）
        │
        ▼
M2 发现与连接管理（mDNS/手动/重连）
        │
        ▼
M3 App/UI 集成（在线状态/远端消息/历史/选中）
        │
        ▼
M4 可靠性、验收、发布 0.1.1
```

**原则**：先协议单测，再 manager 集成，最后接 App/UI。每一步都跑 0.1.0 的 57 个测试，避免联网改动回归本地交互。

## 2. 里程碑

### M0 — 版本、文档、回归基线

- `Cargo.toml` 版本改为 `0.1.1`；CLI help/`--version` 输出 0.1.1。
- 建立 `docs/0.1.1/`（本文档集）。
- 确认 0.1.0 测试全部通过，作为回归基线。
- 0.1.1 功能默认关闭/或本地模式可运行，避免半成品网络影响本地使用。
- 设计 `[network]` 配置与 `--no-network`。

**退出标准**：

- 0.1.0 的 57 个测试仍通过。
- `cargo run -- --version` 输出 0.1.1。
- 无网络配置时界面与 0.1.0 一致。

### M1 — 协议与传输

- `protocol.rs`：帧、Hello/Ready/Chat/Ping/Pong/Bye/Error。
- 长度前缀编码；最大帧 64 KiB；坏帧关闭连接。
- 握手：magic/version/node_id/name/Ready。
- 消息校验/去重；发送失败事件。
- 单元测试：roundtrip、坏 JSON、超长/截断、版本不匹配、重复身份。

**退出标准**：

- 两个 `NetManager` 通过 localhost 连接、握手、互发 Chat。
- 协议测试覆盖正常/异常路径。
- 无 `unwrap`/panic 在可恢复错误路径上。

### M2 — 发现与连接管理

- `discovery.rs`：mDNS 注册/browse；自身按 node_id 过滤；手动 seeds。
- `manager.rs`：peer 状态机、连接去重、心跳、超时。
- `reconnect.rs`：1s→30s 退避 + 抖动；手动断开停止。
- `--connect`/`--port`/`--no-network` CLI。
- 集成测试：重复连接、断线重连、多 peer 广播。

**退出标准**：

- 同机两实例不同端口/配置目录可手动互连。
- 同局域网 mDNS 手工验收（5s 内）。
- 重复 node_id 只保留一条连接。
- 断网 30s 内自动重连。

### M3 — App/UI 集成

- `App` 增加 peers/connection/unread。
- `NetEvent` 接入主循环；`Effect::Net` 发送命令。
- 远端 Chat 入列/入历史；自动/手动选中语义。
- 状态 panel `在线 N`（含本机）/连接状态；手动模式“新消息”提示。
- 右侧成员侧边栏作为新 Layer：`Space u` 开关，`j/k` 选择，`Esc` 关闭。
- 历史增加可选 `from`，兼容旧记录。
- 发送失败标记；无在线节点提示；节点加入/离开走状态/toast。

**退出标准**：

- 两端消息格式/历史正确。
- 自动模式跟随最新；手动模式不跳。
- Compose 期间收消息不打断；关闭后按模式选中。
- 0.1.0 本地交互无回归。

### M4 — 可靠性、验收、发布

- 心跳/重连 soak；错误路径审计。
- 两机局域网验收；mDNS 不可用手动连接。
- 安全声明/README/`:help security`（若做命令）。
- 24h 运行（建议）。
- `cargo fmt/clippy/test/build --release`。
- 打 tag `v0.1.1`（用户确认后）。

## 3. 任务依赖

| 任务 | 依赖 |
| --- | --- |
| 协议帧 | 无 |
| 握手 | 协议帧 |
| manager | 协议帧 + discovery |
| 重连 | manager 状态机 |
| App NetEvent | manager |
| 远端消息/历史 | App 消息模型 |
| 成员面板 | App Layer + peers |
| 在线状态 | peers/connection |
| 集成测试 | manager + App 假事件 |

## 4. 建议 PR 序列

1. `chore: bump to 0.1.1 and add networking docs`
2. `feat: protocol frames and handshake`
3. `feat: net manager with localhost integration tests`
4. `feat: mdns discovery and manual seeds`
5. `feat: heartbeat, reconnect, duplicate connection`
6. `feat: app net events and remote messages`
7. `feat: online status and optional member panel`
8. `feat: history from field and remote persistence`
9. `test: network integration and 0.1.0 regression`
10. `docs: README/security/known limitations`

## 5. 0.1.1 发布门槛

- [ ] 0.1.0 的 57 个测试通过。
- [ ] 协议/manager 集成测试通过。
- [ ] 两机 mDNS + 手动连接验收。
- [ ] 断网/重连/重复连接验收。
- [ ] 自动/手动选中在网络消息下正确。
- [ ] 历史远端消息重启可回看，旧格式兼容。
- [ ] 日志无正文；历史 `0600`。
- [ ] `cargo fmt/clippy/test/build --release` 通过。
- [ ] README 安全声明/已知限制更新。

## 6. 已确认

0.1.1 的 18 个问题已全部确认，见 [05-open-questions.md](05-open-questions.md) 与 [decisions.md](decisions.md)。实现按 M0–M4 推进。
