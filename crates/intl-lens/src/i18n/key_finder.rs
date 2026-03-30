use regex::Regex;

#[derive(Debug, Clone)]
pub struct FoundKey {
    pub key: String,
    pub start_offset: usize,
    pub line: usize,
    pub start_char: usize,
    pub end_char: usize,
}

pub struct KeyFinder {
    pattern: Regex,
}

impl KeyFinder {
    pub fn new(function_names: &[String]) -> Self {
        let default_names = default_function_names();
        let names: Vec<String> = function_names
            .iter()
            .map(|name| name.trim())
            .filter(|name| !name.is_empty() && is_simple_function_name(name))
            .map(regex::escape)
            .collect();

        let joined = if names.is_empty() {
            default_names
                .iter()
                .map(|name| regex::escape(name))
                .collect::<Vec<_>>()
                .join("|")
        } else {
            names.join("|")
        };

        let pattern = Regex::new(&format!(
            r#"(?:^|[^\w.])(?:{})\s*\(\s*["']([^"']+)["']"#,
            joined
        ))
        .expect("function name regex should be valid");

        Self { pattern }
    }

    pub fn find_keys(&self, content: &str) -> Vec<FoundKey> {
        let mut found_keys = Vec::new();

        for cap in self.pattern.captures_iter(content) {
            if let Some(key_match) = cap.get(1) {
                let key = key_match.as_str().to_string();
                let start_offset = key_match.start();
                let end_offset = key_match.end();

                let (line, start_char, end_char) =
                    Self::offset_to_position(content, start_offset, end_offset);

                found_keys.push(FoundKey {
                    key,
                    start_offset,
                    line,
                    start_char,
                    end_char,
                });
            }
        }

        found_keys.sort_by_key(|k| k.start_offset);
        found_keys
    }

    pub fn find_key_at_position(
        &self,
        content: &str,
        line: usize,
        character: usize,
    ) -> Option<FoundKey> {
        let keys = self.find_keys(content);

        keys.into_iter()
            .find(|k| k.line == line && character >= k.start_char && character <= k.end_char)
    }

    fn offset_to_position(
        content: &str,
        start_offset: usize,
        end_offset: usize,
    ) -> (usize, usize, usize) {
        let mut line = 0;
        let mut line_start = 0;

        for (i, ch) in content.char_indices() {
            if i >= start_offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                line_start = i + 1;
            }
        }

        let start_char = start_offset - line_start;
        let end_char = end_offset - line_start;

        (line, start_char, end_char)
    }
}

impl Default for KeyFinder {
    fn default() -> Self {
        Self::new(&default_function_names())
    }
}

fn default_function_names() -> Vec<String> {
    vec!["t".to_string(), "tt".to_string()]
}

fn is_simple_function_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    if !(first.is_ascii_alphabetic() || first == '_' || first == '$') {
        return false;
    }

    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_t_function() {
        let finder = KeyFinder::default();
        let content = r#"const msg = t("hello.world");"#;
        let keys = finder.find_keys(content);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "hello.world");
    }

    #[test]
    fn test_find_dollar_t() {
        let finder = KeyFinder::new(&["$t".to_string()]);
        let content = r#"const msg = $t("common.button");"#;
        let keys = finder.find_keys(content);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "common.button");
    }

    #[test]
    fn test_find_multiple_keys() {
        let finder = KeyFinder::default();
        let content = r#"
            const a = t("first.key");
            const b = t("second.key");
        "#;
        let keys = finder.find_keys(content);
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].key, "first.key");
        assert_eq!(keys[1].key, "second.key");
    }

    #[test]
    fn test_find_tt_function() {
        let finder = KeyFinder::default();
        let content = r#"const msg = tt("my.key");"#;
        let keys = finder.find_keys(content);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key, "my.key");
    }

    #[test]
    fn test_find_key_at_position() {
        let finder = KeyFinder::default();
        let content = r#"const msg = t("hello.world");"#;

        let found = finder.find_key_at_position(content, 0, 16);
        assert!(found.is_some());
        assert_eq!(found.unwrap().key, "hello.world");

        let not_found = finder.find_key_at_position(content, 0, 0);
        assert!(not_found.is_none());
    }

    #[test]
    fn test_should_not_match_other_methods() {
        let finder = KeyFinder::default();
        let test_cases = vec![
            r#"apiClient.post('/api/products')"#,
            r#"i18n.t('scoped.key')"#,
            r#"someObject.tt('scoped.key')"#,
        ];

        for content in test_cases {
            let keys = finder.find_keys(content);
            assert_eq!(
                keys.len(),
                0,
                "Should not match: {} but got {:?}",
                content,
                keys.iter().map(|k| &k.key).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn test_should_match_t_and_tt_but_not_object_calls() {
        let finder = KeyFinder::default();
        let content = r#"
            const msg = t("hello.world");
            const label = tt("profile.label");
            i18n.t("ignored.call");
        "#;
        let keys = finder.find_keys(content);
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].key, "hello.world");
        assert_eq!(keys[1].key, "profile.label");
    }
}
