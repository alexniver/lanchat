use std::collections::BTreeMap;

use super::{ActionId, Key};

#[derive(Debug, Clone, Default)]
pub struct KeyNode {
    pub action: Option<ActionId>,
    pub children: BTreeMap<Key, KeyNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lookup {
    Action(ActionId),
    Prefix,
    None,
}

impl KeyNode {
    pub fn build(bindings: &BTreeMap<Vec<Key>, ActionId>) -> Result<Self, String> {
        let mut root = Self::default();
        for (sequence, action) in bindings {
            if sequence.is_empty() {
                return Err("空键序列".to_string());
            }
            let mut node = &mut root;
            for (index, key) in sequence.iter().enumerate() {
                if node.action.is_some() {
                    return Err(format!("键序列前缀冲突: {}", display_sequence(sequence)));
                }
                node = node.children.entry(*key).or_default();
                if index + 1 == sequence.len() {
                    if node.action.is_some() {
                        return Err(format!("重复键位: {}", display_sequence(sequence)));
                    }
                    if !node.children.is_empty() {
                        return Err(format!("键序列前缀冲突: {}", display_sequence(sequence)));
                    }
                    node.action = Some(*action);
                }
            }
        }
        Ok(root)
    }

    pub fn lookup(&self, keys: &[Key]) -> Lookup {
        let mut node = self;
        for key in keys {
            let Some(child) = node.children.get(key) else {
                return Lookup::None;
            };
            node = child;
        }
        if let Some(action) = node.action {
            Lookup::Action(action)
        } else if node.children.is_empty() {
            Lookup::None
        } else {
            Lookup::Prefix
        }
    }
}

pub fn display_sequence(sequence: &[Key]) -> String {
    sequence
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bindings(entries: &[(&[&str], ActionId)]) -> BTreeMap<Vec<Key>, ActionId> {
        entries
            .iter()
            .map(|(keys, action)| {
                (
                    keys.iter()
                        .map(|key| Key::parse(key).unwrap())
                        .collect::<Vec<_>>(),
                    *action,
                )
            })
            .collect()
    }

    #[test]
    fn resolves_single_and_sequence() {
        let tree = KeyNode::build(&bindings(&[
            (&["j"], ActionId::CursorDown),
            (&["g", "g"], ActionId::CursorUp),
        ]))
        .unwrap();
        assert_eq!(
            tree.lookup(&[Key::Char('j')]),
            Lookup::Action(ActionId::CursorDown)
        );
        assert_eq!(tree.lookup(&[Key::Char('g')]), Lookup::Prefix);
        assert_eq!(
            tree.lookup(&[Key::Char('g'), Key::Char('g')]),
            Lookup::Action(ActionId::CursorUp)
        );
        assert_eq!(tree.lookup(&[Key::Char('x')]), Lookup::None);
    }

    #[test]
    fn rejects_prefix_conflicts() {
        assert!(KeyNode::build(&bindings(&[
            (&["g"], ActionId::CursorDown),
            (&["g", "g"], ActionId::CursorUp),
        ]))
        .is_err());
        assert!(KeyNode::build(&bindings(&[
            (&["g", "g"], ActionId::CursorUp),
            (&["g"], ActionId::CursorDown),
        ]))
        .is_err());
    }
}
