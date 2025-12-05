# SAI v0.10.0 Release Notes

**Release Date:** December 5, 2025

## Overview

Version 0.10.0 introduces significant improvements to SAI's command execution and prompt management capabilities, making it more robust and user-friendly for everyday shell automation tasks.

---

## 🎯 Major Features

### Glob Pattern Expansion in Safe Mode

SAI now safely expands glob patterns (wildcards like `*`, `?`, `[...]`) in command arguments without requiring shell invocation. This enables commands like `wc -l src/*` to work naturally while maintaining strict security guarantees.

**How it works:**

- Glob patterns in arguments are detected and expanded to actual file paths
- If no files match, the literal pattern is passed through
- Expansion happens within SAI's executor, avoiding shell interpretation risks
- **No changes required** to existing configurations or usage patterns

**Example:**

```bash
sai -s . "how many lines are in all the files in src"
# Generates: wc -l src/*
# Executes as: wc -l src/app.rs src/cli.rs src/config.rs ...
```

**Technical Details:**

- Uses the `glob` crate for safe pattern matching
- Only expands patterns containing `*`, `?`, or `[` metacharacters
- Gracefully handles invalid patterns or permission errors
- Maintains all existing safety guarantees (no shell execution in safe mode)

### Directory Context with `-s .`

The `--scope` flag now supports a special value `.` that provides directory awareness to the LLM by sending a listing of the current working directory.

**Benefits:**

- LLM understands what files/directories exist in the working directory
- Improves accuracy of generated commands
- Helps avoid commands that reference non-existent files
- Bounded by size limits to control token usage

**Usage:**

```bash
sai -s . "count the rust source files"
# LLM receives directory listing and generates appropriate command
```

### Interactive Duplicate Resolution for Tool Imports

When merging tool definitions from prompt files using `--add-prompt`, SAI now provides interactive conflict resolution if tool names already exist in the global configuration.

**Features:**

- Shows both definitions side-by-side when conflicts occur
- Options for each conflict:
  - **O**verwrite: Replace global definition with imported one
  - **S**kip: Keep existing global definition
  - **C**ancel: Abort entire import operation
- Non-interactive contexts (no TTY) produce clear error messages
- All-or-nothing: no changes until all conflicts are resolved

**Example:**

```bash
sai --add-prompt prompts/custom-tools.yml
# If 'jq' tool exists in both:
# Shows current vs. new definition
# Prompts: [O]verwrite / [S]kip / [C]ancel?
```

---

## 🛠️ Improvements

### Enhanced CLI Help Banner

The help text now includes SAI's tagline for clarity:

```text
AI-powered, YAML-configured command executor
Tell the shell what you want, not how to do it
```

### Documentation Updates

- **TECHSPEC.md**: Updated execution model documentation to describe glob expansion behavior
- **README.md**: Fixed broken GitHub release page link and badge rendering issues
- Added comprehensive test coverage for glob expansion edge cases

---

## 🔧 Technical Changes

### New Dependencies

- Added `glob = "0.3"` for safe wildcard pattern expansion

### New Module

- `src/scope.rs`: Utilities for building scope-aware context (directory listing helper)

### Modified Modules

- `src/executor.rs`: Implements glob expansion in safe mode execution path
- `src/ops.rs`: Enhanced with interactive duplicate resolution logic
- `src/llm.rs`: Scope context integration for LLM prompts
- `src/cli.rs`: Updated help banner text

### Test Coverage

- New tests for glob expansion with matches, no matches, and invalid patterns
- Tests for directory listing truncation behavior
- Duplicate resolution workflow tests (overwrite, skip, cancel scenarios)

---

## 📊 Statistics

- **11 files changed**
- **755 insertions**, 34 deletions
- **5 commits** since v0.9.0
- New functionality: **3 major features**
- Test suite: **19 tests passing**

---

## 🔒 Security

All changes maintain SAI's security model:

- Glob expansion avoids shell interpretation
- Only filesystem globs are expanded (no shell constructs)
- Directory listing is opt-in via `-s .`
- Existing safety layers (whitelisting, operator blocking, confirmation) unchanged

---

## 🚀 Upgrading

### From v0.9.0

No breaking changes. Simply replace your binary:

```bash
# Linux example
chmod +x sai-x86_64-unknown-linux-gnu
sudo mv sai-x86_64-unknown-linux-gnu /usr/local/bin/sai
```

Existing configurations, prompt files, and workflows continue to work without modification.

### New Cargo.toml Dependency

If building from source, note the new dependency:

```toml
glob = "0.3"
```

Run `cargo build` to fetch and compile.

---

## 📝 Commit History

```text
436337e Version bump to v0.10.0
75b3254 Glob expansion in safe mode to avoid errors when using commands with path expressions like 'src/*'
114f605 Implemented features confirm duplicate command on import and . expansion on -s .
5d888fe Fixed error in README.md badges
dd0e3ed Fixed broken link to Github release page
```

---

## 🙏 Acknowledgments

Thanks to all users who reported issues and provided feedback that shaped this release.

---

## 📦 Download

Get the prebuilt binaries from the [GitHub Releases page](https://github.com/soyrochus/sai/releases/tag/v0.10.0):

- **Linux**: `sai-x86_64-unknown-linux-gnu`
- **macOS Intel**: `sai-x86_64-apple-darwin`
- **macOS Apple Silicon**: `sai-aarch64-apple-darwin`
- **Windows**: `sai.exe`

Or build from source:

```bash
git clone https://github.com/soyrochus/sai.git
cd sai
git checkout v0.10.0
cargo build --release
```

---

## 📚 Documentation

- [README](README.md) - Getting started and usage guide
- [TECHSPEC](TECHSPEC.md) - Technical architecture and design
- [FOSS Pluralism Manifesto](FOSS_PLURALISM_MANIFESTO.md) - Participation principles

---

**Full Changelog**: <https://github.com/soyrochus/sai/compare/v0.9.0...v0.10.0>
