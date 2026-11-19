# 0.1.10 已确认决策

## D-065 使用 GitHub Actions 构建 release

- **状态**：已确认
- **决策**：使用 GitHub-hosted runners 原生构建 Linux/Windows/macOS 产物。
- **理由**：公开仓库免费额度足够；避免本地跨平台工具链。

## D-066 首版手写 matrix

- **状态**：已确认
- **决策**：先写 `release.yml` matrix，不直接上 cargo-dist。
- **理由**：透明、易调试；后续需要安装器再评估 cargo-dist。

## D-067 目标平台

- **状态**：已确认
- **决策**：Linux x86_64、Windows x86_64、macOS Intel + ARM；Linux arm64 可选。
- **理由**：覆盖主流桌面；macOS 双架构必要。
- **影响**：产物数量/CI 时间。

## D-068 draft release

- **状态**：已确认
- **决策**：tag 构建后创建 draft，人工 smoke test 后发布。
- **理由**：跨平台 TUI/网络/文件行为需要真实平台验证。

## D-069 首版不签名

- **状态**：已确认
- **决策**：macOS/Windows 首版不签名，文档说明 Gatekeeper/SmartScreen。
- **理由**：避免证书依赖；后续再加。

## D-070 补 LICENSE

- **状态**：已确认
- **决策**：发布前补 MIT LICENSE 文件并打进压缩包。
- **理由**：Cargo 已声明 MIT；开源发布应包含许可证文本。

## D-071 版本/CHANGELOG

- **状态**：已确认
- **决策**：手工 bump Cargo.toml/CHANGELOG 后打 tag；后续可上 release-plz。
- **理由**：首版简单可控。
