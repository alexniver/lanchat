# 0.1.4 架构改动

> 状态：已实现
> 关联：[01-paths.md](01-paths.md) · [02-filenames.md](02-filenames.md) · [04-crates.md](04-crates.md)

## 1. Paths

[src/config.rs](../../src/config.rs)：

```rust
pub struct Paths {
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
    pub keymap_file: PathBuf,
    pub identity_file: PathBuf,
    pub data_dir: PathBuf,
    pub files_dir: PathBuf,   // 新增
    pub history_dir: PathBuf,
    pub state_dir: PathBuf,
    pub log_dir: PathBuf,
    pub panic_log: PathBuf,
}
```

`Paths::discover`：

```rust
let data_dir = dirs::data_local_dir()?.join("lanchat");
let files_dir = data_dir.join("files");
```

`ensure_dirs` 创建 `files_dir`。

[src/main.rs](../../src/main.rs)：

```rust
net::start(config.network.clone(), config.identity.clone(), config.paths.files_dir.clone())
```

不再由 `main` 拼接 `data_dir/files`。

## 2. 帮助页

[src/app/help.rs](../../src/app/help.rs) 的 `lines` 改为：

```rust
pub fn lines(keymap: &Keymap, files_dir: &Path) -> Vec<String>
```

帮助中的“保存到 …”显示实际路径：

```rust
format!("  - 保存到 {}", files_dir.display())
```

- 不再硬编码 `~/.local/share/lanchat/files`。
- Windows 显示 `C:\Users\...\AppData\Local\lanchat\files`。
- macOS 显示 `~/Library/Application Support/lanchat/files`。

## 3. `~` 展开

[src/app/command.rs](../../src/app/command.rs)：

```rust
let expanded = if raw == "~" {
    dirs::home_dir()?
} else if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\")) {
    dirs::home_dir()?.join(rest)
} else {
    PathBuf::from(raw)
};
let absolute = if expanded.is_absolute() { ... };
let canonical = dunce::canonicalize(&absolute)?;
```

- 去掉 `$HOME` 依赖。
- 支持 `~/`、`~\`。
- `dunce` 避免 Windows `\\?\` 前缀进入错误信息/帮助。

## 4. 文件名 sanitizer

[src/net/file.rs](../../src/net/file.rs)：

- `sanitize_name` 替换 Windows 非法字符。
- `is_windows_reserved` 处理设备名。
- `truncate_name` 保留扩展名。
- `unique_path` 用 sanitize 后的名字 + `try_exists`。
- `part_path` 用 sanitize 后的名字 + transfer_id。

调用点：

- 接收端 `receive_files`：`unique_path`。
- 发送端 `validate_file_offer`：只做协议级校验。
- `FileOffer` 的 basename 来自发送方；接收端最终 sanitize。

## 5. 依赖

- `dirs`：已有，保留。
- `dunce`：新增，用于 canonicalize。
- 不新增 `sanitize-filename`；内置策略。
- 不新增 `shellexpand`；`dirs::home_dir`。

## 6. 兼容性

- Linux 路径保持不变。
- 已有配置/历史/文件不需要迁移。
- `--config` 仍只改配置，不改 data/state。
- 旧版本发送的合法文件名继续可收；非法字符会被安全替换。
- 0.1.3 的 98 个测试继续通过。

## 7. 测试

- `net::file::tests::sanitizes_windows_invalid_characters_and_trailing_dots`
- `net::file::tests::avoids_windows_reserved_device_names`
- `net::file::tests::truncates_long_names_preserving_extension`
- `app::command::tests::expands_tilde_with_home_dir`
- `ui::tests::renders_help_panel_and_status`（版本号更新）
- 全量回归。

## 8. 待做

- Windows/macOS 真实运行。
- 长路径/UNC/ACL。
- 可选 `sanitize-filename` 对照。
- 可选 `--data-dir`/`--state-dir` 完全隔离。
