## Context

See `proposal.md` — Why.

The constraints that shape this design come from the existing code:

- The prompt reaches generation as a plain `String` in `run_with_reader` ([src/app.rs:239](src/app.rs#L239)), assembled from `cli.arg1` / `cli.prompt`. Everything downstream — `build_system_prompt`, `generate`, `validate_and_split_command`, `confirm`, `execute` — consumes only that string. So the entire feature can be inserted as one step that produces the string, with no downstream edits.
- `arg1` is `required_unless_present_any` ([src/cli.rs:71](src/cli.rs#L71)), so clap currently rejects a bare `sai`. That constraint has to be relaxed for the editor to ever open.
- `run_with_reader` already takes an injected `&mut R: BufRead` for confirmation input, and [src/ops.rs](src/ops.rs) already defines an `IoPort` trait with an `is_interactive()` TTY check and a test double. The project has an established injection pattern for terminal interaction; the editor should follow it rather than invent a second one.
- `crossterm 0.27` is already a dependency and already used for raw-mode key reading in [src/ops.rs](src/ops.rs).
- `history.rs` already implements exactly the storage shape needed: JSONL append to a file under `config::config_root_dir()`, with `HISTORY_MAX_BYTES` and `rotate_history_if_needed` using a `.bak` backup file ([src/history.rs](src/history.rs)).

## Goals / Non-Goals

**Goals:**

- Insert prompt acquisition as a single seam in `run_with_reader` so that no downstream stage learns about input modes.
- Make the editor's state machine testable without a TTY — key events in, buffer/outcome out.
- Reuse the existing history file conventions (config dir, JSONL, size-capped rotation) rather than introducing a second storage idiom.
- Fail soft: any terminal or history-file problem degrades to the old single-line behavior instead of aborting the run.

**Non-Goals:**

- Multi-line composition. The buffer is deliberately single-line; SPEC-03 will extend it and this design keeps the buffer type able to grow into that.
- Rendering beyond one prompt line plus an optional search-status line. No panes, no full-screen alternate buffer.
- Any change to `history.log`, `--analyze`, or the `RunSummary` shape.

## Decisions

### Separate the editor's state machine from terminal I/O

`editor.rs` splits in two: an `EditorState` holding the buffer, cursor index, history cursor, saved draft, and search state, exposing a pure `apply(KeyEvent) -> EditorAction` transition; and a thin driver that enables raw mode, reads `crossterm::event::read()`, feeds events to the state, and redraws.

Rationale: the acceptance criteria in both specs are almost entirely state-machine assertions (`Ctrl+K` truncates, Down past newest restores the draft, `Ctrl+R` cycles matches). Testing those through a real terminal is impractical; testing them as `apply()` calls is trivial. It also matches how `ops.rs` already separates decision logic from `IoPort`.

Alternative considered: adopt `rustyline`, which supplies line editing, history, and `Ctrl+R` out of the box. Rejected because it adds a substantial dependency tree to a CLI that currently has a deliberately small one, and because `crossterm` is already present and already used for raw-mode input — the editor here is roughly one screenful of state transitions. Worth revisiting if SPEC-03's multi-line work turns out to be expensive.

### Cursor and buffer are character-indexed, not byte-indexed

The buffer is a `String`; the cursor is an index into `chars()`, converted to a byte offset only at splice points. Column rendering uses display width, not character count.

Rationale: the spec requires multi-byte input to work and the rendered cursor to stay aligned. Byte indexing panics on non-ASCII splices; character indexing misplaces the cursor for wide CJK glyphs. Separating the two concerns handles both. If a width crate proves necessary for wide characters, add `unicode-width` — it is small and has no transitive weight.

### `--interactive` / `--no-interactive` resolve through one precedence function

A single `resolve_prompt_source(&cli, io) -> PromptSource` decides between `Argument(String)`, `Editor { prefill: Option<String> }`, and `PlainRead`, in this order:

1. `--no-interactive` present → `PlainRead` (or `Argument` if a prompt arg exists).
2. `--interactive` present → `Editor`, with `arg1` treated as prefill in simple mode.
3. A natural-language prompt argument present → `Argument`.
4. Not a TTY → `PlainRead`.
5. Otherwise → `Editor` with no prefill.

Clap declares the two flags mutually exclusive via `conflicts_with`, satisfying the conflicting-flags scenario without hand-written checks.

Rationale: keeping precedence in one pure function makes the spec's mode-selection scenarios directly testable and prevents the branching from spreading through `run_with_reader`.

### `--interactive` disambiguates the one-argument advanced-mode case

Today `sai foo.yaml` (a single argument) means "simple mode, prompt text is `foo.yaml`" — arg1 is only read as a config path when a second argument is present ([src/app.rs:224-237](src/app.rs#L224-L237)). Under `--interactive` the prompt comes from the editor, so a lone `arg1` is unambiguous and is read as the per-call config path. Without `--interactive`, `sai foo.yaml` keeps its current meaning exactly.

Alternative considered: sniff whether `arg1` names an existing `.yaml`/`.yml` file and switch modes on that. Rejected — a file named `notes.yaml` in the working directory would silently change what a prompt means, and the failure would be confusing. Requiring the explicit flag costs one word and removes the guesswork.

### Prompt history mirrors `history.rs` rather than extending it

New module `prompt_history.rs` writes JSONL entries `{ts, prompt}` to `config_root_dir().join("prompt_history.log")`, capped at its own byte limit with the same `.bak` rotation as `history.rs`.

Rationale: the two histories differ in cardinality, lifetime, and read pattern — execution history is written once per invocation and read only by `--analyze` (latest entry), while prompt history is read in full at editor startup and searched interactively. Overloading `HistoryEntry` with a nullable-everything prompt-only variant would complicate `--analyze` for no gain. The duplication is a rotation helper worth factoring into a shared private helper if it grows.

The prompt is recorded at submission time, before generation, so a prompt that produces an LLM error is still recallable — that is precisely a prompt the user will want to retry.

### History is loaded once, in memory, newest-first

At editor startup the store is read into a `Vec<String>` ordered newest-first, malformed lines skipped. Navigation indexes it; reverse search is a linear substring scan.

Rationale: bounded by the byte cap, the store holds on the order of thousands of prompts. A linear scan over that is microseconds — well inside "no perceptible delay" — and avoids an index format that would need its own invalidation and rotation handling.

### Cancellation is a distinct outcome, not an error

`Esc`/`Ctrl+C` return `EditorOutcome::Cancelled`, which `run_with_reader` maps to a `RunSummary` with `exit_code: 0` and `notes: Some("cancelled")` — the same shape the existing declined-confirmation path already produces ([src/app.rs:290-293](src/app.rs#L290-L293)).

Rationale: consistency with the existing cancel path, and it keeps "user changed their mind" out of shell error handling.

### Raw mode is guarded so the terminal is always restored

Raw mode is acquired by an RAII guard whose `Drop` disables it and shows the cursor, so panics and early returns both restore the terminal. If `enable_raw_mode()` fails, the editor is never entered and the driver falls back to `PlainRead` with a note on stderr.

Rationale: a CLI that leaves a terminal in raw mode on failure is worse than one without the feature. This directly satisfies the terminal-state-restored and raw-mode-refused scenarios.

## Risks / Trade-offs

- **Windows terminal behavior differs (key codes, `Ctrl+C` handling, ANSI support in legacy consoles)** → `crossterm` normalizes the event model, and the raw-mode-failure fallback covers consoles that cannot support it. Verify on Windows Terminal and PowerShell before release; CI already builds cross-platform, but the editor path needs manual confirmation since it cannot be exercised headlessly.
- **`Ctrl+C` becoming an editor key means it no longer signals the process while the editor is open** → scoped to editor composition only; the raw-mode guard restores default signal behavior on exit. `Ctrl+C` during generation or execution is unaffected.
- **Prompt history is plaintext in the config directory and prompts may contain sensitive strings (hostnames, paths, occasionally secrets)** → the file is created with user-only permissions where the platform supports it, and the README documents its location and how to clear it. Not encrypted; that would be a larger decision than this change warrants.
- **Making `arg1` optional weakens clap's argument validation, so a mistyped flag could fall through to the editor instead of erroring** → the non-TTY path still errors explicitly when no prompt is available, so scripts fail loudly; only interactive sessions see the editor.
- **Duplicated rotation logic between `history.rs` and `prompt_history.rs`** → accepted for now; factor into a shared helper if a third consumer appears.
- **Two capabilities in one change** → they share the editor loop, so splitting them would mean building `prompt-input` and then immediately reopening it. Task ordering keeps them separable: history work sits behind a complete, shippable editor.

## Migration Plan

No data migration. The feature is additive: existing invocations keep their exact behavior, and the prompt-history file is created on first use.

Rollback is removal of the two new modules and the two flags; the prompt-history file becomes an orphan that can be deleted by hand and breaks nothing if left behind.

## Open Questions

- Should the byte cap for `prompt_history.log` match `HISTORY_MAX_BYTES` (1 MB) or be smaller? Prompts are far shorter than execution records, so 1 MB stores a very large number of them. Deferrable — it is a constant, changeable without touching specs or task structure.
- Whether wide-character alignment needs `unicode-width` or whether character count suffices for the terminals in practice. Resolvable during implementation of the rendering task; it does not change the spec or the approach.
