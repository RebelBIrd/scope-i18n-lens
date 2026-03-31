# Scope i18n Lens Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert intl-lens into `scope-i18n-lens` with monorepo-aware per-package locale resolution and behavior aligned with `scope-i18n-lens-design.md`.

**Architecture:** Keep current two-crate structure (`intl-lens`, `intl-lens-extension`) but introduce package-scoped locale resolution and package-local translation store in the server crate. LSP handlers resolve locale context per file path and only read translations within the same package boundary.

**Tech Stack:** Rust, tower-lsp, serde/serde_json, dashmap, regex

---

### Task 1: Config + Resolver Foundation

**Files:**
- Create: `crates/intl-lens/src/i18n/locale_resolver.rs`
- Modify: `crates/intl-lens/src/config.rs`
- Modify: `crates/intl-lens/src/i18n/mod.rs`

- [ ] **Step 1: Write failing tests** for resolver/package boundary and config defaults
- [ ] **Step 2: Implement resolver + config fields** (`localeDirNames`, `locales`, `displayLocale`, `functionNames`, `monorepoDetectors`, `maxWalkDepth`)
- [ ] **Step 3: Ensure key finder respects functionNames (`t`, `tt`)**

### Task 2: Package-Scoped Translation Store

**Files:**
- Modify: `crates/intl-lens/src/i18n/parser.rs`
- Modify: `crates/intl-lens/src/i18n/store.rs`

- [ ] **Step 1: Write failing tests** for isolated package lookups and key line/column tracking
- [ ] **Step 2: Refactor store model** to package-scoped locale maps with entry location metadata
- [ ] **Step 3: Implement load/reload by resolved locale dir**

### Task 3: Backend Behavior Alignment

**Files:**
- Modify: `crates/intl-lens/src/backend.rs`

- [ ] **Step 1: Write/adjust tests for helper behavior** (completion prefix, hover formatting helpers)
- [ ] **Step 2: Update LSP handlers** to resolve locale dir per document and use package-scoped store
- [ ] **Step 3: Align features** (displayLocale in hints/completion, diagnostics severities, sourceLocale goto-def)

### Task 4: Scope Rename + Extension Metadata

**Files:**
- Modify: `crates/intl-lens-extension/extension.toml`
- Modify: `crates/intl-lens-extension/src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `crates/intl-lens/Cargo.toml`
- Modify: `crates/intl-lens-extension/Cargo.toml`

- [ ] **Step 1: Rename extension id and release source** to `scope-i18n-lens`
- [ ] **Step 2: Keep compatibility fallback** for old binary name if needed

### Task 5: Verify

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Run `cargo test`**
- [ ] **Step 2: Run `cargo fmt --check`**
- [ ] **Step 3: Summarize any gaps if local toolchain unavailable**
