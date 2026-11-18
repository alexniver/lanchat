# 技术架构（0.1.0 单机版）

> 状态：草案；核心交互以 [08-core-interaction.md](08-core-interaction.md) 为准
> 关联：[02-keybindings.md](02-keybindings.md) · [06-quality.md](06-quality.md) · [05-network.md](05-network.md)（0.2）

## 1. 设计目标

1. **栈式 UI**：`Vec<Layer>` 表示界面状态；键位分发只看栈顶；`Esc`/`Enter` pop。
2. **单一状态所有者**：`App` 独占状态，不用 `Arc<Mutex<AppState>>`。
3. **纯状态更新**：按键 → `Action` → 修改 `App` → 产生 `Effect`；IO 由运行时执行。
4. **可测试**：栈、键位、选中、复制、历史、键位配置不依赖真实终端。
5. **不阻塞**：TUI 用 `EventStream`；历史写入后台线程；剪贴板/配置读取用 `spawn_blocking`。
6. **错误可见**：IO/配置/历史/剪贴板错误转为状态提示；不静默丢消息。
7. **终端安全**：正常退出、panic、Q 都恢复终端。
8. **0.2 可接网络**：消息模型/UI 预留 room/from；0.1.0 不引入网络依赖。

## 2. 模块划分（0.1.0）

```text
src/
├── main.rs                 # 入口：CLI、日志、runtime、事件循环
├── cli.rs                  # --config/--keymap/--no-config/--version/--help
├── config.rs               # config.toml、XDG 路径、身份/昵称
├── logging.rs              # tracing、panic hook
├── terminal.rs             # TerminalGuard、raw mode、alternate screen、paste
├── app/
│   ├── mod.rs              # App、Layer 栈、事件分发
│   ├── action.rs           # ActionId / Action / Effect
│   ├── list.rs             # List 层：选中、滚动、复制
│   ├── compose.rs          # Compose 层：输入、Enter/Esc
│   ├── message.rs          # 消息模型、格式化、去重
│   └── input.rs            # 单行编辑器、UTF-8 光标、水平滚动
├── key/
│   ├── mod.rs
│   ├── key.rs              # Key 解析/规范化
│   ├── action_id.rs        # 动作注册表
│   ├── default.rs          # 默认 List 键位
│   ├── config.rs           # keymap.toml
│   └── tree.rs             # 前缀树、冲突检测
├── storage/
│   ├── mod.rs
│   └── history.rs          # JSONL 追加/加载/轮转；后台 writer
├── clipboard.rs            # arboard + OSC 52 兜底
└── ui/
    ├── mod.rs              # render 入口
    ├── layout.rs           # 双 panel + 输入 panel、最小尺寸
    ├── list.rs             # 信息列表、选中高亮、折行
    ├── input.rs            # 输入面板、光标
    ├── status.rs           # 状态 panel、临时提示
    └── theme.rs            # 颜色/符号
```

> 0.2 才加入 `net/`；0.1.0 不创建空网络模块。

## 3. 事件循环

### 3.1 顶层流程

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = cli::parse()?;
    logging::init(&cli)?;
    let config = config::load(&cli)?;
    let identity = config::load_or_create_identity(&cli)?;
    let keymap = key::load(&cli, &config)?;      // 失败回退默认
    let history = storage::History::open(&config)?; // 启动加载最近 N 条

    let _guard = terminal::TerminalGuard::enter()?;
    let mut terminal = terminal::new_terminal()?;

    let mut app = App::new(config, identity, keymap, history);
    let mut events = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(250));
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        if app.dirty() {
            terminal.draw(|f| ui::render(f, &app))?;
            app.clear_dirty();
        }

        tokio::select! {
            Some(term_event) = events.next() => app.handle_terminal(term_event?),
            _ = tick.tick() => app.on_tick(Instant::now()),
            Some(internal) = app.internal_rx.recv() => app.handle_internal(internal),
        }

        for effect in app.take_effects() {
            match effect {
                Effect::Quit => {
                    app.shutdown().await;   // flush 历史
                    return Ok(());
                }
                other => other.execute(&mut app).await?,
            }
        }
    }
}
```

- `EventStream` 避免旧版 `event::poll(16ms)` 阻塞/空转。
- 只有 `dirty` 或 tick 时重绘；tick 250 ms 用于状态提示过期/时钟。
- `internal_rx` 接收后台任务结果（历史错误、剪贴板结果、配置重载）。
- `TerminalGuard` 在 `Drop` 中兜底恢复。

### 3.2 事件来源

```rust
enum InternalEvent {
    HistoryError(String),
    ClipboardDone(Result<(), String>),
}
```

- 终端事件：Key、Paste、Resize；Mouse 关闭。
- Tick：提示过期、待决键超时（若做多键序列）。
- Internal：后台结果。
- 0.1.0 没有 `NetEvent`；0.2 再加 `InternalEvent::Net`。

## 4. App 状态与 Layer 栈

```rust
enum Layer {
    List(ListState),
    Compose(ComposeState),
}

