# 键位方案：stack + 可自定义

> 状态：草案；0.1.0 核心交互见 [08-core-interaction.md](08-core-interaction.md)
> 关联：[01-scope.md](01-scope.md) · [03-ui.md](03-ui.md) · [open-questions.md](open-questions.md)

## 1. 设计原则

1. **栈式交互**：UI 状态是 `Vec<Layer>`；键位只看栈顶。0.1.0 只有 `List` 和 `Compose` 两层。
2. **List 模式用键位**：`i/j/k/y/Y` 等；默认 Helix-inspired，但只保留聊天需要的键。
3. **Compose 模式只认 Enter/Esc**：其他按键全部交给输入框；`j/k/y/i` 输入字符，不触发列表动作。
4. **键位可自定义**：List 动作可通过 `keymap.toml` 覆盖/解绑；Compose 的 Enter/Esc 保留。
5. **安全**：坏配置回退默认；`--no-config` 兜底；退出键始终可用。
6. **可发现**：状态 panel 显示当前层和操作提示；`keymap.toml` 可读；帮助可选。

## 2. 层（Layer）与键位分发

```rust
enum Layer {
    List,
    Compose(ComposeState),
}

struct App {
    layers: Vec<Layer>,   // 至少一个 List
    // ...
}

fn handle_key(&mut self, key: Key) {
    match self.layers.last_mut() {
        Some(Layer::List) => self.handle_list_key(key),
        Some(Layer::Compose(state)) => self.handle_compose_key(state, key),
        None => unreachable!("base List 永不 pop"),
    }
}
```

- 启动：`layers = [List]`。
- List 下按 `i`：`push(Compose)`；输入 panel 出现；列表选中高亮隐藏。
- Compose 下 `Esc`/`Enter`：`pop()`；回 List。
- 基础 `List` 永不 pop；`Esc`/`Enter` 在 List 下不退出/不发送。
- 未来 Help/Confirm 也是层；`Esc` 统一 pop。
- 状态 panel 不参与栈，始终渲染。

## 3. 默认键位（可被用户覆盖）

### 3.1 List 层

| 键 | action | 说明 | 可改 |
| --- | --- | --- | --- |
| `i` | `open_compose` | push Compose，显示输入 panel | ✅ |
| `j` | `cursor_down` | 下移选中，clamp | ✅ |
| `k` | `cursor_up` | 上移选中，clamp | ✅ |
| `y` | `copy_body` | 复制正文；状态“复制成功” | ✅ |
| `Y` | `copy_line` | 复制 `[name][datetime]: message` | ✅ |
| `Esc` | `noop` / `clear_status` | 不退出；可清除临时提示 | ✅（建议保留） |
| `Enter` | `noop` | 不发送；发送只在 Compose | ✅ |
| `Q` | `quit` | 退出（List 层，已确认） | ❌ 保留 |

- `j/k` 同时维护选中模式：选到最新一条 → 自动模式；选到非最新 → 手动模式。手动模式下新消息不自动跳转。
- 默认不绑定 `gg/G`、`Ctrl+d/u`、`/`、`:` 等；它们属于可选增强，不进入 0.1.0 核心。
- 用户可自行添加多键序列；核心验收只要求默认单键可用。

### 3.2 Compose 层

| 键 | 行为 | 可改 |
| --- | --- | --- |
| `Enter` | 非空：发送并 pop；空/纯空白：pop，不发送 | ❌ 保留 |
| `Esc` | pop，丢弃输入，不发送 | ❌ 保留 |
| 普通字符（含 `i/j/k/y/Y`） | 插入输入框 | — |
| `←` / `→` | 光标左右 | ✅（编辑键） |
| `Home` / `End` | 行首/行尾 | ✅ |
| `Backspace` / `Delete` | 删除 | ✅ |
| `Ctrl+w` / `Ctrl+u` / `Ctrl+k` | 删词/删到行首/删到行尾（建议） | ✅ |
| `Q` / `q` | 作为普通字符输入 | — |
| `Ctrl+C` | **不作为应用快捷键**；输入框忽略 | — |
| 其他应用快捷键 | **不响应** | — |

- Compose 下 `j/k` 不会移动列表，而是输入 `j`/`k`。
- `y/Y` 不会复制，而是输入 `y`/`Y`。
- 只有 `Enter`/`Esc` 会改变层；这是用户明确要求。

## 4. 键位配置（`keymap.toml`）

### 4.1 位置

```text
~/.config/lanchat/keymap.toml
```

- `--keymap <path>` 指定其他文件；`--no-config` 跳过用户配置（含 keymap）。
- 文件不存在：使用默认键位，不报错。
- 0.1.0 建议启动时加载；热重载 `:keymap reload` 需要命令层，列为可选（见 OQ-17）。

