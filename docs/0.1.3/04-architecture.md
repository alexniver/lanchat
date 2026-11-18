# 帮助页面架构

> 状态：已实现
> 基线：0.1.2 的单所有者 `App` + `Vec<Layer>` 架构
> 关联：[01-scope.md](01-scope.md) · [02-ui.md](02-ui.md) · [03-content.md](03-content.md)

## 1. Layer

在 `src/app/mod.rs` 的 `Layer` 中增加：

```rust
pub enum Layer {
    List(ListState),
    Compose(ComposeState),
    Members(MemberState),
    Help(HelpState),
}
```

`HelpState` 建议：

```rust
#[derive(Debug, Clone, Default)]
pub struct HelpState {
    /// 当前首行在完整帮助内容中的偏移。
    pub offset: usize,
    /// 由布局/渲染计算的最大偏移，用于钳制。
    pub max_offset: usize,
}
```

- `Help` 是顶层 Layer，压入后覆盖 List 的显示区域。
- `List` 仍在 `layers[0]`，保留 selected / follow_latest。
- `Help` 打开时不 push Compose/Members。
- `Esc` pop Help；如果未来有嵌套层，只 pop 顶层。

## 2. ActionId

新增：

```rust
ActionId::OpenHelp
```

- `parse("open_help")` / `as_str() = "open_help"`。
- 默认绑定 `?`（`binding(&["?"], ActionId::OpenHelp)`）。
- `?` 是 List action；Compose 的 `?` 是文本。
- 是否允许 keymap 重映射/解绑：OQ-13-01。
- `Q` 仍是保留键；Help 内 Q 行为 OQ-13-02。

## 3. Key 处理

当前 `App::handle_key`：

```rust
if self.top_is_compose() {
    self.handle_compose_key(key);
} else if self.top_is_members() {
    self.handle_members_key(key);
} else {
    self.handle_list_key(key);
}
```

增加：

```rust
if self.top_is_help() {
    self.handle_help_key(key);
} else if self.top_is_compose() {
    ...
```

`handle_help_key`：

```rust
match key {
    Esc => self.close_help(),
    Char('j') => self.scroll_help(1),
    Char('k') => self.scroll_help(-1),
    // 可选：Home/End/PgUp/PgDn
    _ => { /* ignore */ }
}
```

- Help 内不进入 `handle_list_key`，避免误触发 `i`/`/`/`y`/Space 等。
- `scroll_help` 使用 `max_offset` 钳制。
- 帮助打开/关闭不产生 Effect。
- 帮助打开/关闭不修改 `ListState`。
- 帮助期间 `NetEvent` 仍可更新 messages/peers/status，但 Help 渲染不依赖它们（状态栏可选显示在线数）。

## 4. 布局

`src/ui/layout.rs` 当前 `split(area, has_compose, has_members)` 返回 list/compose/members/status。

帮助模式：

- `has_compose = false`，`has_members = false`。
- 直接把 `areas.list` 作为 Help panel 区域（全宽、上方）。
- `areas.status` 保持 3 行。
- 不需要新的 layout 分支；`ui::render` 根据 `top_is_help()` 选择渲染 Help 而不是 List。

```rust
if app.top_is_help() {
    help::render(f, areas.list, app);
} else {
    list::render(f, areas.list, app);
    if let Some(compose) = areas.compose { input::render(...) }
    if let Some(members) = areas.members { members::render(...) }
}
status::render(f, areas.status, app);
```

- 帮助面板无成员栏：`has_members=false`。
- 帮助面板无 Compose：`has_compose=false`。
- 最小尺寸判断沿用 `layout::split`（60×15）。
- 帮助面板边框标题 ` 帮助 `。

## 5. 帮助内容生成

建议新增 `src/app/help.rs` 或 `src/ui/help.rs`：

```rust
pub fn lines(app: &App) -> Vec<Line<'static>>;
```

- 使用 `app.keymap.hint(ActionId::...)` 生成动态键位。
- 使用 `env!("CARGO_PKG_VERSION")` 生成版本号。
- 静态中文说明 + 分节标题。
- 不读取磁盘、不发网络请求。
- 内容按宽度换行在 UI 层完成；App 层只提供逻辑行。
- 如果把内容放到 `App` 字段，注意不要写入历史/持久化。

### 滚动和换行

- `help::render` 先按 `inner.width` 把逻辑行包装成渲染行，再按 `offset` 裁剪。
- `max_offset = total_rendered_lines.saturating_sub(viewport_height)`。
- 帮助内容不足一屏时 `max_offset=0`。
- 建议在渲染时计算 `max_offset`，并由 `HelpState` 保存；或由 App 保存终端高度后在按键时计算（二选一，见 OQ-13-05）。
- 滚动单位是渲染行。
- `j/k` 每次 1 行。

### 状态栏

`ui/status.rs` 增加 Help 分支：

```text
按 Esc 退出帮助 · j/k 滚动 · 帮助 1/24 · LanChat 0.1.3 · 在线 2
```

- 优先保留“按 Esc 退出帮助”；极窄时截断版本/在线数。
- 帮助分支不显示 List 的“选中/模式”字段。
- 临时 status 不覆盖帮助状态栏（或只在顶部状态行显示，需定）。
- 状态栏高度仍 3。

## 6. 关闭帮助

```rust
fn close_help(&mut self) {
    if self.top_is_help() {
        self.layers.pop();
    }
    self.dirty = true;
}
```

- 不恢复/修改 List selected。
- 不清空 messages/unread。
- 不重发网络事件。
- 帮助期间收到消息，退出后 List 仍然可见；自动模式选中最新，手动模式保持原选中（由既有 `handle_net_event` 决定）。

## 7. 副作用

Help 打开/滚动/关闭：

- 不产生 `Effect`（Persist/Copy/SendChat/SendFile/Reconnect/Disconnect）。
- 不写历史。
- 不更新 status 的临时消息（除非帮助状态栏需要）。
- 不改变 `should_quit`（Q 行为按 OQ-13-02）。

## 8. 测试

### App 单元测试

- `?` 打开 Help；`Esc` 关闭。
- Compose 下 `?` 输入字符，不打开 Help。
- Help 下 `i`/`/`/`y` 不触发 List action。
- Help 下 `j/k` 改变 offset，边界钳制。
- Help 打开前后 List selected/follow_latest 不变。
- Help 打开/关闭不产生 Effect。
- Help 打开时收到 ChatReceived，退出后消息存在、选中语义正确。
- Q 行为按最终决定。
- 小终端渲染不 panic。

### UI 测试

- 帮助 panel 标题/内容出现。
- 状态栏包含“按 Esc 退出帮助”和滚动位置。
- 长内容可滚动。
- 中文换行不溢出。
- 60×15 以下显示提示。

### 回归

- 0.1.2 的 90 个测试继续通过。
- 0.1.2 文件传递/成员/网络无回归。

## 9. 待确认

- OQ-13-01 `?` 是否可重映射/解绑。
- OQ-13-02 Help 内 Q 是否退出程序。
- OQ-13-03 Members 内 `?`。
- OQ-13-04 状态栏显示版本/在线数。
- OQ-13-05 滚动键 j/k 是否跟随 CursorDown/Up 的 keymap。
- OQ-13-06 再次打开是否回顶部。
- OQ-13-07 Home/End/PgUp/PgDn。
