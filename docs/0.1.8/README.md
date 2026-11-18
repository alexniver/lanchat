# lanchat 0.1.8 文件命令改名

> 状态：**0.1.8 已实现，验收中**（0.1.7 已冻结）
> 版本：0.1.8
> 创建日期：2026-11-19

## 目标

把文件传递命令从：

```text
/send file <路径> to <对方>
/recv file <对方>
```

改为更短、更好输入的：

```text
/sendfile <路径> to <对方>
/recvfile <对方>
```

## 示例

```text
/sendfile ~/Download/a.png to no1
/sendfile ~/Download/a.png to ip101
/recvfile no1
```

## 兼容策略

- 新命令是主语法，帮助/README/文件提示只写新命令。
- 旧命令保留为兼容别名：

```text
/send file ... to ...
/recv file ...
/set name ...
```

- 旧命令不显示在帮助中，后续版本可评估移除。
- 参数、路径、寻址规则完全不变。

## 范围

| 能力 | 0.1.8 | 说明 |
| --- | --- | --- |
| `/sendfile` | ✅ 做 | 主语法 |
| `/recvfile` | ✅ 做 | 主语法 |
| 旧 `/send file` | 🟡 兼容 | 不宣传 |
| 旧 `/recv file` | 🟡 兼容 | 不宣传 |
| 旧 `/set name` | 🟡 兼容 | 不宣传 |
| 文件提示 `/recvfile no1` | ✅ 做 | 新提示 |
| 帮助/README | ✅ 更新 | 只写新命令 |
| 参数/寻址 | ✅ 不变 | no/ip/node_id/IP/昵称 |
| 版本 | 0.1.8 | |

## 文档

- [01-scope.md](01-scope.md)
- [02-command.md](02-command.md)
- [03-architecture.md](03-architecture.md)
- [04-testing.md](04-testing.md)
- [05-roadmap.md](05-roadmap.md)
- [decisions.md](decisions.md)
