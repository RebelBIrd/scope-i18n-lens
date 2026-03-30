use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use dashmap::DashMap;

use crate::config::KeyStyle;

use super::parser::TranslationParser;

#[derive(Debug, Clone)]
pub struct TranslationEntry {
    pub value: String,
    pub file_path: PathBuf,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct TranslationLocation {
    pub file_path: PathBuf,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Default)]
struct PackageLocaleStore {
    locales: HashMap<String, HashMap<String, TranslationEntry>>,
}

pub struct TranslationStore {
    packages: DashMap<PathBuf, PackageLocaleStore>,
}

impl TranslationStore {
    pub fn new() -> Self {
        Self {
            packages: DashMap::new(),
        }
    }

    pub fn load_locale_dir(
        &self,
        locale_dir: &Path,
        configured_locales: &[String],
        key_style: KeyStyle,
    ) {
        let normalized = locale_dir.to_path_buf();
        let package_store = self.build_package_store(locale_dir, configured_locales, key_style);
        self.packages.insert(normalized, package_store);
    }

    pub fn reload_locale_dir(
        &self,
        locale_dir: &Path,
        configured_locales: &[String],
        key_style: KeyStyle,
    ) {
        self.load_locale_dir(locale_dir, configured_locales, key_style);
    }

    pub fn reload_for_changed_file(
        &self,
        changed_file: &Path,
        configured_locales: &[String],
        key_style: KeyStyle,
    ) -> bool {
        let mut affected_dirs = Vec::new();

        for entry in self.packages.iter() {
            let locale_dir = entry.key();
            if changed_file.starts_with(locale_dir) {
                affected_dirs.push(locale_dir.clone());
            }
        }

        if affected_dirs.is_empty() {
            return false;
        }

        for locale_dir in affected_dirs {
            self.reload_locale_dir(&locale_dir, configured_locales, key_style);
        }

        true
    }

