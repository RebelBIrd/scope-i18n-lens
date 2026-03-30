<div align="center">

# 🔍 Scope i18n Lens

**Monorepo-aware i18n support for Zed Editor - see translations inline by package scope.**

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Zed](https://img.shields.io/badge/zed-extension-purple.svg)](https://zed.dev)

Stop guessing what `t("common.buttons.submit")` means in large monorepos.<br/>
**See translations inline. Catch missing keys instantly. Ship with confidence.**

[Features](#-features) · [Install](#-installation) · [Configure](#-configuration) · [Contribute](#-contributing)

</div>

---

## ✨ Features

| Feature | Description |
|---------|-------------|
| 🔍 **Inline Hints** | See translation values right next to your i18n keys |
| 💬 **Hover Preview** | View all locale translations with quick jump links |
| ⚠️ **Missing Key Detection** | Get warnings for undefined translation keys |
| 🌐 **Incomplete Coverage** | Know which locales are missing translations |
| ⚡ **Autocomplete** | Type `t("` and get instant key suggestions with previews |
| 🎯 **Go to Definition** | Jump directly to the translation in any locale file |
| 🔄 **Auto Reload** | Changes to translation files are picked up automatically |

## 🎬 Demo

```tsx
// Before: What does this even mean? 🤔
<button>{t("common.actions.submit")}</button>

// After: Crystal clear! ✨
<button>{t("common.actions.submit")}</button>  // → Submit
```

**Hover over any i18n key to see:**
```
🌍 common.actions.submit

en: Submit (↗)
vi: Gửi (↗)
ja: 送信 (↗)
---
```

![Hover Preview](screenshots/screenshot-1.png)

![Autocomplete](screenshots/screenshot-2-auto-compelete.png)

## 🚀 Installation

### From Zed Extensions (Recommended)

1. Open Zed
2. Go to Extensions (`cmd+shift+x`)
3. Search for "Scope i18n Lens"
4. Click Install

### Build from Source

```bash
git clone https://github.com/BigHuang/scope-i18n-lens.git
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

**Restart Zed. Done. 🎉**

## 🎯 Supported Frameworks

Works out of the box with:

| Framework | Patterns |
|-----------|----------|
| **react-i18next** | `t("key")` `useTranslation()` `<Trans i18nKey="key">` |
| **i18next** | `t("key")` `i18n.t("key")` |
| **vue-i18n** | `$t("key")` `t("key")` |
| **react-intl** | `formatMessage({ id: "key" })` |
| **ngx-translate (Angular)** | `translateService.instant("key")` `translateService.get("key")` `| translate` |
| **Transloco (Angular)** | `translocoService.translate("key")` `selectTranslate("key")` `| transloco` |
| **Laravel** | `__("key")` `trans("key")` `Lang::get("key")` `@lang("key")` |
| **Flutter (gen_l10n)** | `AppLocalizations.of(context)!.key` |
| **easy_localization** | `'key'.tr()` `tr('key')` `context.tr('key')` |
| **flutter_i18n** | `FlutterI18n.translate(context, 'key')` `I18nText('key')` |
| **GetX** | `'key'.tr` `'key'.trParams({})` |
| **Custom** | Configure your own patterns! |

## 🧩 Supported Languages

- TypeScript / TSX
- JavaScript / JSX
- HTML
- Angular templates
- PHP
- Blade
- Dart (Flutter)
- Vue.js

## ⚙️ Configuration

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

<details>
<summary><strong>📋 All Options</strong></summary>

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `localeDirNames` | `string[]` | `["locales"]` | Locale directory names searched upward inside package boundary |
| `locales` | `string[]` | `["zh-CN", "zh-HK", "en"]` | Locale list used by hover/diagnostics/completion |
| `sourceLocale` | `string` | `"en"` | Your primary language |
| `displayLocale` | `string` | `"en"` | Locale used for inlay hints and completion details |
| `keyStyle` | `"nested" \| "flat"` | `"flat"` | Translation key style |
| `functionNames` | `string[]` | `["t", "tt"]` | Function names to detect as translation calls |
| `monorepoDetectors` | `string[]` | `["yarn.lock", "pnpm-workspace.yaml", "lerna.json"]` | Stop markers when package root is missing |
| `maxWalkDepth` | `number` | `10` | Safety limit for upward directory traversal |

</details>

<details>
<summary><strong>🔧 Custom Function Names</strong></summary>

```json
{
  "functionNames": ["t", "tt", "translate"]
}
```

</details>

## 📁 Supported File Formats

| Format | Extensions |
|--------|------------|
| JSON | `.json` |
| YAML | `.yaml` `.yml` |
| PHP | `.php` |
| ARB (Flutter) | `.arb` |

**Nested structure:**
```
locales/
├── en/
│   └── common.json
├── vi/
│   └── common.json
└── ja/
    └── common.json
```

**Or flat structure:**
```
locales/
├── en.json
├── vi.json
└── ja.json
```

**Flutter ARB structure:**
```
lib/
└── l10n/
    ├── app_en.arb
    ├── app_es.arb
    └── app_vi.arb
```

## 🛠️ Development

```bash
cargo test          # Run tests
cargo build         # Debug build
cargo build -r      # Release build

# Run with debug logging
RUST_LOG=debug ./target/release/scope-i18n-lens
```

## 🤝 Contributing

Contributions are welcome! Here's how:

1. Fork the repository
2. Create your feature branch (`git checkout -b feat/amazing-feature`)
3. Commit your changes (`git commit -m 'feat: add amazing feature'`)
4. Push to the branch (`git push origin feat/amazing-feature`)
5. Open a Pull Request

### Ideas for Contribution

- [ ] Extract hardcoded strings to translation files
- [ ] Support for more file formats (TOML, PO)
- [ ] Namespace support for large projects
- [ ] Translation file validation
- [ ] Integration with translation services

## 📄 License

MIT © [Trong Nguyen](https://github.com/nguyenphutrong)

---

<div align="center">

**If this project helps you, consider giving it a ⭐**

[Report Bug](https://github.com/BigHuang/scope-i18n-lens/issues) · [Request Feature](https://github.com/BigHuang/scope-i18n-lens/issues)

</div>
