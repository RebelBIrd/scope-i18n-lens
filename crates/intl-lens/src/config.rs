use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct I18nConfig {
    #[serde(
        default = "default_locale_dir_names",
        alias = "localePaths",
        alias = "locale_paths"
    )]
    pub locale_dir_names: Vec<String>,

    #[serde(default = "default_locales")]
    pub locales: Vec<String>,

    #[serde(default = "default_source_locale")]
    pub source_locale: String,

    #[serde(default = "default_display_locale")]
    pub display_locale: String,

    #[serde(default = "default_key_style")]
    pub key_style: KeyStyle,

    #[serde(default = "default_function_names", alias = "functionPatterns")]
    pub function_names: Vec<String>,

    #[serde(default = "default_monorepo_detectors")]
    pub monorepo_detectors: Vec<String>,

    #[serde(default = "default_max_walk_depth")]
    pub max_walk_depth: usize,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum KeyStyle {
    #[default]
    Flat,
    Nested,
}

impl Default for I18nConfig {
    fn default() -> Self {
        Self {
            locale_dir_names: default_locale_dir_names(),
            locales: default_locales(),
            source_locale: default_source_locale(),
            display_locale: default_display_locale(),
            key_style: default_key_style(),
            function_names: default_function_names(),
            monorepo_detectors: default_monorepo_detectors(),
            max_walk_depth: default_max_walk_depth(),
        }
    }
}

impl I18nConfig {
    pub fn load_from_workspace(root: &Path) -> Self {
        let config_path = root.join(".zed/i18n.json");
        let Ok(content) = std::fs::read_to_string(&config_path) else {
            tracing::info!("Using default config");
            return Self::default();
        };

        match Self::parse_config_content(&content) {
            Ok(config) => {
                tracing::info!("Loaded config from {:?}", config_path);
                config.with_sanitized_values()
            }
            Err(err) => {
                tracing::warn!(
                    "Failed to parse i18n config at {:?}: {}. Using defaults.",
                    config_path,
                    err
                );
                Self::default()
            }
        }
    }

    fn parse_config_content(content: &str) -> Result<I18nConfig, String> {
        match serde_json::from_str::<I18nConfig>(content) {
            Ok(config) => Ok(config),
            Err(json_err) => match json5::from_str::<I18nConfig>(content) {
                Ok(config) => Ok(config),
                Err(json5_err) => Err(format!(
                    "json parse failed: {}; json5 parse failed: {}",
                    json_err, json5_err
                )),
            },
        }
    }

    fn with_sanitized_values(mut self) -> Self {
        if self.locale_dir_names.is_empty() {
            self.locale_dir_names = default_locale_dir_names();
        }

        if self.locales.is_empty() {
            self.locales = default_locales();
        }

        if self.source_locale.trim().is_empty() {
            self.source_locale = default_source_locale();
        }

        if self.display_locale.trim().is_empty() {
            self.display_locale = self.source_locale.clone();
        }

        if self.function_names.is_empty() {
            self.function_names = default_function_names();
        }

        if self.monorepo_detectors.is_empty() {
            self.monorepo_detectors = default_monorepo_detectors();
        }

        if self.max_walk_depth == 0 {
            self.max_walk_depth = default_max_walk_depth();
        }

        self
    }
}

fn default_locale_dir_names() -> Vec<String> {
    vec!["locales".to_string()]
}

fn default_locales() -> Vec<String> {
    vec!["zh-CN".to_string(), "zh-HK".to_string(), "en".to_string()]
}

fn default_source_locale() -> String {
    "en".to_string()
}

fn default_display_locale() -> String {
    "en".to_string()
}

fn default_key_style() -> KeyStyle {
    KeyStyle::Flat
}

fn default_function_names() -> Vec<String> {
    vec!["t".to_string(), "tt".to_string()]
}

fn default_monorepo_detectors() -> Vec<String> {
    vec![
        "yarn.lock".to_string(),
        "pnpm-workspace.yaml".to_string(),
        "lerna.json".to_string(),
    ]
}

fn default_max_walk_depth() -> usize {
    10
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_test_root(name: &str) -> std::path::PathBuf {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("scope_i18n_config_{name}_{pid}_{nanos}"))
    }

    #[test]
    fn defaults_match_design_spec() {
        let config = I18nConfig::default();
        assert_eq!(config.locale_dir_names, vec!["locales"]);
        assert_eq!(config.locales, vec!["zh-CN", "zh-HK", "en"]);
        assert_eq!(config.source_locale, "en");
        assert_eq!(config.display_locale, "en");
        assert_eq!(config.key_style, KeyStyle::Flat);
        assert_eq!(config.function_names, vec!["t", "tt"]);
        assert_eq!(
            config.monorepo_detectors,
            vec!["yarn.lock", "pnpm-workspace.yaml", "lerna.json"]
        );
        assert_eq!(config.max_walk_depth, 10);
    }

    #[test]
    fn reads_zed_config_and_applies_values() {
        let root = unique_test_root("load");
        let config_dir = root.join(".zed");
        std::fs::create_dir_all(&config_dir).expect("create .zed dir");
        std::fs::write(
            config_dir.join("i18n.json"),
            r#"{
  "localeDirNames": ["src/locales", "locales"],
  "locales": ["zh-CN", "en"],
  "sourceLocale": "zh-CN",
  "displayLocale": "zh-CN",
  "keyStyle": "nested",
  "functionNames": ["t", "tt", "i18nT"],
  "monorepoDetectors": ["yarn.lock"],
  "maxWalkDepth": 3
}"#,
        )
        .expect("write config");

        let config = I18nConfig::load_from_workspace(&root);
        assert_eq!(config.locale_dir_names, vec!["src/locales", "locales"]);
        assert_eq!(config.locales, vec!["zh-CN", "en"]);
        assert_eq!(config.source_locale, "zh-CN");
        assert_eq!(config.display_locale, "zh-CN");
        assert_eq!(config.key_style, KeyStyle::Nested);
        assert_eq!(config.function_names, vec!["t", "tt", "i18nT"]);
        assert_eq!(config.monorepo_detectors, vec!["yarn.lock"]);
        assert_eq!(config.max_walk_depth, 3);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn falls_back_to_defaults_when_config_missing() {
        let root = unique_test_root("missing");
        std::fs::create_dir_all(&root).expect("create test root");
        let config = I18nConfig::load_from_workspace(&root);
        assert_eq!(config.locale_dir_names, vec!["locales"]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn reads_jsonc_style_config_with_trailing_comma() {
        let root = unique_test_root("jsonc");
        let config_dir = root.join(".zed");
        std::fs::create_dir_all(&config_dir).expect("create .zed dir");
        std::fs::write(
            config_dir.join("i18n.json"),
            r#"{
  // jsonc style comment
  "localeDirNames": ["src/locales", "locales"],
  "locales": ["zh-CN", "zh-HK", "en"],
  "sourceLocale": "zh-HK",
  "displayLocale": "zh-HK",
}"#,
        )
        .expect("write config");

        let config = I18nConfig::load_from_workspace(&root);
        assert_eq!(config.source_locale, "zh-HK");
        assert_eq!(config.display_locale, "zh-HK");
        assert_eq!(config.locale_dir_names, vec!["src/locales", "locales"]);

        let _ = std::fs::remove_dir_all(root);
    }
}
