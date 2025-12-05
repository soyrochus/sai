
---

Latest release (v0.10.0)
SAI v0.10.0 – Enhanced Command Execution and Tool Management

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

First release (v0.9.0) Pre-release
SAI v0.9.0 – Natural Language Shell Commands with Safety Guarantees

Transform natural language into safe, executable shell commands using LLM intelligence with strict guardrails.

Key Features:

Safety-first: Whitelist-based tool execution with operator validation (pipes/redirects blocked by default)
Flexible prompts: Ships with ready-to-use configs for Unix tools, data processing (jq/yq/csvkit), and git workflows
Context-aware: --peek mode lets the LLM see sample data for smarter command generation
Fast & portable: Single-binary Rust implementation for Linux, macOS, and Windows
LLM-powered: OpenAI and Azure OpenAI support with configurable models
Tell the shell what you want, not how to do it.

See README.md for installation and usage details.
