## Context

See proposal.md — Why.

The constraints that shape this design come from what already exists in [src/editor.rs](src/editor.rs):

- `EditorState` is a pure state machine holding `buffer: String` and `cursor: usize`, where the cursor is a **character** index converted to a byte offset only at splice points via `byte_offset()`. It never touches the terminal, which is why every behavior here is unit-testable headlessly by feeding `KeyEvent`s.
- Every editing primitive (`insert_char`, `backspace`, `kill_to_end`, `move_home`, …) currently treats the buffer as one line, so "buffer start" and "line start" coincide.
- `render()` already draws a variable-height area — prompt row, optional help panel, hint row — and maintains an invariant worth quoting: it is entered and left with the terminal cursor on the **top row** of the area, so `Clear(FromCursorDown)` erases the whole area regardless of height, and every descent made while drawing is undone before returning. A render must have net vertical displacement of zero.
- `prompt_line()` returns `(String, usize)` — one rendered row and one cursor **column**, computed in display width rather than character count so wide glyphs advance two columns.
- `Alt+Enter` is already matched and swallowed ahead of the plain `Enter` arm ([editor.rs:355](src/editor.rs#L355)), reserved by the previous change for exactly this purpose.
- `prompt_history.log` is NDJSON, so JSON string escaping already carries `\n` losslessly. Nothing about storage changes.

## Goals / Non-Goals

**Goals:**

- Keep `EditorState` a pure, terminal-free state machine so multi-line behavior stays headlessly testable.
- Keep the flat `buffer: String` + char-index `cursor` representation. Line structure is *derived* from the buffer, never stored alongside it.
- Keep every existing single-line behavior bit-identical, so the current test suite passes unchanged rather than being rewritten.
- Confine the change to [src/editor.rs](src/editor.rs). The editor's contract with `app.rs` is one `String`; only its possible contents widen.

**Non-Goals:**

- Soft wrapping of a single long logical line across terminal columns. A logical line maps to one terminal row; the terminal's own wrapping handles overflow as it does today.
- Any change to the non-interactive read path, prompt-history file format, or the generation/safety/confirmation flow.
- Paste-aware bracketed-paste handling. A pasted multi-line block arrives as whatever key events the terminal sends; making paste a first-class input mode is separate work.
- Vertical scrolling of a prompt area taller than the terminal. See Risks.

## Decisions

### Derive line structure; do not store it

The buffer stays a single `String` and the cursor stays a single char index. Line structure comes from three derived helpers computed on demand:

- `line_bounds(&self) -> Vec<(usize, usize)>` — the (start, end) char index of each line, `end` exclusive of the `\n`.
- `cursor_line(&self) -> usize` — which line the cursor sits on; a cursor exactly on a `\n` boundary belongs to the line the break *ends*.
- `cursor_column(&self) -> usize` — offset of the cursor from its line's start, in characters.

*Alternative considered:* `Vec<String>` of lines with a `(row, col)` cursor. Rejected because it makes the state machine's invariants harder — every existing primitive would need rewriting, `load()` and the search/history paths would need to split and rejoin on every call, and the submitted payload would need a join step that must exactly reproduce what the user typed. Deriving from a flat string keeps the buffer *itself* the single source of truth, which is what makes "line breaks survive submission verbatim" true by construction rather than by careful reassembly.

*Cost:* `line_bounds()` is O(buffer length) per call. At prompt-composition sizes that is irrelevant, and it removes a whole class of desynchronization bug where a cached line vector disagrees with the buffer.

### Line-relative shortcuts are a two-line change each

Because the primitives already convert char index → byte offset at splice points, making them line-relative means substituting the line's bounds for `0` and `char_count()`:

- `move_home` → line start instead of `0`; `move_end` → line end instead of `char_count()`.
- `kill_to_end` → `replace_range(cursor..line_end, "")` instead of `truncate(cursor)`. This is the one that must change shape: `truncate` would drop every following line, which is precisely the destructive behavior the spec forbids.
- `kill_to_start` → `replace_range(line_start..cursor, "")` and cursor to line start, instead of truncating from `0`.

`move_left`/`move_right` need no change at all: a `\n` is one character, so `saturating_sub(1)` already steps across a line break correctly, satisfying the horizontal-traversal scenarios for free.

### Vertical movement is a column-preserving seek

`move_up`/`move_down` compute the current `(line, column)`, target the adjacent line, and clamp the column to that line's length. The column is a **character** offset, not a display width — the two differ for wide glyphs, and preserving the character column is what makes Up-then-Down a round trip. Display width is a rendering concern and stays in the render layer.

*Alternative considered:* remembering a "desired column" across consecutive vertical moves, as vim and emacs do, so passing through a short line does not permanently narrow the column. Deferred: it needs extra state that must be invalidated by every non-vertical key, and the specs do not require it. Worth revisiting if it turns out to annoy in practice.

### Up/Down dispatch on cursor line, not on a mode flag

`Up` is handled as: if `cursor_line() > 0`, move up within the buffer; otherwise delegate to the existing `history_prev()`. `Down` mirrors it against the last line and `history_next()`. There is no mode state and no flag — the condition is read off the buffer each time.

This is what makes the change invisible for a single-line buffer: with one line, `cursor_line()` is always 0 and always the last line, so both conditions hold immediately and both keys reach the history functions exactly as they do today. The existing history tests pass untouched, which is the strongest available evidence that the compatibility claim in the proposal holds.

*Alternative considered:* keeping history on the arrows and adding `Ctrl+P`/`Ctrl+N` for line movement. Rejected in clarification — arrows mean "move" everywhere else, and bash/zsh already set this expectation.

*Consequence worth naming:* after `history_prev()` loads a multi-line entry, `load()` parks the cursor at the end — i.e. on the last line. A further `Up` therefore moves *within* the recalled entry before continuing back through history. That is consistent (the recalled text is now just the buffer) but it does mean walking back through a history of multi-line prompts takes more keystrokes than walking back through single-line ones.

### Rendering: rows replace the single prompt row

`prompt_line()` becomes `prompt_rows(&EditorState) -> (Vec<String>, (usize, usize))` — one string per buffer line plus a `(row, column)` cursor. Row 0 carries `PROMPT_INDICATOR`; later rows carry a continuation indicator of the **same display width**, so text stays left-aligned down the block and the column arithmetic is a single shared constant rather than a per-row special case.

`render()` keeps its net-zero-displacement invariant, now generalized: descend `cursor_row` rows from the top instead of assuming row 0. The existing `Clear(FromCursorDown)` already erases whatever height the area had, so growing and shrinking the buffer needs no extra bookkeeping — this is the payoff from task 8.4 of the previous change.

The line/size indicator is rendered as part of the dim guidance block produced by `guide_lines()`, so it inherits "subordinate to the prompt" and "erased on exit" without new machinery. It reports `line N/M · C chars`.

### Reverse search stays single-line

While searching, the prompt area collapses to the existing single search-status row, unchanged. A multi-line match is previewed with its breaks rendered as a visible marker on that one row; accepting it expands the real breaks back into the buffer. Search matching runs against the entry's full text, so a query hitting line 3 finds it.

*Rationale:* the search row is a query interface, not a composition surface. Growing and shrinking the area on every keystroke of a query would be visually noisy, and readline's reverse search behaves the same way.

## Risks / Trade-offs

- **A prompt area taller than the terminal window** → The cursor-parking arithmetic assumes every row it counts is on screen; if the buffer plus guidance exceeds the window height, the terminal scrolls and the top-row anchor is lost, corrupting subsequent redraws. Mitigation: cap the rendered buffer rows at a window-height budget (query `terminal::size()`), rendering a truncation marker beyond it, and verify against a short window during manual testing. This is the single most likely source of visual breakage and deserves an explicit test pass.

- **Terminals that do not distinguish `Alt+Enter`** → Some terminal emulators send a bare `Enter` for `Alt+Enter`, or nothing. Mitigation: keep `Alt+Enter` and also bind `Ctrl+J`, whose line-feed control byte Crossterm distinguishes from carriage-return Enter in raw mode. Both bindings are documented in the hint and key panel.

- **Windows key-event duplication** → `run_loop` already filters non-press events, so this is covered; noted only because the new bindings must not bypass that filter.

- **`line_bounds()` recomputed per keystroke** → O(n) per key on a buffer of a few hundred characters. Accepted deliberately in exchange for having no cache to invalidate.

- **Existing tests as the compatibility oracle** → The claim that single-line behavior is unchanged rests on the existing editor tests continuing to pass without modification. If any of them needs editing to go green, that is a signal the dispatch conditions are wrong, not that the test is stale. Worth stating because "just update the test" is the tempting wrong move.

## Migration Plan

No data migration. `prompt_history.log` gains entries whose `prompt` field may contain `\n`, which the existing NDJSON reader already handles — an older SAI binary reading a newer store would load such an entry and render it as one long line, degrading rather than failing.

No rollback concern: the change is confined to one module and adds no persistent state.
