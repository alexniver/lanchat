# 测试

> 状态：Linux 已通过；Windows/macOS 待真机

## 已覆盖

- identity 默认 short_no=1、旧文件补号。
- naming `no1` 格式、空名、已有后缀替换。
- `/send to no1`、`/send to ip101`、`/recv no1`。
- 裸 `103` 不再匹配。
- 完整 node_id/IP/昵称。
- 成员/消息显示 `(no1)`。
- 文件提示使用短号。
- 短号冲突顺延/保存/更新（代码路径）。
- 0.1.5 的 111 个测试回归；当前 111。

## 手工

```text
A: /send file ... to no2
B: /recv file no1
```

- 文件保存；
- 双方显示 `(no1)`/`(no2)`；
- `ip101` 目标也可用（同网段）。

## 未覆盖

- 三节点同时启动的冲突竞态。
- Windows/macOS 真机。
- 短号复用后的历史歧义（from node_id 兜底）。
- 旧客户端完全不显示短号（预期）。
