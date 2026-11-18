# 0.1.8 范围与验收

> 状态：已实现

## 目标

- `/sendfile <路径> to <对方>` 替代 `/send file ...`。
- `/recvfile <对方>` 替代 `/recv file ...`。
- 文件请求提示、帮助、README 使用新命令。
- 参数/寻址规则保持不变。
- 旧命令作为别名兼容。

## 非目标

| 不做 | 说明 |
| --- | --- |
| 修改参数语法 | 仍是 `<路径> to <对方>` |
| 修改寻址 | no/ip/node_id/IP/昵称 |
| 删除旧命令 | 0.1.8 保留兼容 |
| 新文件协议 | 只改命令名 |
| 自动补全 | 不做 |

## 规则

- `/sendfile` 无参数 → 用法提示。
- `/sendfile a.png` 缺 `to` → 用法提示。
- `/recvfile` 无参数 → 用法提示。
- `/sendfilex` 未注册 → 普通消息。
- 旧 `/send file`、`/recv file`、`/set name` 仍按旧规则工作。
- 帮助/README 不展示旧命令。

## 验收

- [x] `/sendfile ~/a.png to no1` 能发请求。
- [x] `/recvfile no1` 能接收。
- [x] 文件提示显示 `/recvfile no1`。
- [x] 帮助/README 只写新命令。
- [x] 旧 `/send file`、`/recv file`、`/set name` 仍可用。
- [x] 0.1.7 的 116 个测试回归；当前 118。
