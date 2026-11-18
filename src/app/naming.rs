/// Number of node_id characters shown in UI/display names.
pub const SHORT_NODE_ID_LEN: usize = 8;

/// Short, stable display prefix of a node id. UUIDs are 36 chars; the first
/// 8 hex chars are enough to distinguish peers in a LAN UI.
pub fn short_node_id(node_id: &str) -> String {
    let node_id = node_id.trim();
    node_id.chars().take(SHORT_NODE_ID_LEN).collect()
}

/// Compose the display name used in messages and the member sidebar.
///
/// The configured name and the protocol `name` stay untouched; only the
/// displayed/history value gets the node id suffix.
pub fn display_name(base: &str, node_id: &str, short_no: u32) -> String {
    let cleaned = clean_base(base);
    let base = strip_existing_suffix(&cleaned);
    let suffix = if short_no > 0 {
        format!("no{short_no}")
    } else {
        short_node_id(node_id)
    };
    if suffix.is_empty() {
        base.to_string()
    } else {
        format!("{base} ({suffix})")
    }
}

/// Remove an existing ` (8 hex chars)` suffix so display names do not become
/// `alice (old12345) (new12345)` after a node id change.
fn strip_existing_suffix(base: &str) -> &str {
    let Some(inner) = base
        .strip_suffix(')')
        .and_then(|trimmed| trimmed.rsplit_once(" ("))
    else {
        return base;
    };
    let (head, suffix) = inner;
    let is_node8 =
        suffix.len() == SHORT_NODE_ID_LEN && suffix.chars().all(|ch| ch.is_ascii_hexdigit());
    let is_no = suffix
        .strip_prefix("no")
        .is_some_and(|number| !number.is_empty() && number.chars().all(|ch| ch.is_ascii_digit()));
    if is_node8 || is_no {
        head
    } else {
        base
    }
}

fn clean_base(base: &str) -> String {
    let base: String = base
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect();
    let base = base.trim();
    if base.is_empty() {
        "用户".to_string()
    } else {
        base.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_short_node_id() {
        assert_eq!(
            display_name("alice@arch", "19b9b31e-c81d-4c6a-8af3-27c16587b238", 1),
            "alice@arch (no1)"
        );
    }

    #[test]
    fn short_node_id_handles_short_values() {
        assert_eq!(short_node_id("peer-1"), "peer-1");
        assert_eq!(short_node_id("123456789"), "12345678");
        assert_eq!(short_node_id(""), "");
    }

    #[test]
    fn empty_base_falls_back_to_user() {
        assert_eq!(display_name("", "abcd1234", 1), "用户 (no1)");
        assert_eq!(display_name("   ", "abcd1234", 1), "用户 (no1)");
    }

    #[test]
    fn control_characters_are_cleaned() {
        assert_eq!(display_name("a\nb", "abcd1234", 1), "a b (no1)");
    }

    #[test]
    fn replaces_an_existing_node_suffix() {
        assert_eq!(
            display_name("alice (deadbeef)", "abcd1234", 1),
            "alice (no1)"
        );
        assert_eq!(
            display_name("alice (nothex!!)", "abcd1234", 1),
            "alice (nothex!!) (no1)"
        );
    }
}
