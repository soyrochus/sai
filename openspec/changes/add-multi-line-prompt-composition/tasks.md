## 1. Line-structure primitives

- [x] 1.1 Add `line_bounds()`, `cursor_line()`, and `cursor_column()` to `EditorState` in [src/editor.rs](src/editor.rs), deriving line structure from the flat buffer with no cached state, and defining a cursor sitting on a `\n` boundary as belonging to the line the break ends.
- [x] 1.2 Unit-test the three helpers directly: empty buffer, single line, trailing `\n`, consecutive `\n\n`, cursor at each boundary, and a buffer containing multi-byte and wide characters.

## 2. Line breaks in the buffer

- [x] 2.1 Change the `Alt+Enter` arm from a swallowed no-op to inserting `\n` at the cursor, keeping it matched ahead of the plain `Enter` arm so Enter still submits.
- [x] 2.2 Confirm `backspace` at the start of a line and `delete` at the end of a line join lines without special-casing — a `\n` is one character — and add tests pinning that behavior.
- [x] 2.3 Test that Enter submits the whole buffer from any cursor line, and that a buffer of only line breaks and spaces is rejected as whitespace-only, leaving the editor open.

## 3. Line-relative editing

- [x] 3.1 Rewrite `move_home` and `move_end` against the current line's bounds instead of `0` / `char_count()`.
- [x] 3.2 Rewrite `kill_to_end` as a bounded `replace_range(cursor..line_end, "")` — not `truncate` — so following lines and the line break survive, and `kill_to_start` as `replace_range(line_start..cursor, "")` with the cursor to line start.
- [x] 3.3 Verify `move_left` / `move_right` traverse line breaks with no code change, and add tests for Left at a line start and Right at a line end.
- [x] 3.4 Test `Ctrl+K` and `Ctrl+U` on a middle line of a three-line buffer, asserting the untouched lines are byte-identical afterwards, and that `Ctrl+A` on line two reaches line-two start rather than buffer start.

## 4. Vertical cursor movement and history dispatch

- [x] 4.1 Implement `move_up` / `move_down` as a column-preserving seek to the adjacent line, clamping to a shorter target line, with the column measured in characters rather than display width.
- [x] 4.2 Dispatch `Up` on `cursor_line() > 0` (move) versus `history_prev()` (recall), and `Down` on last-line position versus `history_next()`, reading the condition off the buffer each time with no mode flag.
- [x] 4.3 Confirm the existing single-line history tests pass **unmodified**. If any needs editing to go green, treat that as a dispatch bug rather than a stale test.
- [x] 4.4 Test the new dispatch: Up moving within a two-line buffer, Up from the first line reaching history, Down moving within before advancing history, and Up on the first line with empty history leaving buffer and cursor untouched.
- [x] 4.5 Test that a multi-line draft is saved and restored intact when navigating into history and back out past the newest entry.

## 5. Multi-row rendering

- [x] 5.1 Replace `prompt_line()` with `prompt_rows()` returning one string per buffer line plus a `(row, column)` cursor, with a continuation indicator on rows after the first of the same display width as `PROMPT_INDICATOR`.
- [x] 5.2 Update `render()` to draw the buffer rows above the guidance block and to park the cursor by descending `cursor_row` rows from the top, preserving the net-zero vertical displacement invariant the module documents.
- [x] 5.3 Keep cursor column arithmetic in display width so wide glyphs still advance two columns, now measured within the cursor's line rather than the whole buffer.
- [x] 5.4 Cap rendered buffer rows against `terminal::size()` with a truncation marker beyond the budget, so a prompt taller than the window cannot scroll the area off its top-row anchor.
- [x] 5.5 Verify `clear_area()` erases the full multi-row area on exit with no change needed, and add a test asserting the row count the renderer reports matches the buffer's line count.

## 6. Line and size indicator

- [x] 6.1 Add an indicator line to `guide_lines()` reporting `line N/M · C chars`, so it inherits dim styling and exit erasure from the existing guidance block.
- [x] 6.2 Count size in characters, not bytes, and test with `café` and a wide-character string.
- [x] 6.3 Test that the indicator tracks the cursor across lines and updates as lines are added and removed.

## 7. History storage and recall of multi-line prompts

- [x] 7.1 Confirm NDJSON round-trips an embedded `\n` losslessly in [src/prompt_history.rs](src/prompt_history.rs) — no format change expected — and add a test proving a multi-line prompt survives write-then-read.
- [x] 7.2 Test that a multi-line prompt is recalled as one history entry: Up loads all its lines with the cursor at the end, subsequent Up presses move through its lines, and Up from its first line loads the previous entry.
- [x] 7.3 Test that consecutive-duplicate collapsing treats the same text with a differently placed line break as two distinct prompts.
- [x] 7.4 Confirm reverse search matches text appearing on any line of a stored entry, and that accepting expands all its lines into the buffer.
- [x] 7.5 Render a multi-line match on the single search-status row with breaks shown as a visible marker, leaving the search area single-row while search is active.

## 8. Guidance, docs, and verification

- [x] 8.1 Add the line-break key and vertical movement to `HELP_LINES`, and name line-break insertion in the composing `hint()`, keeping the existing test that asserts the panel stays in step with what the editor implements.
- [x] 8.2 Update the `interactive` help topic in [src/help.rs](src/help.rs) and the README key-binding table with `Alt+Enter` and the multi-line behavior of Up/Down and the control shortcuts.
- [x] 8.3 Add a CHANGELOG entry noting the Up/Down and `Ctrl+A`/`E`/`K`/`U` semantics change, stating explicitly that single-line composition is unaffected.
- [x] 8.4 Run `cargo test` and `cargo clippy` clean.
- [x] 8.5 (Verified on macOS via a real PTY; literal Terminal.app automation was unavailable.) Manually verify on macOS Terminal: line-break insertion, cursor alignment down a multi-row block on lines containing wide characters, redraw as the buffer grows and shrinks, behavior in a window shorter than the prompt area, and a clean terminal on both submit and cancel.
- [x] 8.6 Note Linux and Windows Terminal/PowerShell verification as still outstanding, alongside the equivalent item carried over from the previous editor change.

Verification note: Linux and Windows Terminal/PowerShell still require human verification, as does the matching cross-platform item in the archived interactive-editor change. The macOS PTY pass covered line insertion, wide-character cursor alignment, vertical movement, grow/shrink redraw, terminal-height truncation, and cleanup after both submission and cancellation.
