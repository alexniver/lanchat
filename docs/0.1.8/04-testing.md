# 测试

## 覆盖

- `/sendfile` 正常参数。
- `/recvfile` 正常参数。
- 缺参数 → 用法。
- `/sendfilex` → 普通消息。
- 旧 `/send file`/`/recv file`/`/set name` 兼容。
- App `/sendfile` 产生 Effect。
- 文件提示 `/recvfile no1`。
- 0.1.7 的 116 个测试回归；当前 118。

## 手工

```text
/sendfile ~/a.png to no1
B: /recvfile no1
```

- 文件保存；
- 提示新命令。

## 未覆盖

- 旧命令移除后的迁移（0.1.8 不移除）。
- 命令补全/别名 UI（不做）。
