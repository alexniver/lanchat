use super::InputState;

#[derive(Debug, Clone)]
pub struct ComposeState {
    pub input: InputState,
    pub saved_selected: Option<usize>,
    pub saved_follow_latest: bool,
}

impl ComposeState {
    pub fn new(saved_selected: Option<usize>, saved_follow_latest: bool) -> Self {
        Self {
            input: InputState::new(),
            saved_selected,
            saved_follow_latest,
        }
    }
}
