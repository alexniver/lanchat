# 0.1.4 已确认决策

> 记录本次跨平台路径讨论的结论；未完成项见 [05-testing.md](05-testing.md)。

## D-027 保留 `dirs`，不迁移到 `directories`

- **状态**：已确认（2026-11-19）
- **决策**：继续使用 `dirs` 的 `config_dir`/`data_local_dir`/`state_dir`/`home_dir`。
- **理由**：现有路径稳定，Linux 用户已有数据；切换会带来迁移成本。
- **影响**：不改变 `~/.config/lanchat`、`~/.local/share/lanchat`。

## D-028 接收文件统一使用 `Paths.files_dir`

- **状态**：已确认
- **决策**：`files_dir = data_local_dir()/lanchat/files`，由 `Paths` 统一提供；`main` 不再临时拼接。
- **影响**：帮助、网络管理器、测试共用同一路径。

## D-029 `~` 使用 `dirs::home_dir()`

- **状态**：已确认
- **决策**：`/send file` 的 `~` 展开不再依赖 `$HOME`；支持 `~/` 和 `~\`。
- **理由**：Windows 通常只有 `USERPROFILE`。
- **影响**：Windows 上 `/send file ~/...` 可正常工作。

## D-030 使用 `dunce::canonicalize`

- **状态**：已确认
- **决策**：源文件路径规范化使用 `dunce`，避免 Windows `\\?\` 前缀。
- **影响**：错误信息/帮助中的路径更自然；Unix 行为不变。

## D-031 内置跨平台文件名 sanitizer

- **状态**：已确认
- **决策**：不直接引入 `sanitize-filename`；内置 `sanitize_name`：
  - 所有平台统一替换 Windows 非法字符；
  - 拒绝路径分隔符/`..`；
  - 去掉结尾点/空格；
  - 保留设备名加 `_`；
  - 255 字节截断并保留扩展名。
- **理由**：行为可控、保留中文、便于测试。
- **影响**：Linux 上 `a:b.png` 会保存为 `a_b.png`，这是可移植性取舍。

## D-032 帮助显示实际 files 路径

- **状态**：已确认
- **决策**：`help::lines` 接收 `files_dir`，不再写死 `~/.local/share/lanchat/files`。
- **影响**：Windows/macOS 帮助显示对应平台路径。

## D-033 版本 0.1.4

- **状态**：已确认
- **决策**：本次改动作为 0.1.4；保持 0.1.3 的聊天/文件/帮助行为。
- **影响**：Cargo、CLI、README、帮助版本号同步更新。

---

## 待完成

- Windows 真实机器验证。
- macOS 真实机器验证。
- 长路径/UNC 专项。
- 可选：`--data-dir`/`--state-dir`。
- 可选：`sanitize-filename` 对照测试。
