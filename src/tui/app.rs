use crate::state::AppState;

/// TUI 标签页
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Chat,
    Files,
}

impl Tab {
    /// 切换到另一个标签
    pub fn toggle(self) -> Self {
        match self {
            Tab::Chat => Tab::Files,
            Tab::Files => Tab::Chat,
        }
    }
}

/// UI 交互状态（非共享状态，仅 TUI 线程使用）
pub struct UiState {
    /// 当前选中的标签页
    pub active_tab: Tab,
    /// 消息列表滚动偏移（0 = 底部最新）
    pub chat_scroll: usize,
    /// 输入框文字
    pub input: String,
    /// 输入框光标位置
    pub input_cursor: usize,
    /// 文件视图 — 左栏选中节点索引
    pub files_node_idx: usize,
    /// 文件视图 — 右栏选中文件索引
    pub files_file_idx: usize,
}

impl UiState {
    pub fn new() -> Self {
        Self {
            active_tab: Tab::Chat,
            chat_scroll: 0,
            input: String::new(),
            input_cursor: 0,
            files_node_idx: 0,
            files_file_idx: 0,
        }
    }

    /// 将输入框光标向左移动
    pub fn cursor_left(&mut self) {
        if self.input_cursor > 0 {
            self.input_cursor -= 1;
        }
    }

    /// 将输入框光标向右移动
    pub fn cursor_right(&mut self) {
        if self.input_cursor < self.input.len() {
            self.input_cursor += 1;
        }
    }

    /// 在光标位置插入字符
    pub fn input_insert(&mut self, ch: char) {
        self.input.insert(self.input_cursor, ch);
        self.input_cursor += 1;
    }

    /// 删除光标前一个字符（Backspace）
    pub fn input_backspace(&mut self) {
        if self.input_cursor > 0 {
            self.input_cursor -= 1;
            self.input.remove(self.input_cursor);
        }
    }

    /// 删除光标后一个字符（Delete）
    pub fn input_delete(&mut self) {
        if self.input_cursor < self.input.len() {
            self.input.remove(self.input_cursor);
        }
    }

    /// 滚动聊天消息
    pub fn chat_scroll_up(&mut self, state: &AppState) {
        let max_scroll = state.messages.len().saturating_sub(1);
        if self.chat_scroll < max_scroll {
            self.chat_scroll += 1;
        }
    }

    pub fn chat_scroll_down(&mut self) {
        if self.chat_scroll > 0 {
            self.chat_scroll -= 1;
        }
    }

    pub fn chat_scroll_bottom(&mut self) {
        self.chat_scroll = 0;
    }

    /// 文件视图 — 上移选择
    pub fn files_select_up(&mut self) {
        if self.files_node_idx > 0 {
            self.files_node_idx -= 1;
        }
    }

    pub fn files_select_down(&mut self, max_nodes: usize) {
        if max_nodes > 0 && self.files_node_idx + 1 < max_nodes {
            self.files_node_idx += 1;
        }
    }

    pub fn files_file_up(&mut self) {
        if self.files_file_idx > 0 {
            self.files_file_idx -= 1;
        }
    }

    pub fn files_file_down(&mut self, max_files: usize) {
        if max_files > 0 && self.files_file_idx + 1 < max_files {
            self.files_file_idx += 1;
        }
    }
}
