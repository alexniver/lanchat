# 测试与验收

> 状态：已实现，Linux 已验收；Windows/macOS 待真实机器验证
> 关联：[01-paths.md](01-paths.md) · [02-filenames.md](02-filenames.md)

## 1. 当前验证环境

- Linux（当前会话环境）
- `cargo test --offline`
- `cargo clippy --offline --all-targets -- -D warnings`
- `cargo fmt --check`
- `cargo build --offline --release`

未在真实 Windows/macOS 上运行 TUI/文件系统测试。

## 2. 单元测试

### 路径

- `Paths` 包含 `files_dir`。
- `files_dir = data_dir/files`。
- 测试构造的 `Paths` 已包含 `files_dir`。
- `~` 展开使用 `dirs::home_dir()`：
  - `~` → home
  - `~/` → home
  - `~\`（Windows 分隔符写法）→ home
- `dunce::canonicalize` 用于源路径。

### 文件名

- Windows 非法字符替换为 `_`。
- 结尾点/空格去除。
- `... ` → `file`。
- 路径分隔符拒绝。
- Windows 保留名加 `_`。
- 长度截断保留扩展名。
- 中文名保留。
- 重名 `[0]`。

### UI

- 帮助显示实际 `files_dir`。
- 版本号 0.1.4。
- 帮助状态栏仍为“按 Esc 退出帮助 …”。

## 3. 平台测试矩阵

| 项目 | Linux | macOS | Windows |
| --- | --- | --- | --- |
| 配置目录 | ✅ | 🟡 待测 | 🟡 待测 |
| 历史目录 | ✅ | 🟡 | 🟡 |
| files 目录 | ✅ | 🟡 | 🟡 |
| 日志目录 | ✅ | 🟡 | 🟡 |
| `~` 展开 | ✅ | 🟡 | 🟡 |
| Windows 保留名 | ✅ 单测 | ✅ 单测 | 🟡 真实 FS |
| 非法字符 | ✅ 单测 | ✅ 单测 | 🟡 真实 FS |
| 大小写重名 | 🟡（Linux 区分大小写） | 🟡 | 🟡 |
| 长路径 >260 | ❌ 未测 | ❌ | ❌ |
| ACL/权限 | POSIX 0600/0700 | POSIX | 继承 ACL |

## 4. 验收

- [x] `Paths.files_dir` 统一接收目录。
- [x] `main` 不再临时拼 `data_dir/files`。
- [x] `~` 使用 `dirs::home_dir()`。
- [x] 支持 `~/` 和 `~\`。
- [x] 源路径 canonicalize 使用 `dunce`。
- [x] 文件名清理覆盖 Windows 非法字符/保留名/截断。
- [x] 帮助显示实际 files 路径。
- [x] README 平台路径表。
- [x] 0.1.3 的 98 个测试继续通过；当前总计 103。
- [ ] Windows 真实机器：配置/历史/文件路径、保留名、大小写。
- [ ] macOS 真实机器：Application Support、大小写不敏感。
- [ ] 长路径/UNC 专项。

## 5. 手工验收命令

```bash
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
cargo fmt --check
cargo build --offline --release
./target/release/lanchat --version
```

期望：

```text
lanchat 0.1.4
```

接收文件目录（Linux）：

```bash
ls -ld ~/.local/share/lanchat/files
```

Windows 预期：

```text
%LOCALAPPDATA%\lanchat\files
```

macOS 预期：

```text
~/Library/Application Support/lanchat/files
```
