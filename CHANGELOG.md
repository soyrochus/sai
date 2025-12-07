# Releases/Changelog

## Relase v0.12.0 Package Rename & Deterministic Tests

Highlights:

- Crate renamed to `sai-cli` and published on crates.io; install directly with `cargo install sai-cli` to obtain the `sai` binary on any platform.
- README updated with crates.io install instructions alongside the existing release binaries.

Bug Fixes:

- Eliminated nondeterministic test failures by giving tests their own isolated config/history directories instead of mutating global env vars. This removes the race conditions that triggered sporadic failures in `history` and `app` suites.
- Added deterministic handling across history logging and explain/confirm paths, ensuring confirmation-related tests no longer depend on runtime ordering or shared state.

Tell the shell what you want, not how to do it.

---

## Relase v0.11.0 – Analysis Features and Interactive UX Improvements

Enhanced debugging capabilities with command history analysis and improved interactive prompts.

New Features:

Command history logging: Automatic NDJSON-based logging of all SAI invocations with rotation at 1MB
`--analyze` mode: AI-powered analysis of the most recent command to explain what happened and suggest fixes
`--explain` mode: Get detailed explanation of what a generated command will do before executing it
Single-key prompts: Interactive conflict resolution now accepts single keypress (O/S/C) without requiring Enter

What's Changed:

- New `src/history.rs` module implementing append-only NDJSON history log with automatic rotation
- `--analyze` flag reads latest history entry and asks LLM to diagnose what happened and why
- `--explain` flag generates command explanation before execution, always requires confirmation
- Interactive tool conflict resolution now uses `crossterm` for instant single-character input
- Added confirmation messages: "✓ Overwritten tool 'xyz'" and "✓ Skipped tool 'xyz' (kept existing)"
- Updated README.md and TECHSPEC.md with complete documentation of new features

Bug Fixes:

- Fixed confusing UX in tool conflict resolution where user input appeared to be ignored when skipping duplicate tools - now provides immediate visual feedback with single-key input and confirmation messages
- Fixed test race conditions in scope tests by adding mutex synchronization for directory changes

Tell the shell what you want, not how to do it.

See README.md for installation and TECHSPEC.md for technical details.

---

## Release v0.10.0 – Enhanced Command Execution and Tool Management

Improved glob pattern handling, directory awareness, and interactive tool configuration management.

Key Features:

Glob expansion: Commands like `wc -l src/*` now work naturally in safe mode without shell invocation
Directory awareness: `-s .` option provides current directory context to the LLM for smarter commands
Interactive tool imports: Conflict resolution when merging prompt configs with `--add-prompt`
Enhanced safety: Glob patterns expand securely without shell interpretation risks
Better UX: Improved help banner and documentation

What's Changed:

- Safe glob pattern expansion using the `glob` crate – wildcards work without requiring `--unsafe`
- Special `-s .` scope value sends directory listing to LLM for better file awareness
- Interactive duplicate resolution when importing tools from prompt files
- New `src/scope.rs` module for scope-aware context building
- Updated CLI help banner with project tagline
- 19 passing tests including new glob expansion coverage

Tell the shell what you want, not how to do it.

See README.md for installation and TECHSPEC.md for technical details.

---

## First release (v0.9.0) Pre-release

### SAI v0.9.0 – Natural Language Shell Commands with Safety Guarantees

Transform natural language into safe, executable shell commands using LLM intelligence with strict guardrails.

Key Features:

Safety-first: Whitelist-based tool execution with operator validation (pipes/redirects blocked by default)
Flexible prompts: Ships with ready-to-use configs for Unix tools, data processing (jq/yq/csvkit), and git workflows
Context-aware: --peek mode lets the LLM see sample data for smarter command generation
Fast & portable: Single-binary Rust implementation for Linux, macOS, and Windows
LLM-powered: OpenAI and Azure OpenAI support with configurable models
Tell the shell what you want, not how to do it.

See README.md for installation and usage details.
