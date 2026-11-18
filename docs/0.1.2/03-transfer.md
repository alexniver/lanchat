# 文件传递设计

> 状态：已实现，验收中
> 关联：[01-scope.md](01-scope.md) · [02-commands.md](02-commands.md) · [04-ui.md](04-ui.md)
> 目标：点对点传输，不是目录同步

## 1. 状态机

### 1.1 发送方

```text
创建请求
  │
  ▼
Pending（等待接收，10 分钟）
  │ 对方 /recv
  ▼
Transferring（正在发送）
  │ 完成 / 失败 / 对方断开
  ▼
Completed / Failed / Cancelled / TimedOut
```

### 1.2 接收方

```text
收到 Offer
  │
  ▼
Pending（等待用户 /recv file <对方>）
  │ 用户 /recv
  ▼
Transferring（正在接收）
  │ 完成 / 失败 / 对方断开
  ▼
Completed / Failed / Cancelled / TimedOut
```

- 请求超时只作用于 **Pending**（等待接收）阶段。
- 一旦接受，转入 Transferring；请求的 10 分钟超时停止。
- Transferring 阶段的卡死/断线用传输超时处理（OQ-12-07）。

## 2. 传输通道（已确认：独立数据 TCP 连接）

### 2.1 分工

- **控制消息**（Offer/Accept/Cancel/Timeout/最终确认）走 0.1.1 已有的 peer 连接。
- **文件内容**走新的独立 TCP 连接，避免大文件阻塞聊天。
- 数据连接复用 peer 的监听端口，不额外开端口；accept 后按首帧区分：
  - 首帧 `Hello` → 普通 peer 连接；
  - 首帧 `FileDataHello` → 文件数据连接。
- 数据连接不参与 peer 身份/在线列表，也不会触发重复连接逻辑。

### 2.2 数据连接握手

```text
A（发送方） ── TCP connect ──► B（接收方）的监听端口
A ── FileDataHello { node_id, transfer_ids, nonce } ──► B
B ── FileDataReady { transfer_ids } ───────────────────► A
A ── FileChunk... ─────────────────────────────────────► B
A ── FileDataDone { transfer_ids } ────────────────────► B
B ── FileComplete/FileError（控制连接） ────────────────► A
```

- `FileDataHello` 只允许发送**已经 Accepted** 的 transfer_id。
- 接收方校验：
  - node_id 与 peer 一致；
  - transfer_id 在已接受集合中；
  - 文件总大小/待接收上限；
  - 同一 transfer_id 没有正在传输。
- 一个 `/recv` 批次可以复用一个数据连接传输该发送方的全部文件；也可以每个文件一条连接（建议批次复用）。
- 数据连接失败/超时：通过控制连接发 `FileError`/`FileCancel`，双方进入失败/取消状态。

### 2.3 数据帧

- 沿用 4 字节大端长度 + JSON。
- `FileChunk` 的 `data` 为 base64，原始块 16 KiB（base64 后约 22 KiB，最大帧 64 KiB）。
- 数据连接只包含文件传输帧，不混聊天消息，因此不会拖慢聊天。
- 数据连接关闭即本次传输结束；未完成的请求清理 `.part` 并报错。

## 3. 协议消息

在 0.1.1 的 `WireMessage` 中增加（仅对支持 `file_transfer` capability 的 peer 使用）：

```rust
FileOffer {
    transfer_id: String,     // UUID
    name: String,            // 文件 basename
    size: u64,               // 字节数
    // 0.1.2 暂不做 sha256；后续可加
}

// 仅数据连接首帧
FileDataHello {
    node_id: String,
    transfer_ids: Vec<String>,
    nonce: String,
}
FileDataReady {
    transfer_ids: Vec<String>,
}
FileDataDone {
    transfer_ids: Vec<String>,
}

FileAccept {
    transfer_ids: Vec<String>, // 该对方全部待接收文件
}

FileChunk {
    transfer_id: String,
    seq: u64,
    data: String,            // base64，原始块 <= 16 KiB
}

FileComplete {
    transfer_id: String,
}

FileCancel {
    transfer_id: String,
    reason: String,
}

FileError {
    transfer_id: String,
    code: String,
    message: String,
}
```

### 3.1 capability

- 0.1.2 的 Hello `capabilities` 加入 `"file_transfer"`。
- 只有对端 capability 包含 `file_transfer` 时才允许 `/send file`。
- 对端是 0.1.1：提示“对方版本不支持文件传递，请升级后重试”。
- 不发送文件帧给不支持的对端，避免旧客户端解析失败断开连接。
- 协议版本可以继续用 1；用 capability 做能力协商。

### 3.2 帧顺序

```text
控制连接：
A ── FileOffer(transfer_id, name, size) ──► B
B ── FileAccept([transfer_id...]) ─────────► A

数据连接（新 TCP）：
A ── FileDataHello(node_id, transfer_ids) ► B
B ── FileDataReady(transfer_ids) ─────────► A
A ── FileChunk(transfer_id, seq=0) ───────► B
A ── FileChunk(transfer_id, seq=1) ───────► B
...
A ── FileDataDone(transfer_ids) ──────────► B

控制连接：
B ── FileComplete(transfer_id) ───────────► A   （确认落盘）
```

- 多文件时，B 的 FileAccept 可以一次带上该发送方所有 transfer_id；A 逐文件传输。
- 每个 transfer_id 独立状态；单个失败不影响其他文件（建议）。
- 接收方确认落盘后再回 FileComplete，发送方据此显示“发送完成”。

## 4. 请求超时

