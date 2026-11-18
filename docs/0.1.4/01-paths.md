# 跨平台路径

> 状态：已实现
> 关联：[README.md](README.md) · [02-filenames.md](02-filenames.md) · [03-architecture.md](03-architecture.md)

## 1. 当前实现

[src/config.rs](../../src/config.rs) 的 `Paths::discover` 使用：

```rust
dirs::config_dir()      // 配置、identity、keymap
dirs::data_local_dir()  // data、history、files
dirs::state_dir()       // logs；没有时回退 data_local
```

0.1.4 新增：

```rust
pub files_dir: PathBuf, // data_dir/files
```

`Paths::ensure_dirs` 会创建 `files_dir`；`main` 把 `config.paths.files_dir` 传给网络管理器，接收文件不再在 `main` 里临时拼接。

## 2. 平台路径

| 内容 | Linux | macOS | Windows |
| --- | --- | --- | --- |
| 配置 | `~/.config/lanchat` | `~/Library/Application Support/lanchat` | `%APPDATA%\lanchat` |
| identity | `<config>/identity.json` | 同左 | 同左 |
| keymap | `<config>/keymap.toml` | 同左 | 同左 |
| 历史 | `~/.local/share/lanchat/history` | `~/Library/Application Support/lanchat/history` | `%LOCALAPPDATA%\lanchat\history` |
| 文件 | `~/.local/share/lanchat/files` | `~/Library/Application Support/lanchat/files` | `%LOCALAPPDATA%\lanchat\files` |
| 日志 | `~/.local/state/lanchat/logs` | `<data>/lanchat/logs`（state 回退） | `%LOCALAPPDATA%\lanchat\logs` |
| panic | `<logs>/panic.log` | 同左 | 同左 |

说明：

- `dirs` 在 macOS 的 `config_dir` 与 `data_local_dir` 都指向 `~/Library/Application Support`；配置与数据在同一应用目录下是正常行为。
- Windows `config_dir` 是 Roaming，`data_local_dir` 是 Local；配置跟随用户漫游，历史和接收文件留在本机。
- `state_dir` 在 macOS/Windows 通常为 `None`，日志回退到 Local/Application Support。
- Linux 遵循 `XDG_CONFIG_HOME`、`XDG_DATA_HOME`、`XDG_STATE_HOME`。

## 3. `--config` 语义

`--config <PATH>` 只改变：

- `config_dir`
- `config_file`
- `identity_file`
- 默认 `keymap_file`（除非同时 `--keymap`）

**不改变**：

- 历史目录
- 接收文件目录
- 日志目录

这是有意行为：配置文件可以放在任意位置，但用户数据仍跟随平台目录，避免“换个配置就找不到历史”。

## 4. 为什么保留 `dirs`

当前路径已经稳定，Linux 用户已在这些目录有配置/历史。切换到 `directories::ProjectDirs` 可能改变路径，需要迁移逻辑。

`dirs` 足够：

- 提供 config/data/state/home；
- 跨平台；
- 轻量；
- 现有代码已经依赖。

如果未来需要“应用级目录 + 缓存”统一接口，再评估 `directories`，并加兼容读取旧路径。

## 5. `~` 展开

0.1.4 把 [src/app/command.rs](../../src/app/command.rs) 的 `$HOME` 改为：

```rust
dirs::home_dir()
```

支持：

```text
/send file ~/Download/a.png to 103
/send file ~\Download\a.png to 103
```

- Windows 通常只有 `USERPROFILE`，没有 `HOME`；`dirs::home_dir()` 能正确处理。
- 相对路径仍相对当前工作目录。
- 最后用 `dunce::canonicalize` 规范化；Windows 不显示 `\\?\` 前缀。

## 6. 权限

- Unix：`files_dir` 使用 `0700`，接收文件 `0600`；沿用 0.1.2 行为。
- Windows：没有 POSIX mode；文件继承目录 ACL，代码中的 `set_mode` 是 no-op。
- macOS：POSIX mode 生效。

## 7. 错误信息

0.1.4 去掉了错误信息里的“XDG_CONFIG_HOME/XDG_DATA_HOME”字样，改为：

- `无法确定配置目录`
- `无法确定数据目录`

避免在 Windows/macOS 上给出错误的 XDG 提示。

## 8. 验收

- [x] `Paths` 有 `files_dir`，`main` 使用它。
- [x] `~` 使用 `dirs::home_dir()`。
- [x] `~/` 和 `~\` 都能展开。
- [x] 错误信息不再只提 XDG。
- [x] 帮助显示实际 files 路径。
- [x] Linux 路径保持不变。
- [ ] Windows/macOS 真实机器验证（待做）。