struct ListState {
    selected: Option<usize>,   // 消息索引；None = 无消息
    follow_latest: bool,       // true = 自动；false = 手动
    scroll: usize,             // 显示行偏移
}

struct ComposeState {
    input: InputState,
    saved_selected: Option<usize>,
    saved_follow_latest: bool, // 取消/发送时决定是否恢复自动
}

struct App {
    config: Config,
    identity: Identity,
    keymap: Keymap,

    messages: MessageStore,
    layers: Vec<Layer>,       // 至少一个 List
    status: Status,

    effects: Vec<Effect>,
    internal_rx: mpsc::Receiver<InternalEvent>,
    history: HistoryHandle,
    dirty: bool,
}
```

- 启动：`layers = [Layer::List(ListState { selected: last, follow_latest: true, scroll: bottom })]`。
- `i`：`layers.push(Layer::Compose { input: empty, saved_selected: selected, saved_follow_latest: follow_latest })`。
- `Esc`/空 Enter：pop Compose，恢复 `saved_selected` 与 `saved_follow_latest`。
- 非空 Enter：追加消息，pop Compose。
  - 自动模式（`saved_follow_latest == true`）：选中新消息、滚动到底、保持自动。
  - 手动模式（`saved_follow_latest == false`）：保持 `saved_selected`，不自动跳转。
- 基础 List 永不 pop；`Esc`/`Enter` 在 List 下无动作。
- 状态 panel 不属于栈，始终渲染。
- 选中模式：
  - `follow_latest = true`（自动）：选中始终是最后一条；新消息自动跳到底。
  - `follow_latest = false`（手动）：`j/k` 选中非最新后进入；新消息不改变选中/滚动；用户 `j/k` 选回最后一条后恢复 `true`。
- 状态 panel 显示 `自动`/`手动`，由 `follow_latest` 决定。

```rust
fn move_selection(&mut self, delta: i32) {
    if self.messages.is_empty() { return; }
    let Some(list) = self.list_mut() else { return };
    let last = self.messages.len() - 1;
    let cur = list.selected.unwrap_or(last) as i32;
    let next = (cur + delta).clamp(0, last as i32) as usize;
    list.selected = Some(next);
    list.follow_latest = next == last;   // 选到最新 → 自动；否则手动
    self.scroll_to_selected();
}

