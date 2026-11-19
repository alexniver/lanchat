# lanchat 0.1.10 Release 构建需求

> 状态：**workflow 已添加，待首次 GitHub 运行**（0.1.9 已冻结）
> 版本：0.1.10
> 创建日期：2026-11-20

## 一句话回答

可以。GitHub Actions 完全可以在 **Linux、Windows、macOS** 的原生 runner 上分别构建 `lanchat`，并把二进制压缩包 + 校验和上传到 GitHub Release。

这是 Rust CLI 项目最常见的做法；也可以直接用 `cargo-dist` 生成整套 release workflow。

## 推荐产物

| 平台 | 目标三元组 | 产物 |
| --- | --- | --- |
| Linux x86_64 | `x86_64-unknown-linux-gnu` | `lanchat-v0.1.10-x86_64-unknown-linux-gnu.tar.gz` |
| Linux arm64（可选） | `aarch64-unknown-linux-gnu` | `lanchat-v0.1.10-aarch64-unknown-linux-gnu.tar.gz` |
| Windows x86_64 | `x86_64-pc-windows-msvc` | `lanchat-v0.1.10-x86_64-pc-windows-msvc.zip` |
| macOS Intel | `x86_64-apple-darwin` | `lanchat-v0.1.10-x86_64-apple-darwin.tar.gz` |
| macOS Apple Silicon | `aarch64-apple-darwin` | `lanchat-v0.1.10-aarch64-apple-darwin.tar.gz` |
| 所有平台 | — | `SHA256SUMS` |

每个压缩包建议包含：

```text
lanchat 或 lanchat.exe
README.md
CHANGELOG.md
LICENSE
```

当前仓库还没有 `LICENSE` 文件；Cargo 声明 MIT，发布前建议补上。

## 推荐流程

```text
1. 本地改版本/CHANGELOG
2. git tag v0.1.10
3. git push origin v0.1.10
4. GitHub Actions 矩阵构建 + 测试 + 打包
5. 自动创建 draft Release，上传所有产物和 SHA256SUMS
6. 人工在 Windows/macOS/Linux 上试运行
7. 手动发布 Release
```

推荐 **draft release**，不要直接公开。CLI 跨平台最容易出问题的是动态链接/运行库/终端行为，人工 smoke test 很重要。

## 两种方案

| 方案 | 适合 |
| --- | --- |
| A. 手写 GitHub Actions matrix | 想完全控制产物、命名、打包、签名；首版推荐 |
| B. `cargo-dist` | 想自动生成 release workflow、归档、校验和、安装脚本；Rust 项目常用 |

建议首版先手写 matrix（透明、好调试），后续需要 shell/PowerShell 安装器或 Homebrew 时再评估 `cargo-dist`。

## 文档

- [01-scope.md](01-scope.md)
- [02-actions.md](02-actions.md)
- [03-platforms.md](03-platforms.md)
- [04-oss-practices.md](04-oss-practices.md)
- [05-open-questions.md](05-open-questions.md)
- [06-roadmap.md](06-roadmap.md)
- [decisions.md](decisions.md)
