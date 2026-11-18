use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct KeymapFile {
    #[serde(default)]
    pub list: BTreeMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_list_bindings() {
        let file: KeymapFile = toml::from_str(
            r#"
            [list]
            "j" = "cursor_down"
            "ctrl+d" = "cursor_down"
            "x" = "none"
            "#,
        )
        .unwrap();
        assert_eq!(file.list.get("j").map(String::as_str), Some("cursor_down"));
        assert_eq!(
            file.list.get("ctrl+d").map(String::as_str),
            Some("cursor_down")
        );
        assert_eq!(file.list.get("x").map(String::as_str), Some("none"));
    }
}
