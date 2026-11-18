# 0.1.6 路线图

> 状态：已实现，验收中

## 已完成

- Identity.short_no + 持久化。
- Hello.short_no + capability。
- 冲突顺延 + ShortNoUpdate。
- no/ip/node_id/IP/昵称寻址。
- 显示名/历史/成员/文件提示。
- 测试 + 版本 0.1.6。

## 发布门槛

- [x] no1 显示。
- [x] send/recv 支持 no1、ip101。
- [x] 裸 103 不再匹配。
- [x] 短号冲突顺延。
- [x] 0.1.5 111 个测试回归；当前 111。
- [x] fmt/clippy/test/release。
- [x] 双实例 no1/no2 手工验证。
- [ ] Windows/macOS 真机。
- [ ] 三节点冲突 soak。

## 后续

- 短号显示完整 node_id 开关。
- 手工指定短号（需冲突检查）。
- 短号保留期/回收策略。
- 旧客户端兼容说明。
