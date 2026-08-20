## Why

The prompt editor shipped in v1.2.0 holds exactly one line, so complex intent has to be flattened into a single run-on sentence — the model then guesses at the structure that got lost, and the user pays for it in regeneration cycles. SPEC-03 of the v1.2.0 feature list addresses this directly, and the previous change already laid the groundwork for it: `Alt+Enter` was deliberately reserved and swallowed, and rendering was rewritten to track a multi-row prompt area.

## What Changes

- Make the editor buffer hold line breaks. `Alt+Enter` inserts one at the cursor; `Enter` continues to submit the whole buffer, so no existing muscle memory is retrained.
- Render the buffer across as many rows as it has lines, with a continuation indicator on rows after the first, and place the visible cursor at the correct (row, column) rather than a single column.
- Add a status indicator showing the cursor's current line, the total line count, and the buffer's total size in characters.
- **BREAKING (key semantics only)**: `Up`/`Down` become buffer-first. They move the cursor between buffer lines, and fall through to prompt-history navigation only when the cursor is on the first line (Up) or the last line (Down). In a single-line buffer — every buffer today — behavior is identical to the current behavior, so the change is invisible until the user creates a second line.
- **BREAKING (key semantics only)**: `Ctrl+A`, `Ctrl+E`, `Ctrl+K`, and `Ctrl+U` become line-relative rather than buffer-relative, matching readline and the literal wording of the existing spec ("line start", "line end"). Again identical for a single-line buffer.
- Submit the buffer as one consolidated prompt payload with its line breaks preserved verbatim into generation, and record it in prompt history with the breaks intact.
- Recall a multi-line prompt from history back into a multi-line buffer, and match reverse search against its full text.
- Leave the non-interactive path untouched: `--no-interactive`, piped input, and argument-supplied prompts stay single-line reads exactly as they are today.

### Decisions carried from clarification

- **Up/Down resolve buffer-first with edge fallthrough**, rather than keeping history on the arrows and relegating line movement to `Ctrl+P`/`Ctrl+N`. This is what bash and zsh do, it costs nothing for single-line buffers, and it keeps the arrows meaning "move" — which is what an arrow key means everywhere else.
- **The control shortcuts act on the current line.** The existing `prompt-input` spec already says "line start" and "line end"; that reading was simply unobservable while a buffer could only ever be one line. Making them buffer-relative would let a single `Ctrl+U` silently wipe several lines of composed text.
- **`Enter` still submits; `Alt+Enter` inserts the break.** The reverse (Enter inserts, Ctrl+Enter or similar submits) would be a real retraining cost for the common single-line case, which stays by far the most frequent.

## Capabilities

### New Capabilities

_None — this extends the editor that `prompt-input` already describes._

### Modified Capabilities

- `prompt-input`: The editor buffer gains line breaks and a multi-row prompt area; `Alt+Enter` moves from "reserved, does nothing" to "inserts a line break"; the line-editing and control-shortcut requirements become line-relative and gain vertical cursor movement; a new requirement covers the line/size indicator; submission preserves line breaks through to generation.
- `prompt-history`: Sequential navigation with Up/Down becomes conditional on the cursor sitting at the buffer's first or last line, and recall of a stored multi-line prompt restores its line breaks.

## Impact

- **Code**: [src/editor.rs](src/editor.rs) throughout — `EditorState` needs line-aware cursor arithmetic over its flat char index, `prompt_line` splits into a multi-row layout returning a (row, column) cursor, `render` must draw and clear a variable-height buffer area, `HELP_LINES` and `hint()` gain the new bindings.
- **Unchanged**: `src/app.rs`, `src/cli.rs`, `src/safety.rs`, and generation/execution flow. The editor still hands the caller one `String`; only its contents can now contain `\n`.
- **Storage**: none. `prompt_history.log` is NDJSON, so JSON string escaping already carries embedded newlines losslessly — no format change, no migration.
- **Dependencies**: none added.
- **Testing**: `EditorState` is already a pure state machine driven by injected `KeyEvent`s, so every new behavior is unit-testable headlessly. Terminal rendering of a wrapped multi-row area still needs manual verification, as it did for the single-line editor.
- **Docs**: README key bindings, the `interactive` help topic in [src/help.rs](src/help.rs), and a CHANGELOG entry.
