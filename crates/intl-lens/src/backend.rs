use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use regex::Regex;
use tokio::sync::RwLock;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::config::I18nConfig;
use crate::document::DocumentStore;
use crate::i18n::{KeyFinder, LocaleResolver, TranslationStore};

fn truncate_string(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }

    let truncated: String = s.chars().take(max_chars.saturating_sub(3)).collect();
    format!("{}...", truncated)
}

pub struct I18nBackend {
    client: Client,
    config: Arc<RwLock<I18nConfig>>,
    documents: Arc<RwLock<DocumentStore>>,
    translation_store: Arc<RwLock<Option<TranslationStore>>>,
    locale_resolver: Arc<RwLock<Option<LocaleResolver>>>,
    key_finder: Arc<RwLock<KeyFinder>>,
    workspace_root: Arc<RwLock<Option<PathBuf>>>,
    inlay_hint_dynamic_registration_supported: Arc<RwLock<bool>>,
    inlay_hint_refresh_supported: Arc<RwLock<bool>>,
    watched_files_dynamic_registration_supported: Arc<RwLock<bool>>,
    watched_files_relative_pattern_supported: Arc<RwLock<bool>>,
}

impl I18nBackend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            config: Arc::new(RwLock::new(I18nConfig::default())),
            documents: Arc::new(RwLock::new(DocumentStore::new())),
            translation_store: Arc::new(RwLock::new(None)),
            locale_resolver: Arc::new(RwLock::new(None)),
            key_finder: Arc::new(RwLock::new(KeyFinder::default())),
            workspace_root: Arc::new(RwLock::new(None)),
            inlay_hint_dynamic_registration_supported: Arc::new(RwLock::new(false)),
            inlay_hint_refresh_supported: Arc::new(RwLock::new(false)),
            watched_files_dynamic_registration_supported: Arc::new(RwLock::new(false)),
            watched_files_relative_pattern_supported: Arc::new(RwLock::new(false)),
        }
    }

    async fn initialize_workspace(&self, root: PathBuf) {
        tracing::info!("Initializing workspace at {:?}", root);

        let config = I18nConfig::load_from_workspace(&root);
        let key_finder = KeyFinder::new(&config.function_names);
        let store = TranslationStore::new();
        let resolver = LocaleResolver::new(config.clone());

        *self.key_finder.write().await = key_finder;
        *self.translation_store.write().await = Some(store);
        *self.locale_resolver.write().await = Some(resolver);
        *self.config.write().await = config.clone();
        *self.workspace_root.write().await = Some(root.clone());

        self.client
            .log_message(
                MessageType::INFO,
                format!(
                    "scope-i18n-lens initialized: localeDirs={:?}, locales={:?}",
                    config.locale_dir_names, config.locales
                ),
            )
            .await;
    }

    async fn register_inlay_hint_capability(&self) {
        let supports_dynamic = *self.inlay_hint_dynamic_registration_supported.read().await;

        if !supports_dynamic {
            tracing::debug!("Skipping inlay hint dynamic registration (dynamicRegistration=false)");
            return;
        }

        let document_selector = Some(vec![
            DocumentFilter {
                language: Some("typescript".to_string()),
                scheme: None,
                pattern: None,
            },
            DocumentFilter {
                language: Some("typescriptreact".to_string()),
                scheme: None,
                pattern: None,
            },
            DocumentFilter {
                language: Some("javascript".to_string()),
                scheme: None,
                pattern: None,
            },
            DocumentFilter {
                language: Some("javascriptreact".to_string()),
                scheme: None,
                pattern: None,
            },
        ]);

        let register_options = InlayHintRegistrationOptions {
            inlay_hint_options: InlayHintOptions {
                resolve_provider: Some(false),
                work_done_progress_options: Default::default(),
            },
            text_document_registration_options: TextDocumentRegistrationOptions {
                document_selector,
            },
            static_registration_options: StaticRegistrationOptions {
                id: Some("scope-i18n-lens-inlay-hint".to_string()),
            },
        };

        let register_options = match serde_json::to_value(register_options) {
            Ok(value) => value,
            Err(err) => {
                tracing::warn!(
                    "Failed to serialize inlay hint registration options: {:?}",
                    err
                );
                return;
            }
        };

        let registration = Registration {
            id: "scope-i18n-lens-inlay-hint".to_string(),
            method: "textDocument/inlayHint".to_string(),
            register_options: Some(register_options),
        };

        match self.client.register_capability(vec![registration]).await {
            Ok(_) => tracing::info!("Registered inlay hint capability dynamically"),
            Err(err) => tracing::warn!("Dynamic inlay hint registration failed: {:?}", err),
        }
    }

    async fn register_watched_files_capability(&self) {
        let supports_dynamic = *self
            .watched_files_dynamic_registration_supported
            .read()
            .await;

        if !supports_dynamic {
            tracing::debug!("Skipping watched files registration (dynamicRegistration=false)");
            return;
        }

        let workspace_root = { self.workspace_root.read().await.clone() };
        let relative_pattern_support = *self.watched_files_relative_pattern_supported.read().await;
        let watchers =
            Self::build_file_watchers(workspace_root.as_deref(), relative_pattern_support);

        let register_options = DidChangeWatchedFilesRegistrationOptions { watchers };
        let register_options = match serde_json::to_value(register_options) {
            Ok(value) => value,
            Err(err) => {
                tracing::warn!(
                    "Failed to serialize watched files registration options: {:?}",
                    err
                );
                return;
            }
        };

        let registration = Registration {
            id: "scope-i18n-lens-watched-files".to_string(),
            method: "workspace/didChangeWatchedFiles".to_string(),
            register_options: Some(register_options),
        };

        match self.client.register_capability(vec![registration]).await {
            Ok(_) => tracing::info!("Registered watched files capability dynamically"),
            Err(err) => tracing::warn!("Dynamic watched files registration failed: {:?}", err),
        }
    }

    async fn ensure_locale_dir_for_uri(&self, uri: &Url) -> Option<PathBuf> {
        let file_path = uri.to_file_path().ok()?;

        let resolution = {
            let mut resolver_guard = self.locale_resolver.write().await;
            let resolver = resolver_guard.as_mut()?;
            resolver.resolve_locale_dir(&file_path)
        }?;

        let config = self.config.read().await.clone();

        {
            let store_guard = self.translation_store.read().await;
            let store = store_guard.as_ref()?;

            if !store.is_locale_dir_loaded(&resolution.locale_dir) {
                store.load_locale_dir(&resolution.locale_dir, &config.locales, config.key_style);
            }
        }

        Some(resolution.locale_dir)
    }

    async fn diagnose_document(&self, uri: &Url, content: &str) {
        let diagnostics = self.compute_diagnostics(uri, content).await;

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }

    async fn compute_diagnostics(&self, uri: &Url, content: &str) -> Vec<Diagnostic> {
        let locale_dir = match self.ensure_locale_dir_for_uri(uri).await {
            Some(locale_dir) => locale_dir,
            None => return vec![],
        };

        let key_finder = self.key_finder.read().await;
        let found_keys = key_finder.find_keys(content);

        let translation_store = self.translation_store.read().await;
        let config = self.config.read().await;

        let Some(store) = translation_store.as_ref() else {
            return vec![];
        };

        let mut diagnostics = Vec::new();

        for found_key in found_keys {
            let range = Range {
                start: Position {
                    line: found_key.line as u32,
                    character: found_key.start_char as u32,
                },
                end: Position {
                    line: found_key.line as u32,
                    character: found_key.end_char as u32,
                },
            };

            if !store.key_exists(&locale_dir, &found_key.key) {
                diagnostics.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(NumberOrString::String("missing-translation".to_string())),
                    source: Some("scope-i18n-lens".to_string()),
                    message: format!(
                        "Translation key '{}' not found in current package locales",
                        found_key.key
                    ),
                    ..Default::default()
                });
                continue;
            }

            let missing_locales =
                store.get_missing_locales(&locale_dir, &found_key.key, &config.locales);
            if !missing_locales.is_empty() {
                diagnostics.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: Some(NumberOrString::String("incomplete-translation".to_string())),
                    source: Some("scope-i18n-lens".to_string()),
                    message: format!(
                        "Translation '{}' missing in: {}",
                        found_key.key,
                        missing_locales.join(", ")
                    ),
                    ..Default::default()
                });
            }
        }

        diagnostics
    }

    async fn get_hover_content(&self, locale_dir: &Path, key: &str) -> Option<String> {
        let translation_store = self.translation_store.read().await;
        let config = self.config.read().await;
        let store = translation_store.as_ref()?;

        let translations = store.get_all_translations(locale_dir, key);
        if translations.is_empty() {
            return None;
        }

        let mut content = String::new();
        content.push_str(key);
        content.push_str("\n\n");

        for locale in &config.locales {
            let value = translations
                .get(locale)
                .map(|entry| entry.value.clone())
                .unwrap_or_else(|| "missing".to_string());
            content.push_str(&format!("{}: {}\n", locale, value));
        }

        if let Some(source_location) =
            store.get_translation_location(locale_dir, key, &config.source_locale)
        {
            content.push('\n');
            content.push_str(&format!("Path: {}\n", source_location.file_path.display()));
        }

        let placeholders =
            Self::collect_placeholders(translations.values().map(|entry| entry.value.as_str()));
        if !placeholders.is_empty() {
            content.push('\n');
            content.push_str(&format!("Placeholders: {}\n", placeholders.join(", ")));
        }

        Some(content)
    }

    async fn get_completions(&self, locale_dir: &Path, prefix: &str) -> Vec<CompletionItem> {
        let translation_store = self.translation_store.read().await;
        let config = self.config.read().await;

        let Some(store) = translation_store.as_ref() else {
            return vec![];
        };

        let display_locale = &config.display_locale;
        let mut keys = store.get_all_keys(locale_dir);
        keys.sort();

        let mut ranked: Vec<(u8, String)> = keys
            .into_iter()
            .filter_map(|key| Self::score_key_match(&key, prefix).map(|score| (score, key)))
            .collect();

        ranked.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        ranked
            .into_iter()
            .take(100)
            .map(|(_, key)| {
                let translation = store.get_translation(locale_dir, &key, display_locale);
                CompletionItem {
                    label: key.clone(),
                    kind: Some(CompletionItemKind::TEXT),
                    detail: translation.clone(),
                    documentation: translation.map(|t| {
                        Documentation::MarkupContent(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: format!("**{}**: {}", display_locale, t),
                        })
                    }),
                    insert_text: Some(key),
                    ..Default::default()
                }
            })
            .collect()
    }

    async fn get_definition_location(&self, locale_dir: &Path, key: &str) -> Option<Location> {
        let translation_store = self.translation_store.read().await;
        let config = self.config.read().await;
        let store = translation_store.as_ref()?;

        let location = store.get_translation_location(locale_dir, key, &config.source_locale)?;
        let uri = Url::from_file_path(&location.file_path).ok()?;

        Some(Location {
            uri,
            range: Range {
                start: Position {
                    line: location.line as u32,
                    character: location.column as u32,
                },
                end: Position {
                    line: location.line as u32,
                    character: location.column as u32,
                },
            },
        })
    }

    async fn is_translation_uri(&self, uri: &Url) -> bool {
        let Some(path) = uri.to_file_path().ok() else {
            return false;
        };

        if !Self::has_translation_extension(&path) {
            return false;
        }

        let store_guard = self.translation_store.read().await;
        let Some(store) = store_guard.as_ref() else {
            return false;
        };

        store
            .get_loaded_locale_dirs()
            .into_iter()
            .any(|locale_dir| path.starts_with(locale_dir))
    }

    async fn reload_changed_files(&self, params: &DidChangeWatchedFilesParams) {
        let config = self.config.read().await.clone();
        let store_guard = self.translation_store.read().await;
        let Some(store) = store_guard.as_ref() else {
            return;
        };

        let mut reloaded = false;

        for change in &params.changes {
            let Some(path) = change.uri.to_file_path().ok() else {
                continue;
            };

            if !Self::has_translation_extension(&path) {
                continue;
            }

            if store.reload_for_changed_file(&path, &config.locales, config.key_style) {
                reloaded = true;
            }
        }

        drop(store_guard);

        if reloaded {
            self.refresh_inlay_hints().await;
            self.rediagnose_open_documents().await;
        }
    }

    async fn rediagnose_open_documents(&self) {
        let docs = self.documents.read().await;
        let snapshot = docs.snapshot();
        drop(docs);

        for (uri, content) in snapshot {
            let Ok(uri) = Url::parse(&uri) else {
                continue;
            };
            self.diagnose_document(&uri, &content).await;
        }
    }

    async fn refresh_inlay_hints(&self) {
        if *self.inlay_hint_refresh_supported.read().await {
            if let Err(err) = self.client.inlay_hint_refresh().await {
                tracing::warn!("Inlay hint refresh failed: {:?}", err);
            }
        }
    }

    fn build_file_watchers(
        workspace_root: Option<&Path>,
        relative_pattern_support: bool,
    ) -> Vec<FileSystemWatcher> {
        let pattern = "**/*.json".to_string();

        let glob_pattern = if relative_pattern_support {
            if let Some(root) = workspace_root {
                if let Ok(base_uri) = Url::from_directory_path(root) {
                    GlobPattern::Relative(RelativePattern {
                        base_uri: OneOf::Right(base_uri),
                        pattern,
                    })
                } else {
                    GlobPattern::String("**/*.json".to_string())
                }
            } else {
                GlobPattern::String("**/*.json".to_string())
            }
        } else if let Some(root) = workspace_root {
            GlobPattern::String(Self::to_absolute_pattern(root, "**/*.json"))
        } else {
            GlobPattern::String("**/*.json".to_string())
        };

        vec![FileSystemWatcher {
            glob_pattern,
            kind: None,
        }]
    }

    fn to_absolute_pattern(root: &Path, pattern: &str) -> String {
        let mut root_str = root.to_string_lossy().replace('\\', "/");
        root_str = root_str.trim_end_matches('/').to_string();

        if pattern.starts_with('/') {
            format!("{}{}", root_str, pattern)
        } else {
            format!("{}/{}", root_str, pattern)
        }
    }

    fn has_translation_extension(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
    }

    fn extract_completion_prefix(
        line: &str,
        character: usize,
        function_names: &[String],
    ) -> Option<String> {
        let before_cursor = &line[..character.min(line.len())];
        let mut best_match: Option<(usize, String)> = None;

        for function_name in function_names {
            for quote in ['"', '\''] {
                let marker = format!("{}({}", function_name, quote);
                let mut search_start = before_cursor.len();

                while search_start > 0 {
                    let Some(pos) = before_cursor[..search_start].rfind(&marker) else {
                        break;
                    };

                    let is_member_call =
                        before_cursor[..pos].chars().next_back().is_some_and(|ch| {
                            ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '$'
                        });
                    if is_member_call {
                        search_start = pos;
                        continue;
                    }

                    let after_quote = pos + marker.len();
                    let prefix = &before_cursor[after_quote..];
                    if !prefix.contains(quote) {
                        let candidate = (pos, prefix.to_string());
                        let should_replace = match best_match.as_ref() {
                            Some(current) => candidate.0 > current.0,
                            None => true,
                        };
                        if should_replace {
                            best_match = Some(candidate);
                        }
                    }

                    break;
                }
            }
        }

        best_match.map(|(_, prefix)| prefix)
    }

    fn score_key_match(key: &str, prefix: &str) -> Option<u8> {
        if prefix.is_empty() {
            return Some(0);
        }

        let key_lower = key.to_ascii_lowercase();
        let prefix_lower = prefix.to_ascii_lowercase();

        if key_lower.starts_with(&prefix_lower) {
            return Some(0);
        }

        if key_lower.contains(&prefix_lower) {
            return Some(1);
        }

        if Self::is_subsequence(&prefix_lower, &key_lower) {
            return Some(2);
        }

        None
    }

    fn is_subsequence(needle: &str, haystack: &str) -> bool {
        let mut needle_chars = needle.chars();
        let mut current = needle_chars.next();

        if current.is_none() {
            return true;
        }

        for ch in haystack.chars() {
            if Some(ch) == current {
                current = needle_chars.next();
                if current.is_none() {
                    return true;
                }
            }
        }

        false
    }

    fn collect_placeholders<'a>(values: impl Iterator<Item = &'a str>) -> Vec<String> {
        let regex = Regex::new(r"\{[A-Za-z0-9_]+\}").expect("placeholder regex should compile");
        let mut placeholders = BTreeSet::new();

        for value in values {
            for capture in regex.find_iter(value) {
                placeholders.insert(capture.as_str().to_string());
            }
        }

        placeholders.into_iter().collect()
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for I18nBackend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        tracing::info!("scope-i18n-lens initialize called");
        tracing::debug!("Client capabilities: {:?}", params.capabilities);

        let inlay_hint_dynamic_registration_support = params
            .capabilities
            .text_document
            .as_ref()
            .and_then(|text_document| text_document.inlay_hint.as_ref())
            .and_then(|inlay| inlay.dynamic_registration)
            .unwrap_or(false);

        *self.inlay_hint_dynamic_registration_supported.write().await =
            inlay_hint_dynamic_registration_support;

        let inlay_hint_refresh_supported = params
            .capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.inlay_hint.as_ref())
            .and_then(|inlay| inlay.refresh_support)
            .unwrap_or(false);

        *self.inlay_hint_refresh_supported.write().await = inlay_hint_refresh_supported;

        let watched_files = params
            .capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.did_change_watched_files.as_ref());

        let watched_files_dynamic_registration_support = watched_files
            .and_then(|watch| watch.dynamic_registration)
            .unwrap_or(false);

        let watched_files_relative_pattern_support = watched_files
            .and_then(|watch| watch.relative_pattern_support)
            .unwrap_or(false);

        *self
            .watched_files_dynamic_registration_supported
            .write()
            .await = watched_files_dynamic_registration_support;
        *self.watched_files_relative_pattern_supported.write().await =
            watched_files_relative_pattern_support;

        let root_path = params
            .workspace_folders
            .as_ref()
            .and_then(|folders| folders.first())
            .and_then(|folder| folder.uri.to_file_path().ok())
            .or_else(|| {
                params
                    .root_uri
                    .as_ref()
                    .and_then(|uri| uri.to_file_path().ok())
            });

        if let Some(root) = root_path {
            self.initialize_workspace(root).await;
        } else {
            tracing::warn!("No workspace root found in initialize params");
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        will_save: None,
                        will_save_wait_until: None,
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(false),
                        })),
                    },
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec!["\"".to_string(), "'".to_string()]),
                    ..Default::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
                inlay_hint_provider: Some(OneOf::Right(InlayHintServerCapabilities::Options(
                    InlayHintOptions {
                        resolve_provider: Some(false),
                        work_done_progress_options: Default::default(),
                    },
                ))),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "scope-i18n-lens".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "scope-i18n-lens server initialized")
            .await;
        self.register_inlay_hint_capability().await;
        self.register_watched_files_capability().await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        self.reload_changed_files(&params).await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let content = params.text_document.text.clone();
        let version = params.text_document.version;

        {
            let mut docs = self.documents.write().await;
            docs.open(uri.to_string(), content.clone(), version);
        }

        self.diagnose_document(&uri, &content).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();

        if let Some(change) = params.content_changes.into_iter().next_back() {
            let content = change.text;
            let version = params.text_document.version;

            {
                let mut docs = self.documents.write().await;
                docs.update(uri.as_str(), content.clone(), version);
            }

            self.diagnose_document(&uri, &content).await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        if self.is_translation_uri(&params.text_document.uri).await {
            let params = DidChangeWatchedFilesParams {
                changes: vec![FileEvent {
                    uri: params.text_document.uri,
                    typ: FileChangeType::CHANGED,
                }],
            };
            self.reload_changed_files(&params).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut docs = self.documents.write().await;
        docs.close(params.text_document.uri.as_str());
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let locale_dir = match self.ensure_locale_dir_for_uri(&uri).await {
            Some(locale_dir) => locale_dir,
            None => return Ok(None),
        };

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(uri.as_str()) else {
            return Ok(None);
        };

        let content = doc.content.to_string();
        let key_finder = self.key_finder.read().await;

        let Some(found_key) = key_finder.find_key_at_position(
            &content,
            position.line as usize,
            position.character as usize,
        ) else {
            return Ok(None);
        };

        let Some(hover_content) = self.get_hover_content(&locale_dir, &found_key.key).await else {
            return Ok(None);
        };

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("```text\n{}\n```", hover_content.trim_end()),
            }),
            range: None,
        }))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let locale_dir = match self.ensure_locale_dir_for_uri(&uri).await {
            Some(locale_dir) => locale_dir,
            None => return Ok(None),
        };

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(uri.as_str()) else {
            return Ok(None);
        };

        let content = doc.content.to_string();
        let line_content: String = content
            .lines()
            .nth(position.line as usize)
            .unwrap_or("")
            .to_string();

        let function_names = self.config.read().await.function_names.clone();

        let Some(prefix) = Self::extract_completion_prefix(
            &line_content,
            position.character as usize,
            &function_names,
        ) else {
            return Ok(None);
        };

        let completions = self.get_completions(&locale_dir, &prefix).await;

        if completions.is_empty() {
            return Ok(None);
        }

        Ok(Some(CompletionResponse::Array(completions)))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let locale_dir = match self.ensure_locale_dir_for_uri(&uri).await {
            Some(locale_dir) => locale_dir,
            None => return Ok(None),
        };

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(uri.as_str()) else {
            return Ok(None);
        };

        let content = doc.content.to_string();
        let key_finder = self.key_finder.read().await;

        let Some(found_key) = key_finder.find_key_at_position(
            &content,
            position.line as usize,
            position.character as usize,
        ) else {
            return Ok(None);
        };

        let Some(location) = self
            .get_definition_location(&locale_dir, &found_key.key)
            .await
        else {
            return Ok(None);
        };

        Ok(Some(GotoDefinitionResponse::Scalar(location)))
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri;

        let locale_dir = match self.ensure_locale_dir_for_uri(&uri).await {
            Some(locale_dir) => locale_dir,
            None => return Ok(None),
        };

        let display_locale = self.config.read().await.display_locale.clone();

        let docs = self.documents.read().await;
        let Some(doc) = docs.get(uri.as_str()) else {
            return Ok(None);
        };

        let content = doc.content.as_str();
        let key_finder = self.key_finder.read().await;
        let found_keys = key_finder.find_keys(content);

        let translation_store = self.translation_store.read().await;
        let Some(store) = translation_store.as_ref() else {
            return Ok(None);
        };

        let mut hints = Vec::new();
        let request_range = params.range;
        let request_is_empty = request_range.start == request_range.end;

        let position_leq = |a: Position, b: Position| -> bool {
            a.line < b.line || (a.line == b.line && a.character <= b.character)
        };

        let ranges_overlap = |start: Position, end: Position, range: &Range| -> bool {
            position_leq(range.start, end) && position_leq(start, range.end)
        };

        for found_key in found_keys {
            let key_start = Position {
                line: found_key.line as u32,
                character: found_key.start_char as u32,
            };
            let key_end = Position {
                line: found_key.line as u32,
                character: found_key.end_char as u32,
            };

            if !request_is_empty && !ranges_overlap(key_start, key_end, &request_range) {
                continue;
            }

            let raw_display = store
                .get_translation(&locale_dir, &found_key.key, &display_locale)
                .unwrap_or_else(|| found_key.key.clone());
            let display_text = truncate_string(&raw_display, 40);

            let mut hint_char = found_key.end_char;
            if let Some(line) = content.lines().nth(found_key.line) {
                let line_bytes = line.as_bytes();
                if matches!(line_bytes.get(hint_char), Some(b'\'') | Some(b'"')) {
                    hint_char += 1;
                }
            }

            hints.push(InlayHint {
                position: Position {
                    line: found_key.line as u32,
                    character: hint_char as u32,
                },
                label: InlayHintLabel::String(format!("= {}", display_text)),
                kind: Some(InlayHintKind::TYPE),
                text_edits: None,
                tooltip: None,
                padding_left: Some(true),
                padding_right: None,
                data: None,
            });
        }

        Ok(Some(hints))
    }
}

