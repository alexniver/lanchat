# 文件夹传递设计（已实现）

> 状态：已实现
> 实现方式：发送端临时 zip + 复用文件数据连接；接收端解压到临时目录后整体 rename。

## 1. 扫描与打包

`/sendfolder` 时在发送端：

- 递归遍历目录；
- 记录相对路径、文件大小、文件数、空目录数；
- 符号链接默认跳过（不跟随）；
- 把目录打包为临时 zip（Store，不压缩）；
- 不设单文件/文件夹大小、文件数、深度上限；
- 遇到不可读文件/目录 → 失败并提示，不静默跳过；
- 临时 zip 在传输完成/失败/取消后清理。

归档条目保持相对路径；根目录本身不写入 zip，接收端用 `root_name` 创建。

## 2. 请求

复用 `FileOffer`，新增可选 `folder` 元数据：

```rust
FolderMeta {
    root_name: String,
    total_size: u64,   // 解压后的总大小
    file_count: u32,
    empty_dirs: u32,
}
```

- `FileOffer.size` 是 zip 大小；
- `folder.is_some()` 表示文件夹；
- capability：`folder_transfer`；
- 旧 peer 不支持文件夹，`/sendfolder` 直接提示升级；
- 10 分钟请求超时、重复拦截沿用文件规则。

## 3. 数据连接

复用 0.1.6 的独立数据 TCP 连接：

- 一个文件夹一个 `FileDataHello`/传输；
- 数据内容是 zip 字节流，仍按 16 KiB 分块；
- 接收端先写临时 zip；
- 不阻塞聊天控制连接。

## 4. 磁盘空间检查

- 触发点：`/recvfile` / `/recvfolder` 接受前。
- 计算剩余待接收：
  - 单文件：文件大小；
  - 文件夹：`FolderMeta.total_size`（解压后）；
  - 一次接受多个：求和。
- 要求：`free_space(files_dir) >= ceil(remaining * 1.5)`。
- 不足：
  - 不进入传输；
  - 发送方收到 `FileError`，双方显示“磁盘空间不足，传送失败”；
  - 不留下临时文件。
- 无法读取磁盘空间 → 同样失败。
- 传输中磁盘写满 → 整个文件夹失败并清理。

## 5. 保存（文件夹整体原子）

- 接收 zip 到 `files_dir/.lanchat-folder-<id>.zip`；
- 解压到 `files_dir/.lanchat-folder-<id>.tmp/`；
- 每个相对路径逐段 sanitize；拒绝 `..`、绝对路径、路径穿越；
- 符号链接条目跳过；
- 空目录创建；
- 全部解压成功后，把临时根目录 rename 为最终根：
  - `holiday`；
  - 重名 `holiday[0]`、`holiday[1]`。
- 失败/取消：删除临时 zip 和临时根目录，不保留半成品。
- 不覆盖已有根目录。

## 6. 结果/提示

- 接收方请求提示：`想发送文件夹 holiday（N 个文件，total）… /recvfolder no1`。
- 开始：`开始接收来自 ... 的文件夹：holiday…`
- 完成：`接收完成：来自 ... 的文件夹 holiday 已保存到 ...`
- 发送完成：`发送完成：文件夹 holiday 已发送给 ...`
- 磁盘不足/失败：双方提示。

## 7. 安全边界

- 仅可信局域网，明文；
- 文件名/路径逐段 sanitize；
- 不跟随符号链接；
- 不自动执行；
- 协议帧 64 KiB、文件名 255 字节等边界保留；
- 不设应用层发送大小/数量/深度上限。

## 8. 与单文件

- `/sendfile` 逻辑不变；
- 文件夹使用独立命令和 `folder` 元数据；
- 单文件/文件夹待接收分开：`/recvfile` 只接文件，`/recvfolder` 只接文件夹。
