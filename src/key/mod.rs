pub mod action_id;
pub mod config;
pub mod default;
pub mod keys;
pub mod tree;

use std::collections::BTreeMap;
use std::path::Path;

pub use action_id::ActionId;
pub use config::KeymapFile;
pub use keys::{Key, KeySequence};
pub use tree::{display_sequence, KeyNode, Lookup};

use crate::cli::Cli;
use crate::config::Config;

#[derive(Debug, Clone)]
pub struct Binding {
    pub keys: KeySequence,
    pub action: ActionId,
}

#[derive(Debug, Clone)]
pub struct Keymap {
    root: KeyNode,
    bindings: BTreeMap<KeySequence, ActionId>,
    warnings: Vec<String>,
}

impl Keymap {
    pub fn defaults() -> Self {
        let mut bindings = BTreeMap::new();
        for binding in default::bindings() {
            bindings.insert(binding.keys, binding.action);
        }
        let root = KeyNode::build(&bindings).expect("默认键位无冲突");
        Self {
            root,
            bindings,
            warnings: Vec::new(),
        }
    }

    pub fn from_file(file: &KeymapFile) -> Self {
        let mut bindings = BTreeMap::new();
        for binding in default::bindings() {
            bindings.insert(binding.keys, binding.action);
        }
        let mut warnings = Vec::new();

        for (key_text, action_text) in &file.list {
            let sequence = match Key::parse_sequence(key_text) {
                Ok(sequence) => sequence,
                Err(error) => {
                    warnings.push(format!("keymap: 忽略按键 {key_text:?}: {error}"));
                    continue;
                }
            };
            if sequence.len() == 1 && sequence[0].is_reserved_list_key() {
                warnings.push("keymap: Q 是退出保留键，不能重映射或解绑".to_string());
                continue;
            }
            if action_text == "none" {
                bindings.remove(&sequence);
                continue;
            }
            let action = match ActionId::parse(action_text) {
                Ok(action) => action,
                Err(error) => {
                    warnings.push(format!(
                        "keymap: 忽略 {key_text:?} = {action_text:?}: {error}"
                    ));
                    continue;
                }
            };
            if let Some(existing) = bindings.keys().find(|existing| {
                *existing != &sequence
                    && (is_prefix(existing, &sequence) || is_prefix(&sequence, existing))
            }) {
                warnings.push(format!(
                    "keymap: 前缀冲突，忽略 {:?} = {:?}（与 {:?} 冲突）",
                    key_text,
                    action_text,
                    display_sequence(existing)
                ));
                continue;
            }
            bindings.insert(sequence, action);
        }

        match KeyNode::build(&bindings) {
            Ok(root) => Self {
                root,
                bindings,
                warnings,
            },
            Err(error) => {
                warnings.push(format!("keymap: 构建失败，回退默认: {error}"));
                let mut defaults = Self::defaults();
                defaults.warnings.extend(warnings);
                defaults
            }
        }
    }

    pub fn lookup(&self, keys: &[Key]) -> Lookup {
        self.root.lookup(keys)
    }

    pub fn keys_for(&self, action: ActionId) -> Vec<String> {
        let mut result: Vec<String> = self
            .bindings
            .iter()
            .filter(|(_, bound)| **bound == action)
            .map(|(sequence, _)| display_sequence(sequence))
            .collect();
        result.sort();
        result
    }

    pub fn hint(&self, action: ActionId) -> String {
        let keys = self.keys_for(action);
        if keys.is_empty() {
            "未绑定".to_string()
        } else {
            keys.join("/")
        }
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
}

pub fn load(cli: &Cli, config: &Config) -> Keymap {
    if cli.no_config {
        return Keymap::defaults();
    }
    let path = &config.paths.keymap_file;
    if !path.exists() {
        return Keymap::defaults();
    }
    match load_file(path) {
        Ok(file) => Keymap::from_file(&file),
        Err(error) => {
            let mut keymap = Keymap::defaults();
            keymap
                .warnings
                .push(format!("keymap 读取失败，回退默认: {error}"));
            keymap
        }
    }
}

fn load_file(path: &Path) -> anyhow::Result<KeymapFile> {
    let text = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&text)?)
}

fn is_prefix(prefix: &[Key], full: &[Key]) -> bool {
    prefix.len() < full.len() && full.starts_with(prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keymap(text: &str) -> Keymap {
        let file: KeymapFile = toml::from_str(text).unwrap();
        Keymap::from_file(&file)
    }

    #[test]
    fn default_bindings_are_present() {
        let keymap = Keymap::defaults();
        assert_eq!(
            keymap.lookup(&[Key::Char('i')]),
            Lookup::Action(ActionId::OpenCompose)
        );
        assert_eq!(
            keymap.lookup(&[Key::Char('j')]),
            Lookup::Action(ActionId::CursorDown)
        );
        assert_eq!(
            keymap.lookup(&[Key::Char('Q')]),
            Lookup::Action(ActionId::Quit)
        );
    }

    #[test]
    fn default_member_and_network_bindings() {
        let keymap = Keymap::defaults();
        assert_eq!(
            keymap.lookup(&[Key::Space, Key::Char('u')]),
            Lookup::Action(ActionId::ToggleMembers)
        );
        assert_eq!(
            keymap.lookup(&[Key::Space, Key::Char('r')]),
            Lookup::Action(ActionId::Reconnect)
        );
        assert_eq!(
            keymap.lookup(&[Key::Space, Key::Char('d')]),
            Lookup::Action(ActionId::Disconnect)
        );
    }

    #[test]
    fn overrides_and_unbinds() {
        let keymap = keymap("[list]\n\"j\" = \"cursor_up\"\n\"k\" = \"none\"\n");
        assert_eq!(
            keymap.lookup(&[Key::Char('j')]),
            Lookup::Action(ActionId::CursorUp)
        );
        assert_eq!(keymap.lookup(&[Key::Char('k')]), Lookup::None);
    }

    #[test]
    fn q_is_reserved() {
        let keymap = keymap("[list]\n\"Q\" = \"copy_body\"\n");
        assert_eq!(
            keymap.lookup(&[Key::Char('Q')]),
            Lookup::Action(ActionId::Quit)
        );
        assert!(keymap
            .warnings()
            .iter()
            .any(|warning| warning.contains("保留键")));
    }

    #[test]
    fn unknown_action_is_ignored() {
        let keymap = keymap("[list]\n\"x\" = \"bogus\"\n");
        assert_eq!(keymap.lookup(&[Key::Char('x')]), Lookup::None);
        assert!(keymap
            .warnings()
            .iter()
            .any(|warning| warning.contains("未知 action")));
    }

    #[test]
    fn prefix_conflict_is_ignored() {
        let keymap = keymap("[list]\n\"j k\" = \"cursor_down\"\n\"j\" = \"cursor_up\"\n");
        assert_eq!(
            keymap.lookup(&[Key::Char('j')]),
            Lookup::Action(ActionId::CursorUp)
        );
        assert!(keymap
            .warnings()
            .iter()
            .any(|warning| warning.contains("前缀冲突")));
    }

    #[test]
    fn sequences_work() {
        let keymap = keymap("[list]\n\"g g\" = \"cursor_up\"\n");
        assert_eq!(keymap.lookup(&[Key::Char('g')]), Lookup::Prefix);
        assert_eq!(
            keymap.lookup(&[Key::Char('g'), Key::Char('g')]),
            Lookup::Action(ActionId::CursorUp)
        );
    }
}
