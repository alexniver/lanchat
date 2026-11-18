# 架构

> 状态：已实现

## 命令

`ParsedCommand::SetName { name }`。

`App::handle_set_name`：

1. trim/校验；
2. `config::set_identity_name(config_file, name)`；
3. 更新 `config.identity.name`；
4. `config::save_identity(identity_file, identity)`；
5. pop Compose；
6. 网络启用时发送 `Effect::SetName`；
7. status `名称已修改为 ...`。

## 配置持久化

`config::set_identity_name` 使用 `toml_edit`：

- 文件不存在 → no-op；
- 文件存在 → 解析、设置 `[identity].name`、写回；
- 保留注释/其他段。

## 网络

`NetCommand::SetName { name }`：

- manager 更新 `self.local.name`；
- 向 support `name_update` 的 peer 发送 `NameUpdate`；
- 旧 peer 不发送。

`NetEvent::PeerNameChanged`：

- App 更新 `PeerView.name`；
- 重新计算 display_name（保留 short_no）。

## Hello/能力

- Hello 增加 capability `name_update`。
- 新 peer 握手时用新 Hello.name。
- 旧 peer 重连后才看到新名字。

## 状态栏

`ui/status.rs`：

- List/Compose first `本机 {self_display_name()}`。
- Help 末尾 `本机 ...`。
- 仍保留在线数、消息数、选中、模式、连接状态。

## 历史

- 新消息用新名字；
- 旧消息不改；
- `Message.name` 是显示名。

## 测试

- 命令解析；
- 配置保留注释；
- App 持久化/状态；
- 网络 NameUpdate（代码路径）；
- 0.1.6 111 个测试回归。