fn on_new_message(&mut self, msg: Message) {
    self.messages.push(msg);
    let last = self.messages.len() - 1;
    let Some(list) = self.list_mut() else { return };
    if list.follow_latest {
        list.selected = Some(last);
        self.scroll_to_bottom();
    }
    // 手动模式：selected/scroll 不变，只增加消息数
}
```

- `MessageStore`：`Vec<Message>` + `HashMap<MsgId, usize>`；最多 10,000 条。
- `Message`：`id`、`name`、`datetime`、`body`、`room`、`kind`；0.1.0 只写本机。
- `Status`：临时提示（复制成功/失败、已发送）、最后 tick。

### 4.1 键位分发

```rust
fn handle_key(&mut self, key: Key) {
    match self.layers.last_mut() {
        Some(Layer::List(list)) => {
            if let Some(action) = self.keymap.resolve(key, LayerKind::List) {
                self.apply_list(action, list);
            }
        }
        Some(Layer::Compose(compose)) => {
            match key {
                Key::Enter => self.compose_submit(compose),
                Key::Esc => self.compose_cancel(compose),
                other => compose.input.handle(other), // 文本输入
            }
        }
        None => unreachable!("base List 永不 pop"),
    }
}
```

- **Compose 下不查 List 键位树**；`j/k/y/Y/i` 全部进入输入框。
- `Q` 在 Compose 下是普通字符（先 Esc 回 List）；在 List 下退出。
- 未来 Help/Confirm 作为新 Layer，只有栈顶处理键。

## 5. Action 与 Effect

### 5.1 Action

```rust
enum ActionId {
    OpenCompose,
    CursorDown,
    CursorUp,
    CopyBody,
    CopyLine,
    Quit,
    Noop,
}
```

- List 键位映射到 `ActionId`；`App::apply_list` 修改状态并追加 `Effect`。
- Compose 的 Enter/Esc 是固定逻辑，不走 ActionId；避免用户改坏输入提交。

### 5.2 Effect

```rust
enum Effect {
    PersistMessage(Message),
    CopyToClipboard { text: String, full_line: bool },
    Quit,
}
```

- `PersistMessage`：交给历史 writer；失败 `InternalEvent::HistoryError`，状态提示。
- `CopyToClipboard`：`arboard` 写系统剪贴板；失败尝试 OSC 52；结果 `ClipboardDone`。
- `Quit`：flush 历史、恢复终端、退出。

### 5.3 发送流程

```text
Compose Enter（非空）
  → trim 校验；超长拒绝
  → 生成 Message { name, datetime, body }
  → push 到 MessageStore
  → Effect::PersistMessage
  → pop Compose
  → 自动模式：selected = last、滚动到底
  → 手动模式：保持 saved_selected，不自动跳
  → 状态“已发送”
```

- 空/纯空白 Enter：pop，不产生消息。
- Esc：pop，丢弃输入，不产生消息。
- 历史写入失败：消息仍在内存，状态显示 `! 历史写入失败`，不静默丢。

## 6. 键位引擎与配置

### 6.1 默认键位

```toml
[list]
"i" = "open_compose"
"j" = "cursor_down"
"k" = "cursor_up"
"y" = "copy_body"
"Y" = "copy_line"
# Q 是保留键：默认退出，不写入用户 keymap
```

- 0.1.0 核心单键；引擎保留多键序列能力。
- `keymap.toml` 路径 `~/.config/lanchat/keymap.toml`；`--keymap` 覆盖；`--no-config` 跳过。
- 只配置 `[list]`；Compose 的 `Enter`/`Esc` 固定。

### 6.2 合并/校验

1. 默认 List 绑定 → map by key sequence。
2. 用户 `[list]` 逐条覆盖/新增；`"none"` 删除。
3. 忽略尝试修改 Compose 保留键的条目。
4. 检测同键多动作/前缀冲突。
5. 未知 action/按键语法：报告并忽略该行；其他条目继续。
6. TOML 语法错误：启动回退默认；不阻止启动。
7. `--no-config`：跳过用户 config/keymap。

### 6.3 Action 注册表

`open_compose`、`cursor_down`、`cursor_up`、`copy_body`、`copy_line`、`quit`、`noop`；可选 `cursor_top`、`cursor_bottom`、`page_down`、`page_up`。动作名稳定，帮助/错误信息可读。

## 7. 终端安全

### 7.1 TerminalGuard

```rust
struct TerminalGuard { active: bool }

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen, EnableBracketedPaste, DisableMouseCapture)?;
        Ok(Self { active: true })
    }
    fn restore() {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, DisableBracketedPaste, Show);
    }
}
impl Drop for TerminalGuard {
    fn drop(&mut self) { Self::restore(); }
}
```

- `AtomicBool` 防重复恢复。
- panic hook 先恢复终端，再写日志/调用默认 hook。
- `Ctrl+C` 在 raw mode 下是普通按键；Compose 下忽略，List 下无动作；退出用 `Q`。
- 不在 `Drop` 中 panic；`restore` 全 `let _ =`。

### 7.2 终端能力

- 检测 `TERM`/`NO_COLOR`；不支持颜色时用符号。
- bracketed paste；粘贴内容过滤控制字符。
- 鼠标关闭。
- 最小尺寸 60×15；不足时渲染提示。

## 8. 配置与存储

### 8.1 路径（Linux，XDG）

| 类型 | 路径 |
| --- | --- |
| 配置 | `~/.config/lanchat/config.toml` |
| 键位 | `~/.config/lanchat/keymap.toml` |
| 身份 | `~/.config/lanchat/identity.json`（兼容旧版） |
| 数据/历史 | `~/.local/share/lanchat/history/` |
| 状态/日志 | `~/.local/state/lanchat/logs/` |

### 8.2 config.toml

```toml
[identity]
name = "alice@arch"

