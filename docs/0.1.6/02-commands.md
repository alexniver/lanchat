# 文件传递命令寻址

> 状态：已实现

## 1. 语法

```text
/send file <路径> to <目标>
/recv file <目标>
```

`<目标>`：

| 写法 | 含义 |
| --- | --- |
| `no1` | 短号 1 |
| `ip101` | IPv4 最后一段 101 |
| `19b9b31e` | node_id 唯一前缀 |
| `19b9b31e-c81d-...` | 完整 node_id |
| `192.168.1.101` | 完整 IPv4 |
| `bob` | 在线昵称（大小写不敏感） |

- **不再支持**裸 `103`（IP 最后一段）；必须写 `ip103`。
- 短号必须带 `no` 前缀，避免和 IP 数字混淆。
- 多个匹配 → 报歧义，要求完整 node_id 或完整 IP。
- 文件提示里的 `/recv file` 使用 `no1`（有短号）或 node8（老 peer）。

## 2. 示例

```text
/send file ~/Download/a.png to no1
/send file ~/Download/a.png to ip101
/send file ~/Download/a.png to 19b9b31e
/recv file no1
/recv file ip101
```

## 3. 解析顺序

1. `no<u32>`：短号精确匹配；
2. `ip<u8>`：IPv4 最后一段匹配；
3. 完整 node_id；
4. node_id 唯一前缀；
5. 完整 IP；
6. 昵称。

## 4. 显示

```text
alice (no1) 想发送文件 a.png（1.2 MB）。输入 /recv file no1 接收。
```

- 短号来自 peer 的 Hello/ShortNoUpdate。
- 短号冲突顺延后，后续提示使用新短号。
- 历史消息保留当时显示名。
