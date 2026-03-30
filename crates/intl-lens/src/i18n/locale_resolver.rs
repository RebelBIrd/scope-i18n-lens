use std::path::{Path, PathBuf};

use dashmap::DashMap;

use crate::config::I18nConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleResolution {
    pub package_root: PathBuf,
    pub locale_dir: PathBuf,
}

pub struct LocaleResolver {
    config: I18nConfig,
    cache: DashMap<PathBuf, Option<LocaleResolution>>,
}

impl LocaleResolver {
    pub fn new(config: I18nConfig) -> Self {
        Self {
            config,
            cache: DashMap::new(),
        }
    }

    pub fn resolve_locale_dir(&self, file_path: &Path) -> Option<LocaleResolution> {
        let cache_key = file_path.to_path_buf();
        if let Some(cached) = self.cache.get(&cache_key) {
            return cached.clone();
        }

        let resolved = self.resolve_locale_dir_uncached(file_path);
        self.cache.insert(cache_key, resolved.clone());
        resolved
    }

    fn resolve_locale_dir_uncached(&self, file_path: &Path) -> Option<LocaleResolution> {
        let start_dir = match file_path.parent() {
            Some(parent) => parent.to_path_buf(),
            None => return None,
        };

        let package_root = self.find_package_root(&start_dir)?;
        let mut cursor = start_dir.as_path();
        let mut walked: usize = 0;

        loop {
            for locale_dir_name in &self.config.locale_dir_names {
                if locale_dir_name.trim().is_empty() {
                    continue;
                }

                let candidate = cursor.join(locale_dir_name);
                if self.is_usable_locale_dir(&candidate) {
                    return Some(LocaleResolution {
                        package_root: package_root.clone(),
                        locale_dir: candidate,
                    });
                }
            }

            if cursor == package_root {
                break;
            }

            walked += 1;
            if walked > self.config.max_walk_depth {
                break;
            }

            let Some(parent) = cursor.parent() else {
                break;
            };
            cursor = parent;
        }

        None
    }

    fn find_package_root(&self, start_dir: &Path) -> Option<PathBuf> {
        let mut cursor = start_dir;
        let mut walked: usize = 0;

        loop {
            if cursor.join("package.json").is_file() {
                return Some(cursor.to_path_buf());
            }

            if self.contains_monorepo_detector(cursor) {
                return None;
            }

            walked += 1;
            if walked > self.config.max_walk_depth {
                return None;
            }

            let Some(parent) = cursor.parent() else {
                return None;
            };
            cursor = parent;
        }
    }

    fn contains_monorepo_detector(&self, dir: &Path) -> bool {
        self.config
            .monorepo_detectors
            .iter()
            .any(|detector| dir.join(detector).exists())
    }

    fn is_usable_locale_dir(&self, dir: &Path) -> bool {
        if !dir.is_dir() {
            return false;
        }

        self.config
            .locales
            .iter()
            .any(|locale| dir.join(format!("{locale}.json")).is_file())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_test_root(name: &str) -> PathBuf {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("scope_i18n_lens_{name}_{pid}_{nanos}"))
    }

    fn create_file(path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent dir");
        }
        std::fs::write(path, "{}").expect("write file");
    }

    fn create_dir(path: &Path) {
        std::fs::create_dir_all(path).expect("create dir");
    }

    fn cleanup_dir(path: &Path) {
        if path.exists() {
            let _ = std::fs::remove_dir_all(path);
        }
    }

    #[test]
    fn resolves_locales_within_nearest_package_boundary() {
        let root = unique_test_root("resolver_basic");
        let app_root = root.join("apps/crm-next");
        let finance_root = root.join("apps/finance-next");
        create_file(&app_root.join("package.json"));
        create_file(&finance_root.join("package.json"));
        create_file(&app_root.join("src/locales/en.json"));
        create_file(&finance_root.join("src/locales/en.json"));
        create_file(&app_root.join("src/pages/home.tsx"));

        let config = I18nConfig::default();
        let resolver = LocaleResolver::new(config);
        let resolution = resolver
            .resolve_locale_dir(&app_root.join("src/pages/home.tsx"))
            .expect("resolution should exist");

        assert_eq!(resolution.package_root, app_root);
        assert_eq!(
            resolution.locale_dir,
            root.join("apps/crm-next/src/locales")
        );

        cleanup_dir(&root);
    }

    #[test]
    fn stops_search_when_monorepo_root_is_hit_without_package_json() {
        let root = unique_test_root("resolver_monorepo_stop");
        create_file(&root.join("yarn.lock"));
        create_file(&root.join("src/locales/en.json"));
        create_file(&root.join("src/pages/home.tsx"));

        let resolver = LocaleResolver::new(I18nConfig::default());
        let resolution = resolver.resolve_locale_dir(&root.join("src/pages/home.tsx"));
        assert!(resolution.is_none());

        cleanup_dir(&root);
    }

    #[test]
    fn only_accepts_locale_dirs_with_configured_locale_files() {
        let root = unique_test_root("resolver_locale_file_check");
        let app_root = root.join("apps/app-a");
        create_file(&app_root.join("package.json"));
        create_dir(&app_root.join("src/locales"));
        create_file(&app_root.join("src/locales/ja.json"));
        create_file(&app_root.join("src/pages/home.tsx"));

        let resolver = LocaleResolver::new(I18nConfig::default());
        let resolution = resolver.resolve_locale_dir(&app_root.join("src/pages/home.tsx"));
        assert!(resolution.is_none());

        cleanup_dir(&root);
    }
}
