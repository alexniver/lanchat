# 帮助内容

> 状态：已实现
> 关联：[02-ui.md](02-ui.md) · [01-scope.md](01-scope.md)

## 1. 内容原则

1. **能直接照做**：给出键位/命令，不只写概念。
2. **从常用到高级**：快速开始 → 消息 → 文件 → 成员/网络 → 配置 → 限制。
3. **键位动态化**：默认键位写死在文案里会过时；能从 `Keymap` 取到的键位应动态生成。
4. **只读**：帮助内不执行命令，不显示可点击链接。
5. **中文简短**：每行尽量不超过终端宽度，长内容靠换行和滚动。
6. **版本一致**：标题带 `LanChat 0.1.3`；内容随版本更新。

## 2. 推荐分节

```text
LanChat 0.1.3

快速开始
  i          打开输入
  /          命令输入
  ?          打开帮助
  j / k      选择消息 / 滚动帮助
  y / Y      复制正文 / 完整行
  Esc        取消 / 退出帮助
  Q          退出程序（仅 List）

消息列表
  - 默认自动选中最新消息
  - j/k 选到非最新后进入手动模式
  - 手动模式下新消息不自动跳转
  - 选回最新后恢复自动
  - y 复制正文，Y 复制完整行

输入消息
  - Enter 发送；空内容/纯空白取消
  - Esc 取消
  - 输入中只有 Enter/Esc 是应用快捷键
  - 支持 Home/End、Backspace/Delete、Ctrl+a/e/w/u/k

文件传递
  /send file <路径> to <对方>
  /recv file <对方>
  - 对方可用完整 IP、IP 最后一段、昵称
  - 接收方 /recv 后开始传输
  - 请求 10 分钟未接收自动取消
  - 保存到 ~/.local/share/lanchat/files
  - 重名保存为 a[0].png、a[1].png
  - 文件请求/成功/取消是系统消息，不写入历史

成员与网络
  Space u    打开/关闭成员侧边栏
  Space r    重连
  Space d    断开
  - 状态栏在线数包含本机
  - mDNS 自动发现；失败时用 --connect 或 seeds
  - 明文传输，仅可信局域网

配置
  ~/.config/lanchat/config.toml
  ~/.config/lanchat/keymap.toml
  ~/.local/share/lanchat/history/
  ~/.local/state/lanchat/logs/
  - 修改 keymap 后帮助会显示当前键位

限制
  - 单房间、无加密/鉴权、无历史同步、无断点续传
  - 单条消息 16 KiB；文件上限 1 GiB
  - 仅局域网
```

## 3. 动态键位

### 3.1 原则

- 帮助页的键位列应来自当前 `Keymap`/`ActionId`，而不是复制默认常量。
- 如果某个 action 被解绑或改键，帮助应显示：
  - 新键位；或
  - `未绑定`。
- 多键序列显示为 `Space u` 形式，和状态栏提示一致。
- 如果 keymap 加载失败回退默认，帮助显示默认值。

### 3.2 建议映射

| 帮助条目 | 来源 |
| --- | --- |
| 打开输入 | `keymap.hint(ActionId::OpenCompose)` |
| 打开命令 | `keymap.hint(ActionId::OpenCommand)` |
| 打开帮助 | `keymap.hint(ActionId::OpenHelp)` |
| 下/上 | `CursorDown` / `CursorUp` |
| 复制正文/完整行 | `CopyBody` / `CopyLine` |
| 成员栏 | `ToggleMembers` |
| 重连/断开 | `Reconnect` / `Disconnect` |
| 退出 | `Quit` |

- `keymap.hint` 目前用于状态栏，应该能复用；如果需要多键显示格式，补一个 `hints` 方法。
- 帮助自身键位 `j/k/Esc` 固定（不依赖 keymap？或跟随 `CursorDown/Up`，OQ-13-05）。

## 4. 标题/滚动位置

- 帮助内容首行：`LanChat 0.1.3`
- 空行分隔章节。
- 章节标题单独一行，左侧不缩进；条目缩进 2 空格。
- 帮助内容总行数在渲染时计算（含换行后行数）。
- 状态栏 `帮助 1/24` 使用渲染后的首行位置；内容短时 `帮助 1/1` 或 `全部显示`。

## 5. 内容维护

- 帮助文本放在 `src/app/help.rs` 或 `src/ui/help.rs` 的静态函数中，避免散落在 UI 代码。
- 版本号用 `env!("CARGO_PKG_VERSION")`，不要手写错误版本。
- 功能增加时更新帮助分节；发布检查包含“帮助内容与 README 一致”。
- 帮助内容不写入历史/配置；每次打开重新生成（动态键位）。

## 6. 不建议放入的内容

- 完整协议/架构细节（放 README/docs）。
- 大量排障步骤（放 README）。
- 文件路径的绝对示例（依赖用户环境）。
- 交互式“按这里”链接（终端只读文本）。
- 帮助自身快捷键以外的隐藏彩蛋。

## 7. 文案示例

```text
快速开始
  i          输入消息
  /          命令输入
  ?          打开帮助
  j / k      下/上选择消息
  y / Y      复制正文/完整行
  Esc        取消或退出当前层
  Q          退出程序（仅在消息列表）

文件传递
  /send file ~/Download/a.png to 103
  /recv file 101
  文件保存到 ~/.local/share/lanchat/files
```

## 8. 待确认

- 动态键位做到什么程度（全部 action / 仅常用）。
- Help 自身 `j/k` 是否跟随 `CursorDown/Up` 的自定义键。
- 是否显示“未绑定”。
- 内容是否包含 README 的安装步骤（建议不包含，TUI 内不需要）。
