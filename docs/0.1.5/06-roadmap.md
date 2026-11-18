# 0.1.5 实现路线图

> 状态：已实现，验收中
> 基线：0.1.4

## 1. 顺序

```text
M0 naming 模块（node8）
      │
      ▼
M1 PeerView/self display
      │
      ▼
M2 消息/成员/文件提示
      │
      ▼
M3 node_id 寻址（send/recv）
      │
      ▼
M4 兼容/测试/版本 0.1.5
```

## 2. 已完成

### M0 — naming

- `short_node_id` 8 位。
- `display_name(base, node_id)`。
- 空名/控制字符/已有后缀替换。
- 单元测试。

### M1 — 显示名

- `PeerView.display_name`。
- `PeerUp` 组合。
- `self_display_name`。
- `PeerDown` 使用显示名。

### M2 — UI

- 本地消息用 self display。
- 远端消息用 peer display。
- 成员栏使用 display。
- 文件提示使用 display + `/recv file <node8>`。
- 历史保存 display。

### M3 — node_id 寻址

- `resolve_peer` 完整 node_id。
- 唯一前缀。
- 完整 IP/昵称保留。
- 移除 IP 最后一段。
- `/send`/`/recv` 共用。
- 文件请求提示用 node8。

### M4 — 测试/版本

- naming 单测。
- App 显示/寻址测试。
- `/send` node_id 目标测试。
- UI 测试。
- 0.1.4 的 103 个测试回归；当前 111。
- 版本 0.1.5。

## 3. 发布门槛

- [x] `alice@arch (19b9b31e)` 格式。
- [x] 自己/远端消息带 node8。
- [x] 成员栏带 node8。
- [x] `/send file ... to <node8>`。
- [x] `/recv file <node8>`。
- [x] 完整 node_id 可用。
- [x] `103` 不再作为 IP 末位匹配。
- [x] 历史保存 display。
- [x] 0.1.4 103 个测试回归；当前 111。
- [x] fmt/clippy/test/release。
- [x] 双实例手工验证（固定 node_id 的本地两实例）。

## 4. 风险

- 8 位前缀冲突概率低，但需完整 ID 兜底。
- 旧 peer 看不到后缀（版本差异）。
- 同一 identity 复制导致 node_id 冲突，不是本功能能解决。
- 显示名写入历史后，node_id 改名会导致新旧显示不同（历史事实）。
