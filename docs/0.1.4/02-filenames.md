# 跨平台文件名处理

> 状态：已实现
> 关联：[01-paths.md](01-paths.md) · [03-architecture.md](03-architecture.md)

## 1. 问题

接收文件名来自远端，不能直接 `dir.join(name)`：

- Linux 允许 `:`、`?`、`*` 等 Windows 非法字符；
- Windows 不允许 `< > : " / \ | ? *`，还会把结尾 `.`/空格静默去掉；
- Windows 保留设备名（`CON`、`NUL`、`COM1`…）不能作为文件名；
- `:` 还可能被解释为 NTFS alternate data stream；
- 文件名过长会失败；
- 路径分隔符/`..` 必须拒绝，防止路径穿越。

0.1.2 的 sanitizer 只拒绝 `/`、`\`、NUL 和控制字符，Windows 上会出问题。

## 2. 策略：可移植安全

在所有平台使用同一套“可移植安全”规则：

1. 只接受 basename；拒绝 `/`、`\`、NUL。
2. 拒绝空名、`.`、`..`。
3. 把 Windows 非法字符替换为 `_`：
   - `< > : " | ? *`
   - ASCII 控制字符
4. 去掉结尾的 `.` 和空格。
5. 如果清理后为空，使用 `file`。
6. Windows 保留设备名加 `_` 前缀。
7. 超过 255 字节时截断，尽量保留扩展名。
8. 写入前仍检查 files 目录、不覆盖、重名加 `[0]`/`[1]`。

> 这意味着 Linux 用户收到 `a:b.png` 会保存为 `a_b.png`。这是有意的可移植性取舍。

## 3. 保留名

Windows 保留名（大小写不敏感，带扩展名也算）：

```text
CON PRN AUX NUL
COM1..COM9
LPT1..LPT9
CLOCK$ CONIN$ CONOUT$
```

处理：

```text
CON      → _CON
con.txt  → _con.txt
LPT1.log → _LPT1.log
COM0.txt → COM0.txt   （COM0 不保留）
```

## 4. 长度

- 上限 255 字节（保守适配常见文件系统）。
- 先分离扩展名，再用剩余字节截断 stem，最后拼回扩展名。
- 截断按 UTF-8 字符边界，避免把中文切坏。
- 扩展名本身过长时退化为无扩展名。
- 截断后仍检查保留名/空名。

## 5. 重名

沿用 0.1.2：

```text
a.png → a[0].png → a[1].png
report → report[0]
```

- 先 sanitize，再找未占用名称。
- 使用 `try_exists` 实际查询文件系统；Windows/macOS 大小写不敏感时会自然避开 `A.png`/`a.png` 冲突。
- 不覆盖已有文件。
- `.part` 临时文件也使用 sanitize 后的名字，并用 transfer_id 避免并发冲突。

## 6. 原子写入

- 目标目录：`Paths.files_dir`。
- 临时文件：`.<safe_name>.<transfer_id>.part`。
- 写完后 `rename` 到最终名称。
- 失败/取消/超时删除 `.part`。
- 不自动执行文件，不解析内容。

## 7. 与协议校验的关系

- `validate_file_offer` 仍拒绝路径分隔符/NUL 和空名。
- 非法 Windows 字符不拒绝 Offer，而是在接收端 sanitize 后保存；用户看到最终路径。
- 文件名来自发送方 basename，不信任远端路径。

## 8. 测试

- `a:b?.png` → `a_b_.png`
- 结尾 `.`/空格 → 去掉
- `... ` → `file`
- `a/b.png`、`a\b.png` → 拒绝
- `CON`、`con.txt`、`LPT1.log` → 加 `_`
- `COM0.txt` 不变
- 300 字符 + `.png` → ≤255 字节且保留 `.png`
- 重名 → `[0]`
- 中文名保留

## 9. 待做

- Windows 真实文件系统测试（保留名/ADS/大小写）。
- macOS 大小写不敏感/Unicode 归一化测试。
- 超长路径（>260）专项测试。
- 可选：引入 `sanitize-filename` 做对照，但当前内置规则更可控。
