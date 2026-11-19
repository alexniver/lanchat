# 文件夹传递设计

> 状态：讨论中

## 1. 扫描

发送方：

- 递归遍历目录；
- 记录相对路径、文件大小、目录（含空目录）；
- 跳过符号链接（不跟随）；
- 遇到不可读文件/目录 → 失败并提示，不静默跳过；
- 记录文件数/总大小/深度用于显示和磁盘检查，不设发送上限。

限制：

- **取消**发送端的单文件 1 GiB、文件夹总大小、文件数、深度限制。
- 仍保留协议帧 64 KiB、文件名 255 字节、路径安全等边界。
- 接收端在 `/recvfile` / `/recvfolder` 接受前检查目标磁盘剩余空间。

## 2. 请求

建议新增控制消息：

```rust
FolderOffer {
    transfer_id: String,
    root_name: String,
    file_count: u32,
    total_size: u64,
    empty_dirs: u32,
}
```

- 控制连接发送；
- 接收方显示文件数/总大小；
- 请求 10 分钟超时；
- 重复 key：`(对方 node_id, root_name)`；
- `/recvfolder` 接受该发送方全部 Pending。

可选的 manifest：

- 文件名/相对路径太多时，用 `FolderManifest` 分块发送；
- manifest 可随 Offer 或数据连接分块发送；不再用 1000 文件硬限制。

## 3. 数据连接

复用 0.1.6 独立数据 TCP 连接：

```text
FolderDataHello { node_id, transfer_ids, nonce }
FolderDataReady { transfer_ids }
FolderFileHeader { folder_id, relative_path, size }
FolderFileChunk { ... }
FolderFileDone { ... }
FolderDataDone { transfer_ids }
```

或者复用文件块协议，增加 `relative_path` 字段。设计目标：

- 一个文件夹一条数据连接；
- 逐文件顺序传输；
- 单文件 `FileChunk` 16 KiB；
- 控制连接回 `FolderComplete`。

## 4. 磁盘空间检查

- 触发点：接收方输入 `/recvfile <对方>` / `/recvfolder <对方>` 时。
- 计算剩余待传字节：
  - 单文件：该文件大小；
  - 文件夹：该请求总大小（或尚未接收的剩余大小）；
  - 一次接受多个请求：求和。
- 要求：`free_space(files_dir) >= ceil(remaining * 1.5)`。
- 不足：
  - 不进入传输；
  - 双方显示“磁盘空间不足，传送失败”；
  - 请求失败/取消，不留下临时文件。
- 无法读取磁盘空间：同样失败并提示，不盲目接收。
- 传输中磁盘写满：按单文件/文件夹失败处理，清理临时目录。

## 5. 保存

- 目标根目录最终名称：`<root>`，重名 `<root>[0]`、`<root>[1]`。
- 先写到 `files/.lanchat-folder-<transfer_id>/` 临时目录；
- 所有文件完成后 rename 到最终根；
- 失败/取消删除临时目录；
- 空目录也创建；
- 每个相对路径逐段 sanitize（Windows 非法字符/保留名）；
- 拒绝 `..`、绝对路径、盘符、路径穿越；
- 不保留权限/时间戳；
- 不覆盖已有根目录。

## 6. 进度/结果

- 开始：`开始接收文件夹 holiday（12 个文件）…`
- 进度：`接收 holiday：5/12`（可选）
- 完成：路径 + 文件数。
- 单文件失败：整个文件夹失败（建议原子），清理临时目录。
- 符号链接跳过：请求提示里说明。

## 7. 安全

- 仅可信局域网，明文；
- 文件夹名/相对路径逐段 sanitize；
- 不跟随符号链接；
- 不再设应用层总大小/文件数/深度上限；
- 不自动执行；
- 失败清理临时目录。

## 8. 单文件复用

- `/sendfile` 逻辑不变；
- 文件夹内部文件不复用单文件请求 UI（避免 1000 条请求）；
- 文件夹是独立 Offer/接受/数据流。
