# 0.1.10 路线图

> 状态：讨论中

## M0 需求/决策

- 目标矩阵、方案、签名、发布方式。
- 补 LICENSE（如需要）。

## M1 CI

- `.github/workflows/ci.yml`：
  - fmt、clippy、test、build（Linux 为主）。
- PR/push 触发。

## M2 Release workflow

- `.github/workflows/release.yml`。
- tag/版本校验。
- matrix 构建、测试、打包、SHA256。
- draft release 上传。

## M3 验证

- 在真实 Windows/macOS 下载产物 smoke test。
- 检查 `--version`、TUI 启动、mDNS/文件传递基本路径。
- 修正 runner/打包问题。

## M4 发布

- 打 `v0.1.10` tag。
- 发布 draft release。
- 更新 README 下载说明。

## 验收

- [ ] tag 触发。
- [ ] 5 个平台产物。
- [ ] SHA256SUMS。
- [ ] draft release。
- [ ] 真实平台 smoke test。
