use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn char_width(ch: char) -> usize {
    UnicodeWidthChar::width(ch).unwrap_or(0)
}

pub fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;
    for ch in text.chars() {
        if ch == '\n' {
            lines.push(std::mem::take(&mut current));
            current_width = 0;
            continue;
        }
        let ch_width = char_width(ch);
        if current_width + ch_width > width && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current_width = 0;
        }
        current.push(ch);
        current_width += ch_width;
    }
    lines.push(current);
    lines
}

pub fn truncate_display(text: &str, width: usize) -> String {
    if display_width(text) <= width {
        return text.to_string();
    }
    if width == 0 {
        return String::new();
    }
    let ellipsis_width = 1;
    let budget = width.saturating_sub(ellipsis_width);
    let mut result = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let w = char_width(ch);
        if used + w > budget {
            break;
        }
        result.push(ch);
        used += w;
    }
    result.push('…');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_ascii() {
        assert_eq!(wrap_text("abcdef", 3), vec!["abc", "def"]);
    }

    #[test]
    fn wraps_cjk_by_display_width() {
        assert_eq!(wrap_text("你好", 4), vec!["你好"]);
        assert_eq!(wrap_text("你好", 2), vec!["你", "好"]);
    }

    #[test]
    fn truncates_cjk() {
        assert_eq!(truncate_display("你好世界", 5), "你好…");
        assert_eq!(truncate_display("abc", 3), "abc");
    }
}
