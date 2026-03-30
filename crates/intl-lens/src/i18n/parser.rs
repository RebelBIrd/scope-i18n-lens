use std::collections::HashMap;

use anyhow::Result;
use serde_json::Value as JsonValue;

use crate::config::KeyStyle;

pub struct TranslationParser;

impl TranslationParser {
    pub fn parse_json_with_key_style(
        content: &str,
        key_style: KeyStyle,
    ) -> Result<HashMap<String, String>> {
        let value: JsonValue = serde_json::from_str(content)?;
        let mut result = HashMap::new();

        match key_style {
            KeyStyle::Nested => Self::flatten_json_nested(&value, String::new(), &mut result),
            KeyStyle::Flat => Self::flatten_json_flat(&value, &mut result),
        }

        Ok(result)
    }

    fn flatten_json_nested(
        value: &JsonValue,
        prefix: String,
        result: &mut HashMap<String, String>,
    ) {
        match value {
            JsonValue::Object(map) => {
                for (key, val) in map {
                    let new_key = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", prefix, key)
                    };
                    Self::flatten_json_nested(val, new_key, result);
                }
            }
            JsonValue::String(s) => {
                result.insert(prefix, s.clone());
            }
            JsonValue::Number(n) => {
                result.insert(prefix, n.to_string());
            }
            JsonValue::Bool(b) => {
                result.insert(prefix, b.to_string());
            }
            JsonValue::Array(arr) => {
                for (i, val) in arr.iter().enumerate() {
                    let new_key = format!("{}.{}", prefix, i);
                    Self::flatten_json_nested(val, new_key, result);
                }
            }
            JsonValue::Null => {}
        }
    }

    fn flatten_json_flat(value: &JsonValue, result: &mut HashMap<String, String>) {
        let JsonValue::Object(map) = value else {
            return;
        };

        for (key, val) in map {
            match val {
                JsonValue::String(s) => {
                    result.insert(key.clone(), s.clone());
                }
                JsonValue::Number(n) => {
                    result.insert(key.clone(), n.to_string());
                }
                JsonValue::Bool(b) => {
                    result.insert(key.clone(), b.to_string());
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_json_style() {
        let json = r#"{
          "common": {
            "title": "Title",
            "count": 3
          },
          "items": ["a", "b"]
        }"#;

        let result = TranslationParser::parse_json_with_key_style(json, KeyStyle::Nested).unwrap();
        assert_eq!(result.get("common.title"), Some(&"Title".to_string()));
        assert_eq!(result.get("common.count"), Some(&"3".to_string()));
        assert_eq!(result.get("items.0"), Some(&"a".to_string()));
    }

    #[test]
    fn parses_flat_json_style() {
        let json = r#"{
          "global_save": "Save",
          "global_cancel": "Cancel",
          "nested": { "ignore": true }
        }"#;

        let result = TranslationParser::parse_json_with_key_style(json, KeyStyle::Flat).unwrap();
        assert_eq!(result.get("global_save"), Some(&"Save".to_string()));
        assert_eq!(result.get("global_cancel"), Some(&"Cancel".to_string()));
        assert!(!result.contains_key("nested.ignore"));
    }

    #[test]
    fn returns_error_for_invalid_json() {
        let broken = r#"{ "a": 1,, }"#;
        let parsed = TranslationParser::parse_json_with_key_style(broken, KeyStyle::Flat);
        assert!(parsed.is_err());
    }
}
