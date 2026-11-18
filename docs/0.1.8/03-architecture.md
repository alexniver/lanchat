# 架构

## 解析器

`src/app/command.rs`：

```rust
ParsedCommand::SendFile { path, peer }
ParsedCommand::RecvFile { peer }
```

只改入口字符串：

```text
/sendfile ...  → parse_send_file
/recvfile ...  → parse_recv_file
/send file ... → parse_send_file (legacy)
/recv file ... → parse_recv_file (legacy)
/set name ...  → parse_setname (legacy)
```

内部 variant/Effect/NetCommand 不变。

## 提示

- `FileOfferReceived` 系统消息：

```text
alice (no1) 想发送文件 a.png。输入 /recvfile no1 接收。
```

- 帮助内容 `src/app/help.rs` 改为 `/sendfile`、`/recvfile`。
- README 示例更新。

## 不变

- 文件协议；
- 寻址；
- 保存；
- 超时/去重；
- 版本 0.1.8。
