use super::{ActionId, Binding, Key};

pub fn bindings() -> Vec<Binding> {
    vec![
        binding(&["i"], ActionId::OpenCompose),
        binding(&["/"], ActionId::OpenCommand),
        binding(&["?"], ActionId::OpenHelp),
        binding(&["j"], ActionId::CursorDown),
        binding(&["k"], ActionId::CursorUp),
        binding(&["y"], ActionId::CopyBody),
        binding(&["Y"], ActionId::CopyLine),
        binding(&["space", "u"], ActionId::ToggleMembers),
        binding(&["space", "r"], ActionId::Reconnect),
        binding(&["space", "d"], ActionId::Disconnect),
        binding(&["Q"], ActionId::Quit),
    ]
}

fn binding(keys: &[&str], action: ActionId) -> Binding {
    Binding {
        keys: keys
            .iter()
            .map(|key| Key::parse(key).expect("内置键位必须合法"))
            .collect(),
        action,
    }
}
