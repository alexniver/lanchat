# 其他开源项目怎么做

## 1. 手写 GitHub Actions matrix

最常见、最透明：

- 一个 `release.yml`，matrix 覆盖 OS/target；
- 每个平台原生构建；
- 打包 tar.gz/zip；
- 上传 draft release；
- 校验和。

优点：完全可控、容易调试、无额外工具。
缺点：workflow 要自己维护；安装器/包管理器要另做。

## 2. `cargo-dist`

Rust 项目常用的一体化方案：

```bash
cargo install cargo-dist
cargo dist init
```

生成：

- `.github/workflows/release.yml`；
- 各平台归档；
- SHA256；
- shell/PowerShell 安装器（可选）；
- Homebrew formula（可选）。

优点：少写 YAML、Rust 生态惯例、产物规范。
缺点：生成内容较多；定制签名/特殊打包需要学习其配置。

适合：希望尽快有“下载即安装”的体验，或未来做 Homebrew/Scoop。

## 3. `cargo-release` / `release-plz`

- `cargo-release`：本地 bump 版本、打 tag、发布 crates.io（本项目 `publish=false`）。
- `release-plz`：PR 自动更新版本/CHANGELOG，合并后发布。

适合后续自动化版本管理，不是首版必需。

## 4. 其他常见工具

| 工具 | 用途 |
| --- | --- |
| `cross` | 跨平台/跨架构构建 |
| `cargo-zigbuild` | 用 Zig 链接较老 glibc |
| `upload-rust-binary-action` | 自动打包 Rust 二进制 |
| `softprops/action-gh-release` | 创建/上传 GitHub Release |
| `gh release` | GitHub CLI 发布 |

## 5. 典型项目模式

- 小型 Rust CLI：手写 matrix 或 cargo-dist；
- 较大项目：CI（fmt/clippy/test）与 release（tag）分离；
- 需要签名的 macOS/Windows 应用：release workflow 加签名/公证步骤；
- 需要多架构：native ARM runner 优先，cross 兜底。

## 6. 对本项目的建议

首版：

- 手写 `release.yml`；
- Linux x86_64、Windows x86_64、macOS Intel + ARM；
- draft release + SHA256；
- 不做签名/安装器。

后续：

- 需要一键安装 → 评估 cargo-dist；
- 需要 Homebrew/Scoop → cargo-dist 或单独 tap；
- 需要版本自动化 → release-plz。
