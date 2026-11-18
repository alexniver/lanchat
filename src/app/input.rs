#[derive(Debug, Clone, Default)]
pub struct InputState {
    pub text: String,
    /// Byte offset, always on a UTF-8 character boundary.
    pub cursor: usize,
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn trimmed(&self) -> &str {
        self.text.trim()
    }

    pub fn insert_char(&mut self, ch: char) {
        self.text.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    pub fn insert_text(&mut self, text: &str) {
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        for ch in normalized.chars() {
            match ch {
                '\n' | '\t' => self.insert_char(' '),
                ch if ch.is_control() => self.insert_char(' '),
                ch => self.insert_char(ch),
            }
        }
    }

    pub fn backspace(&mut self) {
        if let Some(prev) = self.prev_boundary(self.cursor) {
            self.text.remove(prev);
            self.cursor = prev;
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.text.len() {
            self.text.remove(self.cursor);
        }
    }

    pub fn left(&mut self) {
        if let Some(prev) = self.prev_boundary(self.cursor) {
            self.cursor = prev;
        }
    }

    pub fn right(&mut self) {
        if let Some(next) = self.next_boundary(self.cursor) {
            self.cursor = next;
        }
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.text.len();
    }

    pub fn delete_word_back(&mut self) {
        let mut pos = self.cursor;
        while let Some(prev) = self.prev_boundary(pos) {
            let ch = self.text[prev..pos].chars().next().unwrap_or(' ');
            if !ch.is_whitespace() {
                break;
            }
            pos = prev;
        }
        while let Some(prev) = self.prev_boundary(pos) {
            let ch = self.text[prev..pos].chars().next().unwrap_or(' ');
            if ch.is_whitespace() {
                break;
            }
            pos = prev;
        }
        self.text.replace_range(pos..self.cursor, "");
        self.cursor = pos;
    }

    pub fn delete_to_start(&mut self) {
        self.text.replace_range(..self.cursor, "");
        self.cursor = 0;
    }

    pub fn delete_to_end(&mut self) {
        self.text.truncate(self.cursor);
    }

    fn prev_boundary(&self, pos: usize) -> Option<usize> {
        if pos == 0 || pos > self.text.len() {
            return None;
        }
        let mut prev = pos - 1;
        while prev > 0 && !self.text.is_char_boundary(prev) {
            prev -= 1;
        }
        Some(prev)
    }

    fn next_boundary(&self, pos: usize) -> Option<usize> {
        if pos >= self.text.len() {
            return None;
        }
        let mut next = pos + 1;
        while next < self.text.len() && !self.text.is_char_boundary(next) {
            next += 1;
        }
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_chinese_and_emoji_by_char_boundary() {
        let mut input = InputState::new();
        input.insert_char('你');
        input.insert_char('好');
        assert_eq!(input.cursor, "你好".len());
        input.left();
        input.insert_char('！');
        assert_eq!(input.text, "你！好");
        input.backspace();
        assert_eq!(input.text, "你好");
        input.right();
        assert_eq!(input.cursor, input.text.len());
    }

    #[test]
    fn inserts_pasted_text_as_one_line() {
        let mut input = InputState::new();
        input.insert_text("a\r\nb\tc\u{0}d");
        assert_eq!(input.text, "a b c d");
    }

    #[test]
    fn deletes_words_and_ranges() {
        let mut input = InputState::new();
        input.insert_text("hello world  ");
        input.delete_word_back();
        assert_eq!(input.text, "hello ");
        input.delete_to_start();
        assert_eq!(input.text, "");
        input.insert_text("abc");
        input.home();
        input.delete_to_end();
        assert_eq!(input.text, "");
    }
}
