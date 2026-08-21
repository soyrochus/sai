# Chapter 12 — An interactive terminal editor

Rich natural-language prompts need multiple lines, cursor editing, history recall, and cancellation. Terminal I/O is difficult to test; state transitions are not.

## Product goal

Create a pure editor core driven by input events, then place a thin Crossterm driver around it.

## Rust concepts

This feature develops enums as events, explicit mutable state, UTF-8 character-to-byte conversion, event loops, history invariants, and RAII cleanup through `Drop`.

## Build

Add:

```toml
crossterm = "0.29"
```

Start with application-specific events rather than terminal events:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EditorInput {
    Character(char),
    Left,
    Right,
    Newline,
    HistoryPrevious,
    HistoryNext,
    Submit,
    Cancel,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EditorAction {
    Continue,
    Submitted(String),
    Cancelled,
}

pub struct EditorState {
    buffer: String,
    cursor: usize, // character index, never a byte index
    history: Vec<String>,
    history_index: Option<usize>,
    saved_draft: Option<String>,
}
```

Keep UTF-8 conversion in one helper:

```rust
impl EditorState {
    fn byte_offset(&self, char_index: usize) -> usize {
        self.buffer
            .char_indices()
            .nth(char_index)
            .map(|(offset, _)| offset)
            .unwrap_or(self.buffer.len())
    }

    fn insert(&mut self, ch: char) {
        let byte = self.byte_offset(self.cursor);
        self.buffer.insert(byte, ch);
        self.cursor += 1;
    }

    pub fn handle(&mut self, input: EditorInput) -> EditorAction {
        match input {
            EditorInput::Character(ch) => self.insert(ch),
            EditorInput::Newline => self.insert('\n'),
            EditorInput::Left => self.cursor = self.cursor.saturating_sub(1),
            EditorInput::Right => {
                self.cursor = (self.cursor + 1).min(self.buffer.chars().count());
            }
            EditorInput::HistoryPrevious => self.history_previous(),
            EditorInput::HistoryNext => self.history_next(),
            EditorInput::Submit => return EditorAction::Submitted(self.buffer.clone()),
            EditorInput::Cancel => return EditorAction::Cancelled,
        }
        EditorAction::Continue
    }
}
```

When history navigation begins, save the current draft. Moving forward past the newest recalled item must restore it. Implement those two methods with that invariant before adding reverse search.

The terminal driver owns raw mode and rendering:

```rust
terminal::enable_raw_mode()?;
let outcome = loop {
    render(&state)?;
    if let Event::Key(key) = event::read()? {
        let input = map_key(key);
        if let Some(input) = input {
            match state.handle(input) {
                EditorAction::Continue => {}
                done => break done,
            }
        }
    }
};
terminal::disable_raw_mode()?;
```

Use an RAII guard so raw mode is restored even when rendering returns an error. The complete reference state machine is in [`src/editor.rs`](../../../src/editor.rs).

## AI collaboration script

Split the work into bounded prompts:

> Define editor state, application-specific inputs, and outcomes. Specify invariants for cursor bounds, Unicode, history navigation, draft restoration, submission, and cancellation. Do not write terminal I/O yet.

Then:

> Implement only the pure transitions and table tests. Treat cursor positions as character indices and centralize character-to-byte conversion.

Only after those pass:

> Add a thin Crossterm adapter that maps keys, renders state, and guarantees raw-mode restoration with a guard.

This sequence keeps the complex logic reviewable and prevents terminal escape codes from dominating every test.

## Compiler conversation

A `String` stores UTF-8 bytes, while a user-visible cursor usually moves over characters. `String::insert` requires a byte boundary. `byte_offset` converts the state machine’s character index immediately before mutation.

Even character counts are not identical to displayed columns: combining marks and wide emoji can occupy zero or two terminal cells. This checkpoint handles valid UTF-8 and character movement; production-quality display width may use the `unicode-width` crate.

The raw-mode guard demonstrates `Drop`:

```rust
struct RawModeGuard;

impl RawModeGuard {
    fn enter() -> std::io::Result<Self> {
        crossterm::terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}
```

Cleanup happens as stack values leave scope, including most `?` error paths.

## Tests

Drive the state directly:

```rust
#[test]
fn inserts_unicode_at_character_cursor() {
    let mut state = EditorState::new("aé".into(), vec![]);
    state.cursor = 1;
    state.handle(EditorInput::Character('🙂'));
    assert_eq!(state.buffer, "a🙂é");
    assert_eq!(state.cursor, 2);
}

#[test]
fn multiline_prompt_is_submitted_verbatim() {
    let mut state = EditorState::new(String::new(), vec![]);
    for input in [
        EditorInput::Character('a'),
        EditorInput::Newline,
        EditorInput::Character('b'),
    ] {
        assert_eq!(state.handle(input), EditorAction::Continue);
    }
    assert_eq!(
        state.handle(EditorInput::Submit),
        EditorAction::Submitted("a\nb".into())
    );
}

#[test]
fn leaving_history_restores_draft() {
    let mut state = EditorState::new("draft".into(), vec!["newest".into()]);
    state.handle(EditorInput::HistoryPrevious);
    assert_eq!(state.buffer, "newest");
    state.handle(EditorInput::HistoryNext);
    assert_eq!(state.buffer, "draft");
}

#[test]
fn cancel_does_not_submit() {
    let mut state = EditorState::new("private draft".into(), vec![]);
    assert_eq!(state.handle(EditorInput::Cancel), EditorAction::Cancelled);
}
```

Keep terminal-driver tests limited to key mapping and formatting. Verify raw-mode behavior manually in a real TTY because captured CI streams are not equivalent terminals.

## Review checklist

- Editing logic does not read from or write to the terminal.
- Cursor invariants are stated and tested with Unicode.
- History navigation restores the user’s draft.
- Cancellation cannot be mistaken for an empty submission.
- A `Drop` guard restores terminal mode on error paths.
- Driver-only behavior has a documented manual check.

## Checkpoint

```bash
git add Cargo.toml Cargo.lock src
git commit -m "tutorial: add a testable terminal editor"
git tag tutorial-12-terminal-editor
```

Evidence: pure state tests pass, a real terminal accepts a multiline prompt, history navigation restores a draft, and cancellation leaves no command to execute.

## Stretch exercise

Add reverse incremental search as a nested state containing the query, current match, saved buffer, and saved cursor. Write the state transitions before mapping Ctrl+R.

## Reflection

- Which parts became simple once terminal I/O was removed from the state machine?
- What is the difference between byte, Unicode scalar, grapheme, and display-column positions?
- How does `Drop` make cleanup more reliable than repeating it at every return site?

## Further learning

- [The Rust Book — Running Code on Cleanup with the `Drop` Trait](https://doc.rust-lang.org/book/ch15-03-drop.html) — the guarantee behind `RawModeGuard`.
- [The Rust Book — Storing UTF-8 Encoded Text with Strings](https://doc.rust-lang.org/book/ch08-02-strings.html) — why `byte_offset` needed its own helper.
- [Comprehensive Rust — `Drop`](https://google.github.io/comprehensive-rust/memory-management/drop.html)
- [Rust by Example — RAII](https://doc.rust-lang.org/rust-by-example/scope/raii.html)

Next: [Deterministic commands](13-deterministic-commands.md).
