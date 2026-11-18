# crate 选择

> 状态：已确认
> 关联：[01-paths.md](01-paths.md) · [03-architecture.md](03-architecture.md)

## 1. 最终依赖

| crate | 状态 | 用途 |
| --- | --- | --- |
| `dirs` | 保留 | config/data/state/home 目录 |
| `dunce` | 新增 | Windows `canonicalize`，避免 `\\?\` 前缀 |
| `sanitize-filename` | 未引入 | 用内置 `sanitize_name` 替代 |
| `directories` | 未引入 | 不改变现有路径，避免迁移 |
| `shellexpand` | 未引入 | `dirs::home_dir()` 足够 |
| `path-slash` | 未引入 | 内部用 `PathBuf`，显示用 `Path::display()` |
| `path-absolutize` | 未引入 | `canonicalize` 足够 |

## 2. 为什么保留 `dirs`

- 已经是项目依赖，路径稳定。
- Linux 用户已有配置/历史在 `~/.config/lanchat`、`~/.local/share/lanchat`。
- `dirs` 直接提供 `config_dir` / `data_local_dir` / `state_dir` / `home_dir`。
- 不需要为了“更高级”的 API 承担路径迁移。

`directories::ProjectDirs` 的优点：

- 统一 `config_dir`/`data_dir`/`data_local_dir`/`cache_dir`；
- 应用名/组织名集中配置；
- 适合从零开始的项目。

但对当前项目：

- 路径可能变化；
- 需要旧路径兼容读取；
- 0.1.4 的目标是适配，不是迁移。

因此保留 `dirs`。

## 3. 为什么新增 `dunce`

Windows `std::fs::canonicalize` 可能返回：

```text
\\?\C:\Users\alice\...
```

这个前缀：

- 不适合显示在消息/帮助里；
- 用户输入路径与显示路径不一致；
- 某些旧程序不识别。

`dunce::canonicalize` 在 Windows 上返回更自然的路径，在 Unix 上等价于 `std::fs::canonicalize`。只用于源文件路径规范化，不用于接收文件目录拼接。

## 4. 为什么没有直接引入 `sanitize-filename`

`sanitize-filename` 能处理 Windows 非法字符和保留名，但当前需求还有：

- 保留中文/空格；
- 保留扩展名截断；
- 与 `[0]`/`[1]` 重名策略配合；
- 明确“所有平台都按可移植规则”；
- 错误信息/验收可测。

内置 `sanitize_name` 约 80 行，规则完全可控，避免第三方库对中文/点的处理差异。后续若发现边界遗漏，可参考 `sanitize-filename` 的测试补充。

## 5. 为什么没有引入 `shellexpand`

`/send file ~/...` 只需要 home 展开，不需要：

- 环境变量展开；
- 波浪号用户名（`~alice`）；
- shell 引号解析。

`dirs::home_dir()` + 手动 `~/`/`~\` 更小、更可预测。路径中的空格通过引号或 `file ... to ...` 解析处理。

## 6. 依赖风险

- `dirs`：成熟、轻量、跨平台。
- `dunce`：小型、单一职责。
- 不引入大依赖树。
- Cargo.lock 保持可离线构建。

## 7. 未来可选项

| 需求 | 可选 crate |
| --- | --- |
| 更完整文件名清理 | `sanitize-filename` |
| 应用目录统一定义 | `directories` |
| `~`/环境变量展开 | `shellexpand` |
| 路径显示统一 `/` | `path-slash` |
| 路径绝对化 | `path-absolutize` |

这些都不是 0.1.4 必需。
