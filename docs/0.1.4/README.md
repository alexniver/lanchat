# lanchat 0.1.4 跨平台路径与文件名适配

> 状态：**0.1.4 已实现，验收中**（0.1.3 已冻结）
> 版本：0.1.4
> 建立日期：2026-11-19

## 目标

让配置、历史、日志和接收文件在 Linux/macOS/Windows 上落到符合平台习惯的位置，并保证远端文件名不会因为 Windows 规则导致接收失败或写到意外位置。

0.1.4 解决四类问题：

1. **路径来源统一**：所有路径由 `Paths` 提供，接收文件不再在 `main` 临时拼接。
2. **`~` 展开跨平台**：从 `$HOME` 改为 `dirs::home_dir()`，并支持 `~/` 和 `~\`。
3. **文件名跨平台安全**：处理 Windows 非法字符、保留设备名、结尾点/空格、长度截断。
4. **Windows 路径显示**：用 `dunce::canonicalize` 避免 `\\?\` 前缀。

## 目标路径

| 内容 | Linux | macOS | Windows |
| --- | --- | --- | --- |
| 配置 | `~/.config/lanchat` | `~/Library/Application Support/lanchat` | `%APPDATA%\lanchat` |
| 身份/键位 | 配置目录下 | 配置目录下 | 配置目录下 |
| 历史 | `~/.local/share/lanchat/history` | `~/Library/Application Support/lanchat/history` | `%LOCALAPPDATA%\lanchat\history` |
| 接收文件 | `~/.local/share/lanchat/files` | `~/Library/Application Support/lanchat/files` | `%LOCALAPPDATA%\lanchat\files` |
| 日志 | `~/.local/state/lanchat/logs` | 回退到 data 目录的 `lanchat/logs` | `%LOCALAPPDATA%\lanchat\logs` |
| panic 日志 | 日志目录下 | 日志目录下 | 日志目录下 |

- 配置使用 `dirs::config_dir()`。
- 历史/接收文件使用 `dirs::data_local_dir()`。
- 日志优先 `dirs::state_dir()`；没有时回退到 `data_local_dir()`。
- 接收文件目录是 `Paths.files_dir`，由 `data_dir/files` 组成。
- 不迁移已有 Linux 路径；当前路径保持不变。

## 关键决策

- **保留 `dirs`**，不切换到 `directories`，避免配置/历史路径迁移。
- **新增 `dunce`**，只用于 `canonicalize` 的 Windows 显示处理。
- **不新增 `sanitize-filename`**：内置跨平台清理函数，行为可控、保留中文、便于测试。
- **不新增 `shellexpand`**：`dirs::home_dir()` 足够。
- **文件名采用“可移植安全”策略**：在所有平台都替换 Windows 非法字符，保证收到的文件以后移到 Windows 也不会出问题。

## 范围

| 能力 | 0.1.4 | 说明 |
| --- | --- | --- |
| `Paths.files_dir` | ✅ 做 | 统一接收文件目录 |
| `dirs::home_dir()` | ✅ 做 | 替代 `$HOME` |
| `~/` 和 `~\` | ✅ 做 | 跨平台 `~` 展开 |
| `dunce::canonicalize` | ✅ 做 | Windows 路径显示 |
| Windows 非法字符清理 | ✅ 做 | `< > : " \| ? *` 等 |
| Windows 保留名 | ✅ 做 | `CON`、`NUL`、`COM1` 等 |
| 结尾点/空格处理 | ✅ 做 | 避免 Windows 静默改名 |
| 长度截断保留扩展名 | ✅ 做 | 255 字节 |
| 帮助显示实际 files 路径 | ✅ 做 | 不再写死 XDG 路径 |
| 平台路径文档 | ✅ 做 | README/0.1.4 |
| 真实 Windows/macOS 运行测试 | 🟡 待做 | 当前只在 Linux 环境验证 |
| 配置/历史自动迁移 | ❌ 不做 | 保持现有路径 |
| Windows ACL/长路径专项 | ❌ 不做 | 系统默认行为；后续可加 |

## 文档索引

| 文档 | 内容 |
| --- | --- |
| [01-paths.md](01-paths.md) | 路径来源、平台表、`--config` 语义、权限 |
| [02-filenames.md](02-filenames.md) | 跨平台文件名 sanitizer、保留名、截断、重名 |
| [03-architecture.md](03-architecture.md) | `Paths.files_dir`、`dunce`、帮助动态路径 |
| [04-crates.md](04-crates.md) | `dirs`/`directories`/`dunce`/`sanitize-filename` 选择 |
| [05-testing.md](05-testing.md) | 单元测试、平台测试矩阵、验收 |
| [06-roadmap.md](06-roadmap.md) | 实现顺序与发布门槛 |
| [decisions.md](decisions.md) | 已确认决策 |

## 与 0.1.3 的关系

- 聊天、文件传递、帮助页面、成员侧边栏行为不变。
- 帮助页面的“保存到 …”改为显示实际 `files_dir`。
- 版本号更新为 0.1.4。
- 0.1.3 的 98 个测试继续通过。
