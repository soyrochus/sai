## Why

Today a SAI prompt must be typed as a single shell argument, which means quoting/escaping headaches, no way to fix a typo mid-sentence, and no way to reuse a prompt you already wrote — every refinement is a full retype. SPEC-01 and SPEC-02 of the v1.2.0 feature list address the same friction from two sides (authoring a prompt and recalling one), and they share a single line-editing loop, so building them separately would mean writing that loop twice.

## What Changes

- Add an interactive mini editor for composing natural-language prompts, opened when `sai` is invoked in a TTY with no prompt argument. It supports cursor movement (left/right, Home/End), insert/delete/backspace, `Ctrl+A`/`Ctrl+E`/`Ctrl+K`/`Ctrl+U`/`Ctrl+L`, Enter to submit, and Esc/`Ctrl+C` to cancel.
- Add `--interactive` to open the editor explicitly (and to pair the editor with a per-call prompt-config file), and `--no-interactive` to force the legacy single-line read even in a TTY.
- Make the positional prompt argument optional so `sai` alone is a valid invocation. **BREAKING (error surface only)**: `sai` with no arguments no longer exits with clap's "required argument" error; in a TTY it opens the editor, and outside a TTY it fails with an explicit "no prompt provided" message.
- Add a persistent prompt history file, separate from the existing execution `history.log`, recording each submitted prompt with a timestamp. It is size-capped with rotation, matching the existing history rotation approach.
- Add history navigation inside the editor: Up/Down through prior prompts, and `Ctrl+R` reverse incremental search. A recalled prompt lands in the editor buffer and is fully editable before submit.
- Leave generation, safety validation, explain, confirmation, and execution untouched — the editor only produces the prompt string that the existing flow already consumes.

### Decisions carried from clarification

- A prompt supplied as an argument runs directly, exactly as today. The editor never intercepts it. This keeps every existing invocation and every script byte-for-byte compatible.
- The editor is one-shot: compose one prompt, submit, run the normal flow, exit. No post-execution loop, which keeps SAI inside the "not a shell replacement" product boundary.
- Prompt history records submitted prompts only (timestamp + text), with consecutive duplicates collapsed. Outcome metadata stays in `history.log`, which already carries it.

## Capabilities

### New Capabilities

- `prompt-input`: How a natural-language prompt is obtained from the user — argument mode, the interactive mini editor, the non-interactive fallback, cancellation, and the flags that select between them.
- `prompt-history`: Persistent storage of submitted prompts and their recall from the editor via up/down navigation and reverse search, including file location, format, and rotation.

### Modified Capabilities

_None — `openspec/specs/` is currently empty, so both capabilities are introduced fresh._

## Impact

- **Code**: new `src/editor.rs` (line-editing loop, key handling, redraw) and `src/prompt_history.rs` (load/append/rotate/search); [src/cli.rs](src/cli.rs) gains `--interactive`/`--no-interactive` and makes `arg1` optional; [src/app.rs](src/app.rs) `run_with_reader` gains a prompt-resolution step ahead of `build_system_prompt`; [src/help.rs](src/help.rs) gains the new flag documentation.
- **Dependencies**: none added. `crossterm 0.27` is already a dependency (used by [src/ops.rs](src/ops.rs)) and covers raw mode, key events, and cursor control on macOS/Linux/Windows.
- **Config directory**: one new file alongside `config.yaml` and `history.log` under `config::config_root_dir()`.
- **Testing**: the editor must be driven by an injected key-event source so its behavior is unit-testable without a real terminal, mirroring the existing `IoPort`/reader injection style in [src/ops.rs](src/ops.rs) and [src/app.rs](src/app.rs).
- **Docs**: README usage section and CHANGELOG entry for v1.2.0.
