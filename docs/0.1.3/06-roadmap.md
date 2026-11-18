# 0.1.3 实现路线图

> 状态：已实现，验收中
> 基线：0.1.2 已实现并冻结
> 关联：[01-scope.md](01-scope.md) · [02-ui.md](02-ui.md) · [04-architecture.md](04-architecture.md)

## 1. 总体顺序

```text
M0 HelpState + ActionId + 内容大纲
        │
        ▼
M1 Layer/Layout/Status：? 进入、Esc 退出
        │
        ▼
M2 帮助内容 + 动态键位
        │
        ▼
M3 j/k 滚动、钳制、resize、测试
        │
        ▼
M4 文档/版本/验收 0.1.3
```

## 2. 里程碑

### M0 — 状态与动作

- 新增 `HelpState { offset, max_offset }`。
- `Layer::Help(HelpState)`。
- `ActionId::OpenHelp` + 默认 `?`。
- `App::top_is_help()`。
- `handle_help_key` 骨架（Esc/j/k）。
- 单元测试：Layer/action/keymap 解析。

**退出标准**：

- `?` 能从 List 进入 Help。
- `Esc` 能 pop Help。
- Compose 下 `?` 仍是文本。

### M1 — 布局与状态栏

- `ui::render` 选择 Help vs List。
- 帮助 panel 使用 `areas.list` 区域；无成员栏/Compose。
- 帮助状态栏分支：`按 Esc 退出帮助 · j/k 滚动 · 帮助 x/y · 版本 · 在线 N`。
- 小终端提示复用现有逻辑。
- UI 测试：标题/状态栏/无消息列表。

**退出标准**：

- List 按 `?` 后只看到帮助 + 状态栏。
- `Esc` 后恢复原来的 List。
- 状态栏包含 Esc 提示。

### M2 — 内容

- `help::lines(&app)` 生成分节内容。
- 动态键位（OpenCompose/OpenCommand/OpenHelp/Cursor/Copy/Members/Reconnect/Quit）。
- 版本号用 `env!("CARGO_PKG_VERSION")`。
- 内容覆盖快速开始、消息、输入、文件、成员/网络、配置、限制。
- 内容测试：关键字符串/版本/动态键位。

**退出标准**：

- 帮助内容可读、分节清楚。
- 用户改 keymap 后帮助显示新键位。
- 无外部文件依赖。

### M3 — 滚动与边界

- `j/k` 滚动一行，offset 钳制。
- 内容不足一屏不越界。
- 终端 resize 重新计算并钳制。
- 帮助打开期间收到消息不改变 offset；退出后 List 正确。
- 可选 Home/End/PgUp/PgDn。
- 测试：滚动边界、resize、后台消息、无 Effect。

**退出标准**：

- 从顶到底再按 j 不 panic、不越界。
- 状态栏位置随滚动更新。
- 退出/再次进入行为符合 OQ-13-06。

### M4 — 文档/验收

- README 增加 `?` 帮助入口说明。
- 0.1.3 文档状态更新。
- 版本号改为 0.1.3（用户确认后）。
- 全量 0.1.2 回归测试通过。
- fmt/clippy/test/build --release。

## 3. 任务依赖

| 任务 | 依赖 |
| --- | --- |
| HelpState/ActionId | 无 |
| 打开/退出 | Layer + key handling |
| 布局 | Layer + ui::render |
| 状态栏 | 布局 + HelpState offset |
| 内容 | Keymap hint 接口 |
| 滚动 | 内容渲染行数 + viewport |
| 测试 | 各层 |

## 4. 建议 PR 序列

1. `feat: add help layer and open action`
2. `feat: render help panel and status bar`
3. `feat: add help content with dynamic key hints`
4. `feat: add help scrolling and resize clamping`
5. `test: help page interaction and regression`
6. `docs: 0.1.3 help usage and limitations`

## 5. 发布门槛

- [ ] List `?` 打开帮助；Esc 关闭。
- [ ] 帮助 panel + 3 行状态栏布局正确。
- [ ] 状态栏包含“按 Esc 退出帮助”。
- [ ] j/k 滚动，边界/短内容/resize 安全。
- [ ] 帮助内容包含快速开始、文件传递、成员/网络、配置、限制。
- [ ] 编译版本显示正确。
- [ ] 0.1.2 的 90 个测试通过（当前总计 98）。
- [ ] fmt/clippy/test/build 通过。

## 6. 待确认

- OQ-13-06 再次打开是否回顶部（当前回到顶部）
- OQ-13-07 Home/End/PgUp/PgDn（当前未实现）
- OQ-13-08 动态键位覆盖范围
- OQ-13-09 帮助是否包含安装步骤（当前不包含）
