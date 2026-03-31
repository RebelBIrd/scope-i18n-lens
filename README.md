<div align="center">

# 🔍 Scope i18n Lens

**Monorepo-aware i18n support for Zed Editor — package-scoped translation hints, hover, diagnostics, and completion.**

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Zed](https://img.shields.io/badge/zed-extension-purple.svg)](https://zed.dev)

Forked from [intl-lens](https://github.com/nguyenphutrong/intl-lens) with monorepo-scoped locale resolution.

</div>

---

## Why this fork?

The original **intl-lens** uses a single global locale path relative to the workspace root. This breaks in monorepos where each package ships its own `locales/` directory.

**Scope i18n Lens** resolves translations per-package: it walks upward from the current file, finds the nearest `package.json` boundary, and looks for locale directories within that scope. Each package gets its own isolated translation context.

```
apps/
├── web/
│   ├── package.json        ← package boundary
│   ├── locales/             ← translations for web
│   │   ├── en.json
│   │   └── zh-CN.json
│   └── src/
│       └── page.tsx         ← t("key") resolves from web/locales/
├── admin/
│   ├── package.json        ← package boundary
│   ├── locales/             ← translations for admin
│   └── src/
│       └── dashboard.tsx    ← t("key") resolves from admin/locales/
```

## Features

| Feature | Description |
|---------|-------------|
| 🔍 **Inline Hints** | See translation values next to i18n keys |
| 💬 **Hover Preview** | View all locale translations with jump links |
| ⚠️ **Missing Key Detection** | Warnings for undefined translation keys |
| 🌐 **Incomplete Coverage** | Know which locales are missing translations |
| ⚡ **Autocomplete** | Type `t("` and get key suggestions with previews |
| 🎯 **Go to Definition** | Jump directly to the translation in locale files |
| 🔄 **Auto Reload** | Translation file changes are picked up automatically |
| 📦 **Monorepo Scoping** | Each package resolves its own locale directory |

## Installation

### From Zed Extensions (Recommended)

1. Open Zed
2. Go to Extensions (`cmd+shift+x`)
3. Search for "Scope i18n Lens"
4. Click Install

### Build from Source

```bash
git clone https://github.com/RebelBIrd/scope-i18n-lens.git
cd scope-i18n-lens
cargo build --release -p scope-i18n-lens
ln -sf $(pwd)/target/release/scope-i18n-lens ~/.local/bin/
```

### Configure Zed (Manual Installation)

Add to `~/.config/zed/settings.json`:

```jsonc
{
  "lsp": {
    "scope-i18n-lens": {
      "binary": { "path": "scope-i18n-lens" }
    }
  },
  "languages": {
    "TSX": {
      "language_servers": ["typescript-language-server", "scope-i18n-lens", "..."]
    },
    "TypeScript": {
      "language_servers": ["typescript-language-server", "scope-i18n-lens", "..."]
    }
  }
}
```

## Supported Languages

TypeScript, TSX, JavaScript, JSX, HTML, Angular, PHP, Blade, Vue.js

## Configuration

Create `.zed/i18n.json` in your project root:

```json
{
  "localeDirNames": ["locales", "src/locales"],
  "locales": ["zh-CN", "zh-HK", "en"],
  "sourceLocale": "en",
  "displayLocale": "en",
  "functionNames": ["t", "tt"]
}
```

### All Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `localeDirNames` | `string[]` | `["locales"]` | Directory names to search upward within the package boundary |
| `locales` | `string[]` | `["zh-CN", "zh-HK", "en"]` | Locale list for hover/diagnostics/completion |
| `sourceLocale` | `string` | `"en"` | Primary language |
| `displayLocale` | `string` | `"en"` | Locale shown in inlay hints and completion details |
| `keyStyle` | `"nested" \| "flat"` | `"flat"` | Translation key style |
| `functionNames` | `string[]` | `["t", "tt"]` | Function names to detect as translation calls |
| `monorepoDetectors` | `string[]` | `["yarn.lock", "pnpm-workspace.yaml", "lerna.json"]` | Fallback stop markers when no `package.json` is found |
| `maxWalkDepth` | `number` | `10` | Max upward directory traversal depth |

### How locale resolution works

1. Starting from the current file, walk upward to find the nearest `package.json` — this is the **package root**
2. Within that scope, search upward for a directory matching any name in `localeDirNames`
3. If no `package.json` is found, fall back to `monorepoDetectors` (e.g. `yarn.lock`) as the stop boundary
4. Results are cached per file path for performance

## Supported File Formats

| Format | Extensions |
|--------|------------|
| JSON | `.json` |
| YAML | `.yaml` `.yml` |
| PHP | `.php` |
| ARB (Flutter) | `.arb` |

## Development

```bash
cargo test          # Run tests
cargo build         # Debug build
cargo build -r      # Release build

# Run with debug logging
RUST_LOG=debug ./target/release/scope-i18n-lens
```

## Credits

This project is forked from [intl-lens](https://github.com/nguyenphutrong/intl-lens) by [Trong Nguyen](https://github.com/nguyenphutrong). The original project provides the core LSP infrastructure and multi-language i18n key detection.

## License

MIT

---

<div align="center">

[Report Bug](https://github.com/RebelBIrd/scope-i18n-lens/issues) · [Request Feature](https://github.com/RebelBIrd/scope-i18n-lens/issues)

</div>
