# lanchat 0.1.9 文件夹传递与帮助提示

> 状态：**需求已确认，待实现**（0.1.8 已冻结）
> 目标版本：0.1.9
> 创建日期：2026-11-20

## 目标

新增两个能力：

1. `/sendfolder <目录> to <对方>`：发送整个文件夹。
2. 状态栏最右侧显示 `[?]帮助`。

## 文件夹传递

发送方：

```text
/sendfolder ~/Pictures/holiday to no1
```

接收方：

```text
/recvfolder no1
/recv folder no1   # 兼容写法
```

接收后保留目录结构，保存到：

```text
<files_dir>/holiday/...
```

如果 `holiday` 已存在：

```text
holiday[0]/...
holiday[1]/...
```

## 状态栏

```text
本机 alice (no1) · 在线 2 · 消息 3 · 选中 3/3 · 自动 · ● online        [?]帮助
```

- `[?]帮助` 固定在最右侧。
- 只在消息列表层显示；`?` 在输入 panel/成员栏/帮助页不是帮助入口。
- `?` 被 keymap 改键时，显示当前键位，例如 `[g]帮助`；未绑定则隐藏。

## 范围速览

| 能力 | 0.1.9 | 说明 |
| --- | --- | --- |
| `/sendfolder` | ✅ 做 | 递归发送目录 |
| `/recvfolder` | ✅ 做 | 接收指定发送方全部待接收文件夹 |
| 保留目录结构 | ✅ 做 | 相对路径 |
| 空目录 | ✅ 做 | 保留空目录 |
| 符号链接 | ❌ 默认跳过 | 不跟随，避免循环/越界 |
| 重名根目录 | ✅ 做 | `holiday[0]` |
| 10 分钟请求超时 | ✅ 做 | 复用文件规则 |
| 接收端磁盘检查 | ✅ 做 | 剩余空间 ≥ 1.5× |
| 发送大小/数量限制 | ❌ 取消 | 接收端做磁盘检查 |
| 文件夹进度 | 🟡 可选 | 至少显示文件数 |
| `[?]帮助` | ✅ 做 | List 层最右 |
| 旧命令兼容 | ✅ 做 | `/sendfolder` 新命令 |

## 文档

- [01-scope.md](01-scope.md)
- [02-command.md](02-command.md)
- [03-transfer.md](03-transfer.md)
- [04-status.md](04-status.md)
- [05-open-questions.md](05-open-questions.md)
- [06-roadmap.md](06-roadmap.md)
- [decisions.md](decisions.md)

## 与 0.1.8 的关系

- 单文件 `/sendfile`、`/recvfile` 保持不变。
- 文件夹是独立的 `/sendfolder`、`/recvfolder`，不与单文件混用。
- 复用独立数据 TCP 连接、保存目录、重名策略、10 分钟超时。
- 状态栏本机名/在线/消息/选中/模式保持不变，只增加最右帮助提示。