### 4.2 格式

```toml
[list]
"i" = "open_compose"
"j" = "cursor_down"
"k" = "cursor_up"
"y" = "copy_body"
"Y" = "copy_line"

# Q 退出是保留键，不写入 keymap（不可改）

# 用户自定义：把 j/k 换成方向键
# "down" = "cursor_down"
# "up" = "cursor_up"

# 解绑默认键
# "j" = "none"

# 多键序列（可选能力）
# "g g" = "cursor_top"
```

- 键语法：
  - 单字符：`"i"`、`"Y"`、`"?"`；空格用 `"space"`。
  - 修饰键：`"ctrl+d"`、`"alt+enter"`、`"shift+tab"`。
  - 特殊键：`"esc"`、`"enter"`、`"up"`、`"down"`、`"left"`、`"right"`、`"home"`、`"end"`、`"pageup"`、`"pagedown"`、`"backspace"`、`"delete"`、`"f1"`…`"f12"`。
  - 多键序列：空格分隔，如 `"g g"`。
- 动作值：稳定 action 名，或 `"none"` 解绑。
- 只允许配置 `[list]`；`[compose]` 的 `Enter`/`Esc` 保留，其他编辑键 0.1.0 不做配置（避免输入体验被破坏）。
- 未知 action/按键语法/保留键：报告并忽略该行；其他行继续生效。
- TOML 语法错误：启动回退默认并警告；不阻止启动。

### 4.3 合并规则

1. 加载默认 List 键位。
2. 读取用户 `[list]`，逐条覆盖/新增/解绑。
3. 忽略尝试修改 Compose 保留键的条目。
4. 检测冲突：同一键多动作、前缀冲突。
5. 构建前缀树；失败条目不影响其他条目。

### 4.4 动作注册表（0.1.0 核心）

| action | 说明 |
| --- | --- |
| `open_compose` | push Compose |
| `cursor_down` | 下移选中 |
| `cursor_up` | 上移选中 |
| `copy_body` | 复制正文 |
| `copy_line` | 复制完整格式 |
| `quit` | 退出 |
| `noop` | 无动作 |

可选（不绑定默认）：`cursor_top`、`cursor_bottom`、`page_down`、`page_up`、`copy_selection`、`help`。动作名一旦发布尽量不重命名。

## 5. 键序列引擎

- 0.1.0 核心只需单键 + `Ctrl` 组合；但引擎保留多键前缀能力（为自定义/后续）。
- 数据结构：

```rust
struct Binding { keys: Vec<Key>, action: ActionId }
struct KeyNode { action: Option<ActionId>, children: BTreeMap<Key, KeyNode> }
struct Pending { keys: Vec<Key>, started: Instant }
```

- 超时（默认 1000 ms）清除待决；`Esc` 清除。
- 未知序列：bell + 清除，不崩溃。
- Compose 下不走 List 键位树；只有 Enter/Esc 进入应用逻辑，其余交输入框。

## 6. 退出与保留键

- 0.1.0 已确认：`Q` 退出（List 层），Compose 下 `Q` 是普通字符（先 Esc 回 List）。
- 退出前 flush 历史、恢复终端。
- 保留键：
  - Compose `Enter`：发送/取消，不可改。
  - Compose `Esc`：取消，不可改。
  - List `Q`：退出，保留键（OQ-26 已确认）。
- 如果用户把 `i` 解绑，仍可通过默认/自定义其他键打开输入；若全部解绑，可用 `--no-config` 恢复。
- `Esc` 在 List 下不退出；避免误触。

## 7. 复制

- `y` 复制正文；`Y` 复制完整 `[name][datetime]: message`。
- 本地优先 `arboard`（系统剪贴板）；失败/远程尝试 OSC 52。
- 成功状态“复制成功”2 秒；失败“复制失败”。
- 复制内容与显示一致；`Y` 不含高亮/折行。

## 8. 待确认

| ID | 问题 | 推荐 |
| --- | --- | --- |
| OQ-25 | Compose Enter 语义 | **已确认**：非空发送；空/空白取消；Esc 取消 |
| OQ-26 | 退出键 | **已确认**：List `Q`；Compose 下 Q 是字符 |
| OQ-16 | 保留键范围 | Compose Enter/Esc；List Q（已确认） |
| OQ-17 | keymap 文件格式/重载 | `[list]` 键优先；启动加载；reload 可选 |
| OQ-29 | 剪贴板方案 | arboard + OSC52 兜底 |
| OQ-30 | 状态 panel 高度 | 3 行；不用边框可 2 行 |
| OQ-31 | 手动模式下自己发送消息是否自动跳最新 | 不跳；按 j 到最新后恢复自动 |