- Pending 超时：**10 分钟**，从 FileOffer 发出/收到开始计时。
- 超时后：
  - 双方状态转 TimedOut；
  - 双方显示“文件 a.png 的请求已取消：超过 10 分钟没有接收”；
  - 发送方清理该请求；接收方从待接收列表移除。
- 接受后不再用 10 分钟请求超时。
- 传输阶段超时（建议）：
  - 连续 60 秒没有收到任何 FileChunk → 失败；
  - 或总时长上限（OQ-12-07）。
- 超时精度：定时检查，误差 < 5 秒。

## 5. 重复请求

- 去重 key：`(对方 node_id, 文件 basename)`。
- 在 Pending 或 Transferring 状态时，再次发送同名文件 → 拒绝：
  `a.png 的请求已经发给 192.168.1.103 了，请等对方接收完成后再发。`
- 完成后（Completed/Failed/Cancelled/TimedOut）允许再次发送。
- 不同对方可以同时发同名文件。
- 同一对方不同文件名可以同时待接收。
- 收到 FileOffer 时接收方也做同样去重，防止重复事件。

## 6. 多文件

- 一次 `/send` 只发一个文件；多个文件通过多次命令排队。
- 接收方 `/recv file 101` 接受 101 的全部 Pending 请求。
- 传输顺序建议按请求创建顺序（或文件名排序，需确定）。
- 多文件提示可汇总：

```text
192.168.1.101 想发送 2 个文件：a.png（1.2 MB）、b.jpg（340 KB）。
输入 /recv file 101 全部接收。
```

- 每个文件独立成功/失败；最后一个完成后显示汇总。

## 7. 保存与命名

### 7.1 目录

- 使用 `dirs::data_local_dir()/lanchat/files`：
  - Linux：`~/.local/share/lanchat/files`
  - macOS/Windows：系统数据目录下的 `lanchat/files`
- 目录不存在时自动创建（Unix 权限 `0700`）。
- 只写这个目录，不跟随远端路径。

### 7.2 文件名安全

- 只取 basename；拒绝包含 `/`、`\`、NUL 的名称。
- 拒绝 `.`、`..`、空名。
- 去除控制字符；保留中文/空格/常见符号。
- 文件名过长截断（保留扩展名）或拒绝；上限 255 字节（D-012）。
- 不允许覆盖已有文件。

### 7.3 重名

用户要求“文件名后加 `[0]`、`[1]` 等自增数字”。建议：

```text
a.png     → a[0].png
a.png     → a[1].png
report    → report[0]
photo.jpg → photo[1].jpg
```

- 从 `[0]` 开始找第一个未占用的名字。
- 同名多个文件连续接收时，依次得到 `[0]`、`[1]`…
- 大小写按文件系统实际行为处理。

### 7.4 原子写入

- 先写入临时文件：`.a.png.<transfer_id>.part`（同目录）。
- 写入完成、flush 后 `rename` 到最终名称。
- rename 失败（目标被占用）重新选名。
- 失败/超时/取消时删除 `.part`。
- 文件权限（Unix）建议 `0600`。

## 8. 限制

| 项 | 建议 |
| --- | --- |
| 单文件大小 | 1 GiB（D-012） |
| 数据块 | 原始 16 KiB / base64 后约 22 KiB |
| 单次待接收 | 每个发送方 20 个文件（建议） |
| 总待接收 | 100 个（建议） |
| 文件名 | ≤ 255 字节（UTF-8 截断保留扩展名） |
| 请求超时 | 10 分钟 |
| 传输空闲超时 | 60 秒（建议） |

- 超限在命令阶段/FileOffer 阶段拒绝并提示。
- 不一次性把整个文件读进内存；按块流式读写。

## 9. 错误/取消

- 发送方读取文件失败 → FileError/Cancel，双方提示。
- 接收方磁盘满/写入失败 → 删除 `.part`，FileError，发送方显示失败。
- 任一方断开 → 未完成传输取消；双方提示（对端可能已离线，能显示则显示）。
- 接收方拒绝/超时 → FileCancel/TimedOut。
- 传输中不自动重试；用户可重新 `/send`。

## 10. 持久化

- 0.1.2 建议 Pending/Transferring 只保存在内存。
- 程序退出/重启后未完成请求失效；接收方重启后不能继续 `/recv` 旧请求。
- 系统消息不写历史（D-013）；Pending 请求只在当前运行有效。
- 已接收文件本身保存在 files 目录，永久保留（用户自行管理）。

## 11. 安全

- 沿用 0.1.1 明文局域网边界；文件内容不加密。
- 必须用户明确 `/recv`；不自动接收。
- 只写 files 目录；拒绝路径穿越和绝对路径。
- 不覆盖；不执行；不解析文件内容。
- 限制大小/数量/块大小，防止内存/磁盘耗尽。
- 接收方可以只信任自己发起的 `/recv`；不因 FileOffer 自动创建文件。
- 不做文件哈希校验（0.1.2）。TCP 保证传输完整性；落盘后可用系统工具校验。若需要，后续加 SHA256（OQ-12-11）。

## 12. 已确认

- 传输通道：独立数据 TCP 连接（D-011）。
- 单文件 1 GiB、每发送方 20 个、全局 100 个（D-012）。
- 重名格式 `a[0].png`（D-015）。
- 系统消息不写历史（D-013）。

仍待确认：接受后传输阶段超时（OQ-12-07）、`/recv file` 无参数（OQ-12-08）、手动取消（OQ-12-09）、SHA256（OQ-12-11）、进度（OQ-12-13）。
