# 命令语法

## 新命令

```text
/sendfile <路径> to <对方>
/recvfile <对方>
```

### 示例

```text
/sendfile ~/Download/a.png to no1
/sendfile "./my photo.png" to ip101
/sendfile a.png to 19b9b31e
/recvfile no1
```

### 参数

- 路径：`~`、相对、绝对、引号；必须普通文件。
- 对方：`no1`、`ip101`、完整 node_id、node_id 前缀、完整 IP、昵称。
- 裸 `103` 仍不匹配；必须 `ip103`。

## 旧命令（兼容）

```text
/send file <路径> to <对方>
/recv file <对方>
/set name <新名字>
```

- 仍可解析；
- 不写入帮助/README；
- 文件提示只使用新命令；
- 后续版本可移除。

## 错误提示

```text
用法：/sendfile <文件路径> to <对方>
用法：/recvfile <对方>
```

## 与 0.1.7 相同

- 10 分钟超时；
- 重复请求拦截；
- 多文件接收；
- 保存路径/重名；
- 文件提示使用短号。
