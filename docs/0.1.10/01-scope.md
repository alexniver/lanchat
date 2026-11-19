# 0.1.10 范围与验收

## 目标

- 用 GitHub Actions 从 tag 自动构建 Linux/Windows/macOS 二进制。
- 产物可直接下载解压运行，不需要用户装 Rust。
- 每个平台提供校验和。
- 构建失败/测试失败时不创建 release。
- 发布流程可重复、可审计。

## 非目标

| 不做 | 说明 |
| --- | --- |
| 应用商店/包管理器 | 首版只发 GitHub Release 压缩包 |
| 自动发布正式版 | 先 draft release，人工 smoke test |
| 代码签名/公证（首版） | 可选后续；macOS/Windows 未签名会有提示 |
| 自动版本号/CHANGELOG | 可用 cargo-release/release-plz，后续 |
| Linux 全发行版 | 先覆盖主流 glibc；musl 可选 |
| 自动更新器 | 不做 |
| 安装器 | 首版 tar.gz/zip；安装脚本后续 |

## 功能需求

### FR-01 触发

- tag `v*`（如 `v0.1.10`）触发 release。
- 支持 `workflow_dispatch` 手动重跑。
- PR/push 的 CI 与 release 分离。
- 校验 tag 与 `Cargo.toml` 版本一致，不一致失败。

### FR-02 构建矩阵

- `ubuntu-22.04`：Linux x86_64 gnu。
- `windows-2022`：Windows x86_64 msvc。
- macOS Intel + Apple Silicon 分开构建。
- 可选 Linux arm64 / musl。
- 每个 job 只构建本平台原生 target，避免跨平台 TUI 依赖问题。

### FR-03 测试

- 构建前/后运行 `cargo test --locked`。
- 至少 Linux 跑 fmt/clippy/test。
- 各平台至少 `cargo test`；TUI 使用 TestBackend，无需真实终端。
- 集成测试使用 localhost，应该可在 runner 上运行。

### FR-04 打包

- Unix：`tar.gz`，保留可执行权限。
- Windows：`.zip`。
- 文件名含版本、target。
- 包含二进制、README、CHANGELOG、LICENSE。
- 生成 `SHA256SUMS`。

### FR-05 发布

- 使用 draft GitHub Release。
- 上传所有产物和 SHA256SUMS。
- 发布说明来自 CHANGELOG 对应版本（或 generate_release_notes）。
- 不把 GitHub Actions 的临时 artifact 当最终下载入口（90 天/需登录）。

### FR-06 安全

- `permissions: contents: write` 只给 release job。
- 固定 action 到可信 major/SHA。
- 不在日志打印 secrets。
- 构建来自 tag/受保护分支。

## 验收

- [ ] push `v0.1.10` 生成 5 个平台产物（含可选 arm64）。
- [ ] 每个产物解压后二进制 `--version` 正确。
- [ ] Linux/macOS 二进制可执行，Windows `.exe` 可运行。
- [ ] SHA256SUMS 与产物一致。
- [ ] draft release 包含说明和全部资产。
- [ ] tag 版本不匹配时 workflow 失败。
- [ ] 测试失败时不产生 release。
