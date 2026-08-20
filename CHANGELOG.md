# Releases/Changelog

## Release v1.2.0 - Interactive Prompt Editor and Prompt History

Sai-cli 1.2.0 makes composing a prompt an editable step rather than a one-shot
shell argument, and remembers the prompts you have already written.

Highlights:
- `--unrestricted` lifts the tool whitelist and operator blocking for a single
  call, in generation as well as validation, and forces inspection in exchange:
  the command is always explained, always confirmed, and the confirmation
  requires typing `yes` in full. Because that explanation comes from the same
  model that wrote the command, the confirmation also shows risk markers
  computed locally from the command text. `safety.allow_unrestricted: false`
  forbids the mode outright.
- Interactive mini editor for composing natural language prompts. Running `sai`
  with no prompt in a terminal now opens it instead of erroring.
- Persistent prompt history with Up/Down navigation and `Ctrl+R` reverse
  search. Recalled prompts are editable before submission.
- New flags: `--interactive` / `-i`, `--no-interactive`, and `--prompt-config`.
- Full line editing in the prompt: cursor movement, Home/End, Delete,
  `Ctrl+A`/`Ctrl+E`/`Ctrl+K`/`Ctrl+U`/`Ctrl+L`, Esc / `Ctrl+C` to cancel.
- Multi-line prompt composition with `Alt+Enter`. Up/Down move between buffer
  lines before falling through to history at the first/last line, while
  `Ctrl+A`/`Ctrl+E`/`Ctrl+K`/`Ctrl+U` now act on the current line.
- Prompt history is stored as NDJSON in `prompt_history.log` under the config
  directory, rotating at 256 KB and created owner-readable only on Unix.

Compatibility:
- Single-line composition is unaffected by the new line-relative key semantics.
- Passing a prompt as an argument is unchanged: it runs directly, never through
  the editor. Piped and redirected input never opens the editor either.
- **Behaviour change**: `sai` with no arguments no longer exits with clap's
  "required argument" error. In a terminal it opens the editor; outside one it
  exits non-zero with an explicit "No prompt provided" message.
- No new dependencies; the editor is built on the `crossterm` crate already in
  use.

Tell the shell what you want, not how to do it.

---

## Release v1.1.0 - Model/API Refresh and Explain Guardrails

Sai-cli 1.1.0 updates OpenAI integration defaults and expands safety controls in prompt-driven workflows.

Highlights:
- Crate version bumped to 1.1.0.
- Rust edition bumped to 2024.
- Updated model and API usage in current branch work.
- Added force-explain capability in prompt/config tool definitions.

What's Changed:
- Updated model and API use (commit 6ae63fe4e8572320a9b38e30330b800a058ea55d).
- Updated API access and model (gpt-5.2-mini) (commit e9454ffd4ff18641457821e5f2bef8b0d0d6abd4).
- Implemented 'foce-explain' parameter in prompt/config files (commit ea7ebf45f124b6c6439482a126804a0fea799059).

Tell the shell what you want, not how to do it.

---

## Release v1.0.0 – First Stable Sai-cli

Sai-cli reaches its first stable, full release with improved onboarding, clearer
help, and refreshed defaults.

Highlights:
- Stable 1.0.0 version bump with a focus on predictable UX and docs.
- Help system rewritten with extracted templates for easier maintenance and
  clearer guidance across all topics.
- Global init now seeds standard commands by default for faster setup.

What's Changed:
- Refined help output and topic content (commit 6d9f91f31adbc691318b89faf1c65feee6d6a1a2).
- Global init now includes the standard command set (commit 6df92e3262696bd508019ba37f6ffedd56410373).
- Additional fixes and polish captured in the git log for this release.
- Extracted help templates to `/templates/help` for maintainability and reuse.

Tell the shell what you want, not how to do it.

---

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
