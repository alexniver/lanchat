# 命令语法

## 发送文件夹

```text
/sendfolder <本地目录> to <对方>
```

示例：

```text
/sendfolder ~/Pictures/holiday to no1
/sendfolder "./my folder" to ip101
```

- `<本地目录>`：`~`、相对、绝对、引号。
- 必须是目录；普通文件请用 `/sendfile`。
- `<对方>` 与 0.1.8 相同：`no1`、`ip101`、完整 node_id/前缀、完整 IP、昵称。
- 一次一个文件夹。

## 接收文件夹

```text
/recvfolder <对方>
/recv folder <对方>   # 兼容写法
```

- 接收该发送方全部待接收文件夹；`/recv folder` 等价。
- 不与 `/recvfile` 混用；单文件和文件夹分开。
- 没有待接收文件夹时提示。

## 提示语

发送方：

```text
已向 bob (no2) 发送 holiday 文件夹的接收请求（12 个文件，34.2 MB），等待接收（10 分钟内有效）。
```

接收方：

```text
alice (no1) 想发送文件夹 holiday（12 个文件，34.2 MB）。输入 /recvfolder no1 接收。
```

成功：

```text
接收完成：来自 alice (no1) 的文件夹 holiday 已保存到 .../files/holiday。
```

重名：

```text
接收完成：holiday 已存在，已另存为 holiday[0]。
```

## 兼容

- `/sendfolder` 是 0.1.9 新命令。
- 不保留 `/send folder` 兼容别名（新命令没有旧历史）；如需要可加。
- `/recvfolder` 同理。
