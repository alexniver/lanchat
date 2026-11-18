# 0.1.7 路线图

> 状态：已实现，验收中

## 已完成

- `/setname` 解析/校验。
- identity.json + config.toml 持久化。
- `NameUpdate` + capability。
- 状态栏本机名（List/Compose/Help）。
- 测试 + 版本 0.1.7。

## 发布门槛

- [x] `/setname alice` 显示 `alice (no1)`。
- [x] 状态栏 `本机 alice (no1)`。
- [x] 重启保持。
- [x] config 注释保留。
- [x] 0.1.6 111 个测试回归；当前 116。
- [x] fmt/clippy/test/release。
- [x] 手工 `/setname` 验证。
- [ ] Windows/macOS 真机。

## 风险

- 旧 peer 不实时改名，重连后生效。
- 名字重名仍靠 short_no 区分。
- config.toml 在极端格式下 toml_edit 可能重排，但保留内容。
