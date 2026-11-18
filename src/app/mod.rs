pub mod action;
pub mod compose;
pub mod input;
pub mod list;
pub mod message;

use std::time::{Duration, Instant};

use crossterm::event::Event;

use crate::clipboard::Clipboard;
use crate::config::Config;
use crate::key::{ActionId, Key, Keymap, Lookup};
use crate::storage::HistoryHandle;

pub use action::{Effect, Status, StatusKind};
pub use compose::ComposeState;
pub use input::InputState;
pub use list::ListState;
pub use message::{Message, MAX_BODY_BYTES};

#[derive(Debug)]
pub enum Layer {
    List(ListState),
    Compose(ComposeState),
}

pub struct App {
    pub config: Config,
    pub keymap: Keymap,
    pub messages: Vec<Message>,
    pub layers: Vec<Layer>,
    pub status: Option<Status>,
    effects: Vec<Effect>,
    history: HistoryHandle,
    clipboard: Clipboard,
    pending: Vec<Key>,
    pending_at: Option<Instant>,
    dirty: bool,
    should_quit: bool,
}

impl App {
    pub fn new(
        config: Config,
        keymap: Keymap,
        history: HistoryHandle,
        messages: Vec<Message>,
        warnings: Vec<String>,
    ) -> Self {
        let layers = vec![Layer::List(ListState::new(messages.len()))];
        let status = if let Some(warning) = warnings.first() {
            Some(Status::new(
                warning.clone(),
                StatusKind::Error,
                Duration::from_secs(8),
            ))
        } else if config.ui.show_help_hint {
            Some(Status::new(
                format!(
                    "{}:输入 {}/{}:选择 {}/{}:复制 {}:退出",
                    keymap.hint(ActionId::OpenCompose),
                    keymap.hint(ActionId::CursorDown),
                    keymap.hint(ActionId::CursorUp),
                    keymap.hint(ActionId::CopyBody),
                    keymap.hint(ActionId::CopyLine),
                    keymap.hint(ActionId::Quit),
                ),
                StatusKind::Info,
                Duration::from_secs(6),
            ))
        } else {
            None
        };
        Self {
            config,
            keymap,
            messages,
            layers,
            status,
            effects: Vec::new(),
            history,
            clipboard: Clipboard::new(),
            pending: Vec::new(),
            pending_at: None,
            dirty: true,
            should_quit: false,
        }
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn top_is_compose(&self) -> bool {
        matches!(self.layers.last(), Some(Layer::Compose(_)))
    }

    pub fn list(&self) -> Option<&ListState> {
        match self.layers.first() {
            Some(Layer::List(list)) => Some(list),
            _ => None,
        }
    }

    fn list_mut(&mut self) -> Option<&mut ListState> {
        match self.layers.first_mut() {
            Some(Layer::List(list)) => Some(list),
            _ => None,
        }
    }

    pub fn selected_message(&self) -> Option<&Message> {
        let selected = self.list()?.selected?;
        self.messages.get(selected)
    }

    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    pub fn handle_terminal(&mut self, event: Event) {
        match event {
            Event::Key(key) => {
                if let Some(key) = Key::from_event(key) {
                    self.handle_key(key);
                }
            }
            Event::Paste(text) => {
                if let Some(Layer::Compose(compose)) = self.layers.last_mut() {
                    compose.input.insert_text(&text);
                    self.dirty = true;
                }
            }
            Event::Resize(_, _) => {
                self.dirty = true;
            }
            _ => {}
        }
    }

    pub fn handle_key(&mut self, key: Key) {
        if self.top_is_compose() {
            self.handle_compose_key(key);
        } else {
            self.handle_list_key(key);
        }
    }

    fn handle_list_key(&mut self, key: Key) {
        if key == Key::Esc && !self.pending.is_empty() {
            self.clear_pending();
            return;
        }
        if key == Key::Ctrl('c') {
            self.clear_pending();
            self.status = None;
            self.dirty = true;
            return;
        }

        let mut sequence = self.pending.clone();
        sequence.push(key);
        match self.keymap.lookup(&sequence) {
            Lookup::Action(action) => {
                self.pending.clear();
                self.pending_at = None;
                self.apply_list_action(action);
            }
            Lookup::Prefix => {
                self.pending = sequence;
                self.pending_at = Some(Instant::now());
                self.dirty = true;
            }
            Lookup::None => {
                if self.pending.is_empty() {
                    return;
                }
                self.clear_pending();
                if let Lookup::Action(action) = self.keymap.lookup(&[key]) {
                    self.apply_list_action(action);
                } else if self.keymap.lookup(&[key]) == Lookup::Prefix {
                    self.pending.push(key);
                    self.pending_at = Some(Instant::now());
                    self.dirty = true;
                }
            }
        }
    }

    fn apply_list_action(&mut self, action: ActionId) {
        match action {
            ActionId::OpenCompose => {
                let Some(list) = self.list() else {
                    return;
                };
                let compose = ComposeState::new(list.selected, list.follow_latest);
                self.layers.push(Layer::Compose(compose));
                self.status = None;
                self.dirty = true;
            }
            ActionId::CursorDown => {
                let len = self.messages.len();
                if let Some(list) = self.list_mut() {
                    list.move_selection(1, len);
                }
                self.dirty = true;
            }
            ActionId::CursorUp => {
                let len = self.messages.len();
                if let Some(list) = self.list_mut() {
                    list.move_selection(-1, len);
                }
                self.dirty = true;
            }
            ActionId::CopyBody => self.copy_selected(false),
            ActionId::CopyLine => self.copy_selected(true),
            ActionId::Quit => {
                self.should_quit = true;
                self.dirty = true;
            }
            ActionId::Noop => {
                self.status = None;
                self.dirty = true;
            }
        }
    }

    fn handle_compose_key(&mut self, key: Key) {
        match key {
            Key::Enter => {
                self.submit_compose();
                return;
            }
            Key::Esc => {
                self.cancel_compose();
                return;
            }
            _ => {}
        }

        let Some(Layer::Compose(compose)) = self.layers.last_mut() else {
            return;
        };
        let changed = match key {
            Key::Char(ch) => {
                compose.input.insert_char(ch);
                true
            }
            Key::Space => {
                compose.input.insert_char(' ');
                true
            }
            Key::Backspace => {
                compose.input.backspace();
                true
            }
            Key::Delete => {
                compose.input.delete();
                true
            }
            Key::Left => {
                compose.input.left();
                true
            }
            Key::Right => {
                compose.input.right();
                true
            }
            Key::Home => {
                compose.input.home();
                true
            }
            Key::End => {
                compose.input.end();
                true
            }
            Key::Ctrl('w') => {
                compose.input.delete_word_back();
                true
            }
            Key::Ctrl('u') => {
                compose.input.delete_to_start();
                true
            }
            Key::Ctrl('k') => {
                compose.input.delete_to_end();
                true
            }
            Key::Ctrl('a') => {
                compose.input.home();
                true
            }
            Key::Ctrl('e') => {
                compose.input.end();
                true
            }
            _ => false,
        };
        if changed {
            self.dirty = true;
        }
    }

    fn submit_compose(&mut self) {
        let Some(Layer::Compose(compose)) = self.layers.last() else {
            return;
        };
        let text = compose.input.trimmed().to_string();
        if text.is_empty() {
            self.cancel_compose();
            return;
        }
        if !crate::app::message::body_is_valid(&text) {
            let message = format!("消息过长（上限 {MAX_BODY_BYTES} 字节）");
            self.set_status(message, StatusKind::Error);
            return;
        }
        let saved_selected = compose.saved_selected;
        let saved_follow_latest = compose.saved_follow_latest;
        let name = self.config.identity.name.clone();
        let message = Message::new(name, text);
        self.messages.push(message.clone());
        self.effects.push(Effect::Persist(message));
        self.layers.pop();
        let last = self.messages.len() - 1;
        if let Some(list) = self.list_mut() {
            if saved_follow_latest {
                list.on_new_message(last + 1);
            } else {
                list.selected = saved_selected;
                list.follow_latest = false;
            }
        }
        self.set_status("已发送", StatusKind::Success);
    }

    fn cancel_compose(&mut self) {
        let Some(layer) = self.layers.pop() else {
            return;
        };
        if let Layer::Compose(compose) = layer {
            let message_count = self.messages.len();
            if let Some(list) = self.list_mut() {
                list.restore(
                    compose.saved_selected,
                    compose.saved_follow_latest,
                    message_count,
                );
            }
        }
        self.status = None;
        self.dirty = true;
    }

    fn copy_selected(&mut self, full_line: bool) {
        let Some(message) = self.selected_message() else {
            self.set_status("没有可复制的消息", StatusKind::Error);
            return;
        };
        let text = if full_line {
            message.format_line()
        } else {
            message.body.clone()
        };
        self.effects.push(Effect::Copy { text });
        self.dirty = true;
    }

    pub fn on_clipboard_result(&mut self, result: anyhow::Result<()>) {
        match result {
            Ok(()) => self.set_status("复制成功", StatusKind::Success),
            Err(error) => self.set_status(format!("复制失败: {error}"), StatusKind::Error),
        }
    }

    pub fn on_history_error(&mut self, error: String) {
        self.set_status(error, StatusKind::Error);
    }

    pub fn on_tick(&mut self, now: Instant) {
        if let Some(started) = self.pending_at {
            if now.duration_since(started) >= Duration::from_secs(1) {
                self.clear_pending();
            }
        }
        if self
            .status
            .as_ref()
            .is_some_and(|status| status.is_expired(now))
        {
            self.status = None;
            self.dirty = true;
        }
    }

    pub fn persist_message(&self, message: &Message) {
        self.history.append(message);
    }

    pub fn copy_text(&mut self, text: &str) -> anyhow::Result<()> {
        self.clipboard.copy(text)
    }

    pub fn shutdown(&mut self) {
        self.history.shutdown();
    }

    fn clear_pending(&mut self) {
        self.pending.clear();
        self.pending_at = None;
        self.dirty = true;
    }

    fn set_status(&mut self, text: impl Into<String>, kind: StatusKind) {
        self.status = Some(Status::new(text, kind, Duration::from_secs(2)));
        self.dirty = true;
    }
}

#[cfg(test)]
pub(crate) fn test_app(messages: Vec<Message>) -> App {
    use crate::config::{HistoryConfig, Identity, Paths, UiConfig};
    use crate::storage::HistoryHandle;
    use std::path::PathBuf;

    let config = Config {
        paths: Paths {
            config_dir: PathBuf::from("/tmp/lanchat-ui-test"),
            config_file: PathBuf::from("/tmp/lanchat-ui-test/config.toml"),
            keymap_file: PathBuf::from("/tmp/lanchat-ui-test/keymap.toml"),
            identity_file: PathBuf::from("/tmp/lanchat-ui-test/identity.json"),
            data_dir: PathBuf::from("/tmp/lanchat-ui-test/data"),
            history_dir: PathBuf::from("/tmp/lanchat-ui-test/history"),
            state_dir: PathBuf::from("/tmp/lanchat-ui-test/state"),
            log_dir: PathBuf::from("/tmp/lanchat-ui-test/logs"),
            panic_log: PathBuf::from("/tmp/lanchat-ui-test/panic.log"),
        },
        identity: Identity {
            node_id: "node".to_string(),
            name: "alice@arch".to_string(),
        },
        history: HistoryConfig {
            enabled: false,
            ..HistoryConfig::default()
        },
        ui: UiConfig {
            show_help_hint: false,
        },
        warnings: Vec::new(),
    };
    App::new(
        config,
        Keymap::defaults(),
        HistoryHandle::disabled(),
        messages,
        Vec::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{HistoryConfig, Identity, Paths, UiConfig};
    use crate::storage::HistoryHandle;
    use std::path::PathBuf;

    fn test_config() -> Config {
        Config {
            paths: Paths {
                config_dir: PathBuf::from("/tmp/lanchat-test"),
                config_file: PathBuf::from("/tmp/lanchat-test/config.toml"),
                keymap_file: PathBuf::from("/tmp/lanchat-test/keymap.toml"),
                identity_file: PathBuf::from("/tmp/lanchat-test/identity.json"),
                data_dir: PathBuf::from("/tmp/lanchat-test/data"),
                history_dir: PathBuf::from("/tmp/lanchat-test/history"),
                state_dir: PathBuf::from("/tmp/lanchat-test/state"),
                log_dir: PathBuf::from("/tmp/lanchat-test/logs"),
                panic_log: PathBuf::from("/tmp/lanchat-test/panic.log"),
            },
            identity: Identity {
                node_id: "node".to_string(),
                name: "alice@arch".to_string(),
            },
            history: HistoryConfig {
                enabled: false,
                ..HistoryConfig::default()
            },
            ui: UiConfig {
                show_help_hint: false,
            },
            warnings: Vec::new(),
        }
    }

    fn app_with(messages: Vec<Message>) -> App {
        App::new(
            test_config(),
            Keymap::defaults(),
            HistoryHandle::disabled(),
            messages,
            Vec::new(),
        )
    }

    fn message(body: &str) -> Message {
        Message::new("alice@arch", body)
    }

    #[test]
    fn compose_sends_and_auto_selects_latest() {
        let mut app = app_with(vec![message("one"), message("two")]);
        app.handle_key(Key::Char('i'));
        assert!(app.top_is_compose());
        for ch in "你好".chars() {
            app.handle_key(Key::Char(ch));
        }
        app.handle_key(Key::Enter);
        assert!(!app.top_is_compose());
        assert_eq!(app.messages.len(), 3);
        assert_eq!(app.messages[2].body, "你好");
        assert_eq!(app.list().unwrap().selected, Some(2));
        assert!(app.list().unwrap().follow_latest);
        assert!(matches!(
            app.take_effects().first(),
            Some(Effect::Persist(_))
        ));
    }

    #[test]
    fn compose_ignores_list_shortcuts_and_esc_cancels() {
        let mut app = app_with(vec![message("one"), message("two")]);
        app.handle_key(Key::Char('i'));
        for key in ['j', 'k', 'y', 'Y', 'i'] {
            app.handle_key(Key::Char(key));
        }
        assert_eq!(app.messages.len(), 2);
        let Layer::Compose(compose) = app.layers.last().unwrap() else {
            panic!("compose expected");
        };
        assert_eq!(compose.input.text, "jkyYi");
        app.handle_key(Key::Esc);
        assert!(!app.top_is_compose());
        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.list().unwrap().selected, Some(1));
    }

    #[test]
    fn manual_mode_does_not_follow_self_sent_messages() {
        let mut app = app_with(vec![message("one"), message("two")]);
        app.handle_key(Key::Char('k'));
        assert_eq!(app.list().unwrap().selected, Some(0));
        assert!(!app.list().unwrap().follow_latest);

        app.handle_key(Key::Char('i'));
        app.handle_key(Key::Char('x'));
        app.handle_key(Key::Enter);
        assert_eq!(app.messages.len(), 3);
        assert_eq!(app.list().unwrap().selected, Some(0));
        assert!(!app.list().unwrap().follow_latest);

        app.handle_key(Key::Char('j'));
        assert_eq!(app.list().unwrap().selected, Some(1));
        assert!(!app.list().unwrap().follow_latest);
        app.handle_key(Key::Char('j'));
        assert_eq!(app.list().unwrap().selected, Some(2));
        assert!(app.list().unwrap().follow_latest);
    }

    #[test]
    fn empty_enter_cancels_without_message() {
        let mut app = app_with(vec![message("one")]);
        app.handle_key(Key::Char('i'));
        app.handle_key(Key::Char(' '));
        app.handle_key(Key::Enter);
        assert_eq!(app.messages.len(), 1);
        assert!(!app.top_is_compose());
    }

    #[test]
    fn q_quits_in_list_but_is_text_in_compose() {
        let mut app = app_with(vec![]);
        app.handle_key(Key::Char('i'));
        app.handle_key(Key::Char('Q'));
        assert!(!app.should_quit());
        let Layer::Compose(compose) = app.layers.last().unwrap() else {
            panic!("compose expected");
        };
        assert_eq!(compose.input.text, "Q");
        app.handle_key(Key::Esc);
        app.handle_key(Key::Char('Q'));
        assert!(app.should_quit());
    }

    #[test]
    fn y_and_shift_y_copy_body_and_full_line() {
        let mut app = app_with(vec![message("hello")]);
        app.handle_key(Key::Char('y'));
        let effects = app.take_effects();
        assert!(matches!(&effects[0], Effect::Copy { text } if text == "hello"));

        app.handle_key(Key::Char('Y'));
        let effects = app.take_effects();
        assert!(
            matches!(&effects[0], Effect::Copy { text } if text.starts_with("[alice@arch][") && text.ends_with("]: hello"))
        );
    }

    #[test]
    fn clipboard_result_updates_status() {
        let mut app = app_with(vec![]);
        app.on_clipboard_result(Ok(()));
        assert_eq!(app.status.as_ref().unwrap().text, "复制成功");
        app.on_clipboard_result(Err(anyhow::anyhow!("no clipboard")));
        assert!(app.status.as_ref().unwrap().text.contains("复制失败"));
    }

    #[test]
    fn manual_selection_restores_on_cancel() {
        let mut app = app_with(vec![message("one"), message("two"), message("three")]);
        app.handle_key(Key::Char('k'));
        let before = app.list().unwrap().selected;
        app.handle_key(Key::Char('i'));
        app.handle_key(Key::Esc);
        assert_eq!(app.list().unwrap().selected, before);
        assert!(!app.list().unwrap().follow_latest);
    }
}
