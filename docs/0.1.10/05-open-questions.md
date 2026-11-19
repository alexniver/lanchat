# 0.1.10 待确认

## P0

1. **目标平台**
   - 首版是否包含 Linux arm64？
   - macOS 是否同时发 Intel + Apple Silicon？
   - 是否要 musl 静态 Linux？
2. **方案**
   - 手写 GitHub Actions matrix（推荐首版）；
   - 还是直接采用 `cargo-dist`？
3. **发布方式**
   - draft release + 人工发布（推荐）；
   - 还是 tag 自动公开？
4. **代码签名**
   - 首版是否接受未签名 macOS/Windows 产物？
   - 是否有 Apple Developer / Windows 代码签名证书？
5. **LICENSE**
   - 当前 Cargo 声明 MIT 但没有 LICENSE 文件；发布前是否补？
6. **版本流程**
   - 手工 bump/tag，还是用 cargo-release/release-plz？

## P1

- 是否生成安装脚本（shell/PowerShell）。
- 是否提供 Homebrew/Scoop。
- 是否做 universal macOS 包。
- Release notes 从 CHANGELOG 提取还是自动生成。
- 是否在 release workflow 里跑完整测试还是仅构建。
- Runner 标签选择（macOS Intel/ARM 当前可用标签）。
