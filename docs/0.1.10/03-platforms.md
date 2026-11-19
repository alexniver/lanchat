# 平台构建细节

## Linux

- 推荐 `ubuntu-22.04` 构建 gnu 版，兼容较老 glibc。
- `ubuntu-latest` 产物可能要求较新 glibc，不适合所有用户。
- 可选 musl 静态版（`x86_64-unknown-linux-musl`），但需装 musl 工具链或 `cross`。
- 可选 arm64：`ubuntu-24.04-arm`（若仓库可用）或 cross/QEMU。
- 打包：`tar.gz`，保留可执行位。

## Windows

- 推荐 `windows-2022` + `x86_64-pc-windows-msvc`。
- 需要 MSVC 工具链；GitHub runner 已带。
- 打包：`.zip`，内含 `lanchat.exe`。
- 未签名 exe 可能触发 SmartScreen；首版可接受，后续可买代码签名证书。
- 终端：Windows Terminal / PowerShell；crossterm 已支持。

## macOS

- Intel：`x86_64-apple-darwin`，用 Intel runner（如 `macos-15-intel`，以当前 GitHub 标签为准）。
- Apple Silicon：`aarch64-apple-darwin`，用 ARM runner（`macos-14`/`macos-15`）。
- 可以分别发两个包，也可以 `lipo` 合成 universal 包；首版建议分开，简单可验证。
- 未签名/未公证的下载二进制可能被 Gatekeeper 拦截；用户可 `xattr -d com.apple.quarantine`，但体验一般。
- 正式分发可考虑 Apple Developer 签名+公证；需要账号/证书，0.1.10 不强制。
- 打包：`tar.gz`。

## 运行库/依赖

- 本项目纯 Rust 依赖为主：tokio、ratatui、crossterm、mdns-sd、arboard、zip。
- Linux `arboard` 的 clipboard 可能依赖 X11/Wayland 运行环境；构建不需要显示。
- macOS/Windows 原生构建可避免交叉编译链接问题。
- 不需要用户安装 Rust。

## 校验和

- 每个平台生成 SHA256。
- 汇总为 `SHA256SUMS` 上传。
- 发布说明里写校验命令。

## 版本显示

- `lanchat --version` 应显示 `lanchat 0.1.10`。
- tag、Cargo.toml、CHANGELOG 三处版本一致。