#[cfg(test)]
mod tests {
    use super::I18nBackend;

    #[test]
    fn extracts_completion_prefix_for_t_and_tt() {
        let names = vec!["t".to_string(), "tt".to_string()];
        let line1 = "const x = t('global_";
        let line2 = "const x = tt(\"profile.";
        assert_eq!(
            I18nBackend::extract_completion_prefix(line1, line1.len(), &names),
            Some("global_".to_string())
        );
        assert_eq!(
            I18nBackend::extract_completion_prefix(line2, line2.len(), &names),
            Some("profile.".to_string())
        );
    }

    #[test]
    fn does_not_extract_completion_from_member_calls() {
        let names = vec!["t".to_string(), "tt".to_string()];
        let line = "const x = i18n.t('foo";
        assert_eq!(
            I18nBackend::extract_completion_prefix(line, line.len(), &names),
            None
        );
    }

    #[test]
    fn fuzzy_match_prefers_prefix() {
        assert_eq!(
            I18nBackend::score_key_match("global_cancel", "glob"),
            Some(0)
        );
        assert_eq!(
            I18nBackend::score_key_match("auth.global_cancel", "glob"),
            Some(1)
        );
        assert_eq!(
            I18nBackend::score_key_match("global_cancel", "gcn"),
            Some(2)
        );
        assert_eq!(I18nBackend::score_key_match("global_cancel", "xyz"), None);
    }
}
