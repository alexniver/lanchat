# 0.1.4 实现路线图

> 状态：M0–M3 已实现；M4 验收中
> 基线：0.1.3 已实现
> 关联：[01-paths.md](01-paths.md) · [02-filenames.md](02-filenames.md)

## 1. 顺序

```text
M0 Paths.files_dir
      │
      ▼
M1 home_dir + dunce
      │
      ▼
M2 跨平台文件名 sanitizer
      │
      ▼
M3 帮助/README 动态路径
      │
      ▼
M4 测试、Windows/macOS 验收、0.1.4 发布
```

## 2. 里程碑

### M0 — Paths

- `Paths.files_dir`。
- `ensure_dirs` 创建 files。
- `main` 使用 `files_dir`。
- 测试构造 `Paths` 补齐字段。

**完成**。

### M1 — home/dunce

- `resolve_path` 使用 `dirs::home_dir()`。
- 支持 `~/`、`~\`。
- `dunce::canonicalize`。
- 去掉 XDG 专属错误文案。
- 测试 `~`。

**完成**。

### M2 — 文件名

- Windows 非法字符替换。
- 保留设备名。
- 结尾点/空格。
- 255 字节截断，保留扩展名。
- 重名/临时文件继续使用 sanitize。
- 单元测试。

**完成**。

### M3 — 帮助/文档

- `help::lines` 接收 `files_dir`。
- 帮助显示实际路径。
- README 平台路径表。
- docs/0.1.4。

**完成**。

### M4 — 验收

- 全量回归。
- fmt/clippy/test/release。
- Windows 真实机器验收。
- macOS 真实机器验收。
- 版本号 0.1.4。
- 可选：长路径/UNC 专项。

**进行中**（Linux 已通过；Windows/macOS 待测）。

## 3. 任务依赖

| 任务 | 依赖 |
| --- | --- |
| files_dir | Paths |
| main 使用 | files_dir |
| home_dir | command |
| sanitizer | file transfer |
| 帮助路径 | files_dir + help |
| 文档 | 以上 |

## 4. 发布门槛

- [x] files_dir 统一。
- [x] home_dir/dunce。
- [x] Windows 文件名规则单测。
- [x] 帮助实际路径。
- [x] 0.1.3 98 个测试回归；当前 103。
- [x] fmt/clippy/test/release。
- [ ] Windows 真实机器。
- [ ] macOS 真实机器。
- [ ] 长路径/UNC。

## 5. 后续

- `--data-dir` / `--state-dir` 完全隔离（如果用户需要）。
- 旧路径迁移辅助（不默认迁移）。
- 可选 `sanitize-filename` 对照测试。
- Windows 长路径 manifest/`\\?\` 策略。
