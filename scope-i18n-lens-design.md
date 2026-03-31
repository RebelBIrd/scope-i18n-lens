# Scope i18n Lens — Zed Extension Design Spec

## Context

In the fe-wealth-admin monorepo (Yarn Workspaces, ~25 sub-apps/packages), each sub-package has its own `locales/` directory with flat JSON files for `zh-CN`, `zh-HK`, and `en`. The existing Zed extension **intl-lens** provides strong i18n inline hints but resolves locale paths from the project root, which causes cross-package pollution in monorepos.

**Goal:** Fork intl-lens and add monorepo-aware locale resolution so that `t('key')` and `tt('key')` in `apps/crm-next/` only read translations from `apps/crm-next/src/locales/` (or other locale dir inside the same package boundary), never from sibling packages. Publish to the Zed extension marketplace as `scope-i18n-lens`.

## Reference Projects

| Project | Language | Value |
|---------|----------|-------|
| [intl-lens](https://github.com/nguyenphutrong/intl-lens) | Rust | **Primary fork target** — full LSP with inlay hints, hover, diagnostics, autocomplete, goto-def |
| [i18n-ally](https://github.com/lokalise/i18n-ally) | TypeScript | Package boundary detection idea (walk up to nearest `package.json`) |
| [ruby-lsp-i18n](https://github.com/bukhr/ruby-lsp-i18n) | Ruby | Clean LSP addon pattern for inlay hints + hover |
| [zed-industries/extensions](https://github.com/zed-industries/extensions) | Rust | Standard Zed extension structure and publishing workflow |

## Architecture

### Project Structure

```text
scope-i18n-lens/
├── extension.toml              # Zed extension manifest
├── Cargo.toml                  # Workspace root
├── LICENSE                     # MIT
├── crates/
│   ├── extension/              # Zed WASM entry point
│   │   ├── Cargo.toml
│   │   └── src/lib.rs          # Downloads/launches LSP binary
│   ├── lsp-server/             # LSP server (runs as native binary, not WASM)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── locale_resolver.rs   # Monorepo-aware path resolution
│   │       ├── locale_store.rs      # Locale loading, caching, hot-reload
│   │       ├── hints.rs             # textDocument/inlayHint provider
│   │       ├── hover.rs             # textDocument/hover provider
│   │       ├── code_actions.rs      # textDocument/codeAction + applyEdit
│   │       ├── diagnostics.rs       # Missing key / incomplete coverage
│   │       ├── completion.rs        # textDocument/completion provider
│   │       └── definition.rs        # textDocument/definition provider
│   └── shared/                 # Common types and utilities
│       ├── Cargo.toml
│       └── src/lib.rs
└── README.md
```

### Core Module: `locale_resolver`

This is the key differentiation from intl-lens.

**Resolution algorithm:**

```text
fn resolve_locale_dir(file_path) -> Option<LocaleDir>:
    1. Check cache: file_path -> locale_dir mapping
    2. If cache miss:
       a. start_dir = file_path.parent()
       b. Find package_root by walking up from start_dir:
          - if current_dir contains package.json: package_root = current_dir; stop
          - if current_dir contains monorepo detector file (yarn.lock, etc.): stop with None
          - if reach filesystem root or maxWalkDepth: stop with None
       c. Walk up from start_dir to package_root (inclusive):
          - For each candidate in config.localeDirNames:
              if current_dir/candidate exists and contains at least one configured locale file:
                  cache[file_path] = current_dir/candidate
                  return current_dir/candidate
       d. return None
```

Key behavior:
- Never walk above `package_root`, so sibling package locales are never visible.
- Locale discovery is constrained by configured locale list, not by scanning all files.

### Core Module: `locale_store`

To support precise go-to-definition and safe edit-in-place, values are not stored as plain strings only.

```text
TranslationEntry {
  value: String,
  file_path: PathBuf,
  line: u32,
  column: u32,
}

LocaleMap = HashMap<Key, TranslationEntry>
PackageLocaleStore = HashMap<LocaleCode, LocaleMap>
```

### Cache Invalidation

- `workspace/didChangeWatchedFiles` for locale JSON changes -> reload changed locale file
- New file open with uncached path -> resolve on demand
- Workspace folder change -> clear all caches
- New package-local locale dir discovered -> register file watcher for that dir

## Configuration: `.zed/i18n.json`

Located at the workspace root (the directory opened in Zed). If not found, all defaults apply.

```json
{
  "localeDirNames": ["locales"],
  "locales": ["zh-CN", "zh-HK", "en"],
  "sourceLocale": "en",
  "displayLocale": "en",
  "keyStyle": "flat",
  "functionNames": ["t", "tt"],
  "monorepoDetectors": ["yarn.lock", "pnpm-workspace.yaml", "lerna.json"],
  "maxWalkDepth": 10
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `localeDirNames` | `["locales"]` | Directory names to match as direct children when walking up within the package boundary |
| `locales` | `["zh-CN", "zh-HK", "en"]` | Language list used by hover, diagnostics completeness checks, and edit form. Not inferred from `locales/*.json` |
| `sourceLocale` | `"en"` | Primary locale for go-to-definition target |
| `displayLocale` | `"en"` | Language shown in inlay hints and completion details |
| `keyStyle` | `"flat"` | `"flat"`: top-level keys only (dots are literal key chars). `"nested"`: dot segments map to nested objects |
| `functionNames` | `["t", "tt"]` | Function names to detect via Tree-sitter call expression analysis |
| `monorepoDetectors` | `["yarn.lock", ...]` | Files that signal monorepo root for early stop when package root cannot be found |
| `maxWalkDepth` | `10` | Maximum directory levels to walk up (safety net) |

## Feature Specifications

### 1. Inlay Hints

**Trigger:** Any `t('key')` or `tt('key')` call detected by `functionNames`.

**Behavior:**
- Display translation value from `displayLocale` as an inline hint after the closing parenthesis
- If key exists: show translated text (for default `displayLocale=en`, `t('global_cancel')` -> `// Cancel`)
- If key does not exist in current sub-package: show raw key string (`// unknown_key`)
- Truncate long translations to ~40 characters with `...`

**Performance:**
- Debounce 200ms after editing stops
- Only compute for visible viewport range
- Detection uses Tree-sitter AST to avoid false positives in comments/strings

### 2. Hover

**Trigger:** Hover over a translation key string inside `t()` or `tt()`.

**Display format:**

```text
global_cancel

zh-CN: 取消
zh-HK: 取消
en:    Cancel

Path: apps/crm-next/src/locales/en.json
```

Behavior:
- Show all locales from `settings.locales` (not fixed to 3)
- If missing in some locale, show `missing`
- If key has placeholders (`{0}`, `{name}`), append `Placeholders: {0}, {name}`

### 3. Edit Translations (Code Action)

**Trigger:** Cursor on key literal in `t('key')` or `tt('key')`, then run Code Action `Edit translations`.

**UI:**
- Single form containing one input per locale in `settings.locales`
- Prefill existing values
- Per-locale field status:
  - `file missing`: read-only (no auto file creation)
  - `key missing`: editable
  - `exists`: editable

**Save rules:**
- If locale file does not exist: skip write for that locale
- If locale file exists and key exists: update value in place
- If locale file exists and key is missing: append key at end of JSON object
- Do not reorder existing keys globally

**Post-save:**
- Re-parse changed locale files
- Refresh inlay hints, hover, diagnostics, completion

### 4. Diagnostics

**Levels:**
- **Error:** Key not found in any locale file of current sub-package
- **Warning:** Key exists in some configured locales but missing in others

**Scope:**
- Diagnose currently open file only

**Update triggers:**
- File open / file change (debounced)
- Locale JSON file change (re-diagnose open files for same locale dir)

### 5. Autocomplete

**Trigger:** Typing inside `t('` or `tt('` after opening quote.

**Behavior:**
- List all keys from current sub-package locale store
- Show `displayLocale` translation as detail/description
- Sort by prefix match, then alphabetically
- Fuzzy matching support

### 6. Go-to-Definition

**Trigger:** Cmd+Click or F12 on a translation key.

**Behavior:**
- Jump to `sourceLocale` JSON file (default `en.json`)
- Position cursor at exact line/column of key token
- If key does not exist in source locale: no-op

### 7. `tt()` Adaptation

`tt()` returns a multi-language object at runtime, but for extension behavior it is treated as `t()` for key resolution:
- Same inlay hint behavior (`displayLocale`)
- Same hover behavior (all configured locales)
- Same diagnostics/completion/definition behavior
- Detection via `functionNames` with both `t` and `tt`

### 8. Hot Reload

**Mechanism:**
- Register `workspace/didChangeWatchedFiles` for `**/*.json` under resolved locale dirs
- On change: debounce 500ms, re-parse only changed file
- Update `LocaleMap` entries for affected locale
- Re-trigger diagnostics for open files referencing same package locale dir

### 9. Variable Placeholder Display

**Detection:** Scan translation values for `{0}`, `{1}`, `{key}`, `{name}` patterns.

**Display:**
- In hover: add a line listing placeholders
- Example: `Placeholders: {0}, {name}`

## Known Limitations (V1)

1. **No cross-package locale merging.** The extension resolves from nearest locale dir inside package boundary only.
2. **Dynamic keys not supported.** Template literal interpolation keys are ignored.
3. **Locale files are JSON only.** `.ts`/`.tsx` locale sources are ignored.
4. **No one-click machine translation in V1.** API-backed translation autofill is deferred to V2.

## Performance Strategy

| Concern | Mitigation |
|---------|------------|
| Path resolution overhead | LRU cache: `FilePath -> LocaleDir`, cleared on workspace change |
| Locale loading at startup | Lazy load by package when first file opens |
| Inlay hints on large files | Debounce 200ms + visible range only |
| JSON parsing on hot reload | Re-parse changed locale file only |
| Diagnostics computation | Open files only, debounced |
| Memory usage | `TranslationEntry` map per locale per package; locale count from `settings.locales` |
| Walk-up safety | `maxWalkDepth` plus package boundary stop |

## Publishing Plan

1. Fork intl-lens and rename extension metadata to `scope-i18n-lens`
2. Implement monorepo resolver and code action editing in existing LSP architecture
3. Test locally via `Install Dev Extension` in Zed
4. License: MIT
5. Submit PR to `zed-industries/extensions` with git submodule
6. Extension ID: `scope-i18n-lens`

## Verification Plan

1. **Unit tests:**
- `locale_resolver` with multi-package mock filesystem
- boundary checks: never read sibling package locales
- nearest `package.json` stop behavior

2. **Integration tests:**
- open two package files and verify isolated inlay/completion sets
- edit translation via code action and verify JSON write behavior

3. **Manual tests in Zed:**
- Open monorepo root in Zed
- Edit `apps/crm-next/src/pages/foo.tsx` -> `t('key')` uses crm-next locales only
- Edit `apps/finance-next/src/pages/bar.tsx` -> `t('key')` uses finance-next locales only
- Negative test: finance keys do not autocomplete in crm file
- Hover on `t('key')` -> show all locales from settings (default `zh-CN/zh-HK/en`)
- Change `displayLocale` to `zh-CN` -> inlay and completion detail switch to zh-CN
- Code Action `Edit translations` on existing key -> save updates and refresh hints
- Missing locale file case -> field is read-only and not created
- Missing key in existing locale file -> editable and appended at file end on save
- Cmd+Click on key -> jump to exact key position in `sourceLocale` file

4. **No config test:**
- Remove `.zed/i18n.json` and verify defaults (`displayLocale=en`, locales list default) still work
