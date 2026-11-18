#[derive(Debug, Clone)]
pub struct ListState {
    pub selected: Option<usize>,
    /// true = auto-follow latest; false = manual selection.
    pub follow_latest: bool,
}

impl ListState {
    pub fn new(message_count: usize) -> Self {
        Self {
            selected: message_count.checked_sub(1),
            follow_latest: true,
        }
    }

    pub fn move_selection(&mut self, delta: i32, message_count: usize) {
        if message_count == 0 {
            self.selected = None;
            self.follow_latest = true;
            return;
        }
        let last = message_count - 1;
        let current = self.selected.unwrap_or(last) as i32;
        let next = (current + delta).clamp(0, last as i32) as usize;
        self.selected = Some(next);
        self.follow_latest = next == last;
    }

    pub fn on_new_message(&mut self, message_count: usize) {
        if message_count == 0 {
            self.selected = None;
            self.follow_latest = true;
            return;
        }
        if self.follow_latest {
            self.selected = Some(message_count - 1);
        }
    }

    pub fn restore(&mut self, selected: Option<usize>, follow_latest: bool, message_count: usize) {
        self.follow_latest = follow_latest;
        self.selected = selected.map(|index| index.min(message_count.saturating_sub(1)));
        if message_count == 0 {
            self.selected = None;
            self.follow_latest = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_auto_last() {
        let list = ListState::new(3);
        assert_eq!(list.selected, Some(2));
        assert!(list.follow_latest);
    }

    #[test]
    fn manual_mode_stops_following_and_resumes_at_last() {
        let mut list = ListState::new(3);
        list.move_selection(-1, 3);
        assert_eq!(list.selected, Some(1));
        assert!(!list.follow_latest);
        list.on_new_message(4);
        assert_eq!(list.selected, Some(1));
        assert!(!list.follow_latest);
        list.move_selection(1, 4);
        assert_eq!(list.selected, Some(2));
        assert!(!list.follow_latest);
        list.move_selection(1, 4);
        assert_eq!(list.selected, Some(3));
        assert!(list.follow_latest);
    }

    #[test]
    fn clamps_at_edges_and_handles_empty() {
        let mut list = ListState::new(0);
        list.move_selection(1, 0);
        assert_eq!(list.selected, None);
        let mut list = ListState::new(2);
        list.move_selection(-5, 2);
        assert_eq!(list.selected, Some(0));
        assert!(!list.follow_latest);
        list.move_selection(5, 2);
        assert_eq!(list.selected, Some(1));
        assert!(list.follow_latest);
    }
}