[ui]
show_help_hint = true

[history]
enabled = true
max_messages = 10000
max_file_mb = 10
```

- `node_id` 存 `identity.json`；首次生成，旧版存在则迁移。
- CLI > 环境变量 > 配置 > 默认。
- 未知字段忽略并警告；坏配置用默认并提示，不阻止启动（身份损坏除外）。
- 0.1.0 不含网络配置。

### 8.3 历史

```json
{"v":1,"id":"...","room":"lobby","name":"alice@arch","datetime":"2026-11-19T10:30:01+08:00","body":"你好"}
```

- 路径：`~/.local/share/lanchat/history/lobby-YYYY-MM.jsonl`（按月）。
- 追加写；启动加载最近 N 条（默认 5,000 或 10 MB）。
- 权限 `0600`（Unix）；目录 `0700`；坏行跳过并计数。
- 轮转：单文件 10 MB 或按月；保留 30 天/100 MB。
- 后台 writer：有界 channel；退出 flush，最多等 1 秒。
- 日志不写正文；只写 msg_id/room/错误。
- 0.1.0 不提供删除历史命令。

## 9. 剪贴板

- 目标：`y` 正文、`Y` 完整行复制到系统剪贴板。
- 本机优先 `arboard`（X11/Wayland/macOS/Windows）。
- 失败/远程终端尝试 OSC 52：`ESC ] 52 ; c ; <base64> BEL`。
- 两者都失败：状态“复制失败”，不 panic。
- 复制结果通过 `InternalEvent::ClipboardDone` 回到 UI；状态 2 秒后消失。
- 剪贴板操作在 `spawn_blocking`/后台线程，避免阻塞 UI。

## 10. 日志与诊断

- `tracing` + `tracing-subscriber` + `tracing-appender`；默认 INFO，`LANCHAT_LOG=debug`。
- 文件日志按天轮转；保留 7 天；不写聊天正文。
- panic hook：时间、位置、backtrace；先恢复终端。
- `--version`/`:version`（若做命令）显示版本、路径、模式。
- `--debug` 输出 stderr；TUI 运行中会破坏画面，默认只写文件。

## 11. 依赖草案（0.1.0）

| crate | 用途 | 备注 |
| --- | --- | --- |
| `tokio` | async runtime、channel、time、spawn_blocking | features：`rt-multi-thread`、`macros`、`sync`、`time` |
| `ratatui` | TUI 渲染 | 0.29 |
| `crossterm` | 终端/事件 | `event-stream` feature |
| `futures-util` | `EventStream` 的 `StreamExt` | |
| `serde` / `serde_json` | 历史/身份 | |
| `toml` | config/keymap | |
| `anyhow` / `thiserror` | 错误处理 | 边界 anyhow，库内 thiserror |
| `uuid` | node_id / msg_id | v4 + serde |
| `chrono` | 时间戳 | serde |
| `tracing` / `tracing-subscriber` / `tracing-appender` | 日志 | |
| `dirs` | XDG 路径 | |
| `unicode-width` | 显示宽度 | 中文/折行/光标 |
| `base64` | OSC 52 | 仅复制 |
| `arboard` | 系统剪贴板 | 失败时 OSC 52 兜底 |
| `whoami` / `hostname` | 默认昵称 | 或环境变量 |
| dev: `tempfile` | 临时目录测试 | |
| dev: `insta`（可选） | 状态/布局快照 | 避免过度快照 |

**0.1.0 不引入**：`mdns-sd`、`tokio-util`、`bytes`、socket/网络协议、`sha2`、文件共享依赖。

## 12. 测试策略

| 层级 | 内容 |
| --- | --- |
| 单元 | Layer 栈 push/pop；List/Compose 键分发；Compose 忽略 j/k/y/Y/i 为动作；Compose 下 Q 输入字符 |
| 单元 | 选中：默认自动最后；j/k 到非最新转手动；新消息手动不跳；j/k 回最新恢复自动；clamp；空列表 |
| 单元 | 消息格式化 `[name][datetime]: message`；CJK 宽度/折行 |
| 单元 | 输入编辑器：中文/emoji、光标、删除、水平滚动、粘贴 |
| 单元 | keymap.toml：覆盖/解绑/保留键/未知动作/冲突/坏 TOML |
| 单元 | 历史：追加/加载/坏行/权限/轮转/flush |
| 集成 | App + 假历史 writer + 假剪贴板：发送、取消、复制、失败提示 |
| TUI | `TestBackend`：双 panel、输入 panel、选中高亮、状态文字、resize |
| 手工 | CJK 字体、终端 resize、tmux/SSH、OSC52、panic、24h |

- CI：`cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`。
- 不对真实剪贴板做强依赖测试；用 trait/mock。
- 0.2 再加网络集成测试。

## 13. 旧版清理与迁移

- 根 `Cargo.toml` 只保留 0.1.0 依赖；`old/` 被 workspace exclude。
- 旧 `src/` 不参与编译；新代码不 `mod old`。
- 旧身份 `~/.config/lanchat/identity.json`：成功则复用 node_id/display_name；失败生成新身份并提示。
- 旧日志 `log/` 已移动到 `old/log/`；新日志走 XDG state。
- 旧共享目录不再扫描；不删除文件，文档说明 0.1.0 不处理。
- 旧配置/环境变量冲突在 README 说明。

## 14. 0.2 网络接入点（不在 0.1.0）

- `InternalEvent::Net(NetEvent)` 新增变体；
- `Effect::Net(NetCommand)`；
- `net::start()` 返回 `NetHandle` + `NetEvent` receiver；
- `Message` 已有 `room`/`name`；0.2 加 `from`/peer；
- 状态 panel 显示 peer 数/连接状态；成员面板作为新 Layer；
- 历史格式不因网络改变；
- 详细协议见 [05-network.md](05-network.md)。

## 15. 风险与应对

| 风险 | 应对 |
| --- | --- |
| Compose 下误触列表动作 | 键分发只看栈顶；Compose 不查 List 键位树 |
| 退出键被改坏 | `Q` 保留（List）；`--no-config` 兜底 |
| 中文错位 | `unicode-width`；文档要求 CJK 等宽字体 |
| 剪贴板在 SSH/headless 不可用 | arboard + OSC52；失败提示 |
| 历史隐私/磁盘增长 | 不写正文日志；`0600`；轮转；可关闭 |
| 大消息/长历史卡顿 | 16 KiB 上限；只渲染可见行；缓存折行 |
| 终端恢复失败 | TerminalGuard + panic hook + 测试 |
| 范围膨胀（搜索/命令/网络） | 核心验收只按 08 文档；其他 P2/0.2 |

## 16. 待确认

- OQ-25 Compose Enter 语义（已确认）。
- OQ-26 退出键（已确认：List `Q`）。
- OQ-27 datetime 格式（已确认）。
- OQ-28 取消输入后的选中项。
- OQ-29 剪贴板方案（arboard + OSC52）。
- OQ-30 状态 panel 高度（建议 3 行）。
- OQ-31 手动模式下自己发送是否自动跳最新（建议不跳）。
- OQ-17 keymap 文件格式/是否热重载。