    pub fn get_loaded_locale_dirs(&self) -> Vec<PathBuf> {
        self.packages
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    pub fn is_locale_dir_loaded(&self, locale_dir: &Path) -> bool {
        self.packages.contains_key(locale_dir)
    }

    pub fn get_translation(&self, locale_dir: &Path, key: &str, locale: &str) -> Option<String> {
        let package = self.packages.get(locale_dir)?;
        let locale_map = package.locales.get(locale)?;
        let entry = locale_map.get(key)?;
        Some(entry.value.clone())
    }

    pub fn get_all_translations(
        &self,
        locale_dir: &Path,
        key: &str,
    ) -> HashMap<String, TranslationEntry> {
        let Some(package) = self.packages.get(locale_dir) else {
            return HashMap::new();
        };

        let mut result = HashMap::new();
        for (locale, locale_map) in &package.locales {
            if let Some(entry) = locale_map.get(key) {
                result.insert(locale.clone(), entry.clone());
            }
        }
        result
    }

    pub fn get_translation_location(
        &self,
        locale_dir: &Path,
        key: &str,
        locale: &str,
    ) -> Option<TranslationLocation> {
        let package = self.packages.get(locale_dir)?;
        let locale_map = package.locales.get(locale)?;
        let entry = locale_map.get(key)?;

        Some(TranslationLocation {
            file_path: entry.file_path.clone(),
            line: entry.line,
            column: entry.column,
        })
    }

    pub fn get_all_keys(&self, locale_dir: &Path) -> Vec<String> {
        let Some(package) = self.packages.get(locale_dir) else {
            return Vec::new();
        };

        let mut keys = HashSet::new();
        for locale_map in package.locales.values() {
            for key in locale_map.keys() {
                keys.insert(key.clone());
            }
        }
        keys.into_iter().collect()
    }

    pub fn key_exists(&self, locale_dir: &Path, key: &str) -> bool {
        self.packages.get(locale_dir).is_some_and(|package| {
            package
                .locales
                .values()
                .any(|locale_map| locale_map.contains_key(key))
        })
    }

    pub fn get_missing_locales(
        &self,
        locale_dir: &Path,
        key: &str,
        configured_locales: &[String],
    ) -> Vec<String> {
        let Some(package) = self.packages.get(locale_dir) else {
            return configured_locales.to_vec();
        };

        configured_locales
            .iter()
            .filter(|locale| match package.locales.get(*locale) {
                Some(locale_map) => !locale_map.contains_key(key),
                None => true,
            })
            .cloned()
            .collect()
    }

    fn build_package_store(
        &self,
        locale_dir: &Path,
        configured_locales: &[String],
        key_style: KeyStyle,
    ) -> PackageLocaleStore {
        let mut package_store = PackageLocaleStore::default();

        for locale in configured_locales {
            let file_path = locale_dir.join(format!("{locale}.json"));
            if !file_path.is_file() {
                continue;
            }

            let Ok(file_content) = std::fs::read_to_string(&file_path) else {
                tracing::warn!("Failed to read {:?}", file_path);
                continue;
            };

            let Ok(translations) =
                TranslationParser::parse_json_with_key_style(&file_content, key_style)
            else {
                tracing::warn!("Failed to parse locale file {:?}", file_path);
                continue;
            };

            let locale_map = package_store.locales.entry(locale.clone()).or_default();
            for (key, value) in translations {
                let (line, column) = find_key_position(&file_content, &key).unwrap_or((0, 0));
                locale_map.insert(
                    key,
                    TranslationEntry {
                        value,
                        file_path: file_path.clone(),
                        line,
                        column,
                    },
                );
            }
        }

        package_store
    }
}

fn find_key_position(content: &str, key: &str) -> Option<(usize, usize)> {
    let key_tail = key.split('.').next_back().unwrap_or(key);
    let candidates = [key, key_tail];

    for candidate in candidates {
        let double_quote_pattern = format!("\"{candidate}\"");
        if let Some((line, column)) = find_pattern_position(content, &double_quote_pattern) {
            return Some((line, column + 1));
        }

        let single_quote_pattern = format!("'{candidate}'");
        if let Some((line, column)) = find_pattern_position(content, &single_quote_pattern) {
            return Some((line, column + 1));
        }
    }

    None
}

fn find_pattern_position(content: &str, pattern: &str) -> Option<(usize, usize)> {
    for (line_index, line) in content.lines().enumerate() {
        if let Some(column) = line.find(pattern) {
            return Some((line_index, column));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::KeyStyle;

    fn unique_test_root(name: &str) -> PathBuf {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("scope_i18n_store_{name}_{pid}_{nanos}"))
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent dir");
        }
        std::fs::write(path, content).expect("write file");
    }

    fn cleanup_dir(path: &Path) {
        if path.exists() {
            let _ = std::fs::remove_dir_all(path);
        }
    }

    #[test]
    fn isolates_keys_by_locale_dir() {
        let root = unique_test_root("isolation");
        let crm_locale_dir = root.join("apps/crm-next/src/locales");
        let finance_locale_dir = root.join("apps/finance-next/src/locales");

        write_file(
            &crm_locale_dir.join("en.json"),
            r#"{"global_cancel":"Cancel","crm_only":"CRM"}"#,
        );
        write_file(
            &finance_locale_dir.join("en.json"),
            r#"{"global_cancel":"Abort","finance_only":"Finance"}"#,
        );

        let store = TranslationStore::new();
        let locales = vec!["en".to_string()];
        store.load_locale_dir(&crm_locale_dir, &locales, KeyStyle::Flat);
        store.load_locale_dir(&finance_locale_dir, &locales, KeyStyle::Flat);

        assert_eq!(
            store.get_translation(&crm_locale_dir, "crm_only", "en"),
            Some("CRM".to_string())
        );
        assert_eq!(
            store.get_translation(&finance_locale_dir, "finance_only", "en"),
            Some("Finance".to_string())
        );
        assert_eq!(
            store.get_translation(&crm_locale_dir, "finance_only", "en"),
            None
        );

        cleanup_dir(&root);
    }

    #[test]
    fn stores_line_and_column_for_keys() {
        let root = unique_test_root("location");
        let locale_dir = root.join("apps/crm-next/src/locales");
        write_file(
            &locale_dir.join("en.json"),
            "{\n  \"global_cancel\": \"Cancel\",\n  \"name\": \"Name\"\n}\n",
        );

        let store = TranslationStore::new();
        let locales = vec!["en".to_string()];
        store.load_locale_dir(&locale_dir, &locales, KeyStyle::Flat);

        let location = store
            .get_translation_location(&locale_dir, "name", "en")
            .expect("location should exist");
        assert_eq!(location.line, 2);
        assert!(location.column > 0);

        cleanup_dir(&root);
    }
}
