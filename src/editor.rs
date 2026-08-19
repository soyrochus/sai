//! Interactive mini editor for composing natural language prompts.
//!
//! The editing logic lives in [`EditorState`], a pure state machine driven by
//! [`crossterm`] key events. It never touches the terminal, so every editing,
//! shortcut, navigation and search behaviour can be unit tested by feeding it
//! `KeyEvent`s directly. The terminal driver sits in `driver.rs`-style code at
//! the bottom of this module and only renders what the state reports.

use anyhow::{anyhow, Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::style::{Color, ResetColor, SetForegroundColor};
use crossterm::terminal::ClearType;
use crossterm::{cursor, execute, queue, terminal};
use std::io::{self, BufRead, Write};

/// What the driver should do after feeding a key to the state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorAction {
    /// Keep editing; redraw the prompt area.
    Continue,
    /// Clear the prompt area completely, then redraw it (Ctrl+L).
    Redraw,
    /// Stop the loop and report this outcome.
    Finish(EditorOutcome),
}

/// How a composition session ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorOutcome {
    /// The user submitted this prompt text.
    Submitted(String),
    /// The user cancelled with Esc or Ctrl+C.
    Cancelled,
}

/// Reverse incremental search state, active only while the user is in Ctrl+R mode.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchState {
    /// The query typed so far.
    query: String,
    /// Index into `history` of the current match, if any.
    match_index: Option<usize>,
    /// Buffer and cursor to restore if the search is cancelled.
    saved_buffer: String,
    saved_cursor: usize,
}

/// The editor's full state: the prompt buffer plus history navigation and search.
#[derive(Debug, Clone)]
pub struct EditorState {
    buffer: String,
    /// Cursor position counted in characters, not bytes.
    cursor: usize,
    /// Prior prompts, newest first.
    history: Vec<String>,
    /// Position in `history` while navigating; `None` means "editing my own draft".
    history_index: Option<usize>,
    /// The draft that was in the buffer before history navigation began.
    saved_draft: Option<String>,
    search: Option<SearchState>,
    /// Whether the expanded key-binding panel is showing (Ctrl+G).
    show_help: bool,
}

impl EditorState {
    /// A fresh editor, optionally pre-loaded with `prefill` and with `history`
    /// ordered newest first.
    pub fn new(prefill: Option<String>, history: Vec<String>) -> Self {
        let buffer = prefill.unwrap_or_default();
        let cursor = buffer.chars().count();
        Self {
            buffer,
            cursor,
            history,
            history_index: None,
            saved_draft: None,
            search: None,
            show_help: false,
        }
    }

    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Cursor position in characters from the start of the buffer.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The active reverse-search query, if search mode is on.
    pub fn search_query(&self) -> Option<&str> {
        self.search.as_ref().map(|s| s.query.as_str())
    }

    /// Whether the expanded key-binding panel is showing.
    pub fn show_help(&self) -> bool {
        self.show_help
    }

    /// The one-line hint drawn under the prompt, matching the current mode.
    pub fn hint(&self) -> &'static str {
        if self.show_help {
            "^G hide keys"
        } else if self.search.is_some() {
            "^R next match \u{b7} Enter accept \u{b7} Esc cancel search \u{b7} ^G keys"
        } else {
            "\u{2191}\u{2193} history \u{b7} ^R search \u{b7} Enter send \u{b7} Esc cancel \u{b7} ^G keys"
        }
    }

    /// Whether reverse search is active.
    pub fn is_searching(&self) -> bool {
        self.search.is_some()
    }

    /// The prompt currently matched by reverse search, if any.
    #[cfg(test)]
    pub fn search_match(&self) -> Option<&str> {
        let search = self.search.as_ref()?;
        let index = search.match_index?;
        self.history.get(index).map(|s| s.as_str())
    }

    /// True when search is active with a query that matches nothing.
    pub fn search_failed(&self) -> bool {
        match self.search.as_ref() {
            Some(search) => !search.query.is_empty() && search.match_index.is_none(),
            None => false,
        }
    }

    fn char_count(&self) -> usize {
        self.buffer.chars().count()
    }

    /// Byte offset of character index `index`, for splicing into the buffer.
    fn byte_offset(&self, index: usize) -> usize {
        self.buffer
            .char_indices()
            .nth(index)
            .map(|(offset, _)| offset)
            .unwrap_or(self.buffer.len())
    }

    fn insert_char(&mut self, ch: char) {
        let offset = self.byte_offset(self.cursor);
        self.buffer.insert(offset, ch);
        self.cursor += 1;
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let offset = self.byte_offset(self.cursor - 1);
        self.buffer.remove(offset);
        self.cursor -= 1;
    }

    fn delete(&mut self) {
        if self.cursor >= self.char_count() {
            return;
        }
        let offset = self.byte_offset(self.cursor);
        self.buffer.remove(offset);
    }

    fn kill_to_end(&mut self) {
        let offset = self.byte_offset(self.cursor);
        self.buffer.truncate(offset);
    }

    fn kill_to_start(&mut self) {
        let offset = self.byte_offset(self.cursor);
        self.buffer.replace_range(..offset, "");
        self.cursor = 0;
    }

    fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    fn move_right(&mut self) {
        if self.cursor < self.char_count() {
            self.cursor += 1;
        }
    }

    fn move_home(&mut self) {
        self.cursor = 0;
    }

    fn move_end(&mut self) {
        self.cursor = self.char_count();
    }

    /// Replace the buffer wholesale and park the cursor at the end.
    fn load(&mut self, text: String) {
        self.buffer = text;
        self.cursor = self.char_count();
    }

    /// Walk one step towards older history entries.
    fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let next = match self.history_index {
            None => {
                self.saved_draft = Some(self.buffer.clone());
                0
            }
            Some(index) if index + 1 < self.history.len() => index + 1,
            // Already at the oldest entry: stay put.
            Some(index) => index,
        };
        self.history_index = Some(next);
        self.load(self.history[next].clone());
    }

    /// Walk one step towards newer history entries, restoring the draft past the newest.
    fn history_next(&mut self) {
        let Some(index) = self.history_index else {
            return;
        };
        if index == 0 {
            self.history_index = None;
            let draft = self.saved_draft.take().unwrap_or_default();
            self.load(draft);
        } else {
            let next = index - 1;
            self.history_index = Some(next);
            self.load(self.history[next].clone());
        }
    }

    /// Most recent history entry matching `query`, at or after `from`.
    fn find_match(&self, query: &str, from: usize) -> Option<usize> {
        if query.is_empty() {
            return None;
        }
        self.history
            .iter()
            .enumerate()
            .skip(from)
            .find(|(_, entry)| entry.contains(query))
            .map(|(index, _)| index)
    }

    fn start_search(&mut self) {
        self.search = Some(SearchState {
            query: String::new(),
            match_index: None,
            saved_buffer: self.buffer.clone(),
            saved_cursor: self.cursor,
        });
    }

    /// Step to the next older match for the current query.
    fn search_next(&mut self) {
        let Some(search) = self.search.as_ref() else {
            return;
        };
        let from = search.match_index.map(|index| index + 1).unwrap_or(0);
        let query = search.query.clone();
        if let Some(found) = self.find_match(&query, from) {
            if let Some(search) = self.search.as_mut() {
                search.match_index = Some(found);
            }
            self.load(self.history[found].clone());
        }
    }

    /// Re-run the search from the top after the query changed.
    fn search_refresh(&mut self) {
        let Some(search) = self.search.as_ref() else {
            return;
        };
        let query = search.query.clone();
        let found = self.find_match(&query, 0);
        if let Some(search) = self.search.as_mut() {
            search.match_index = found;
        }
        match found {
            Some(index) => self.load(self.history[index].clone()),
            None => {
                // No match: leave the buffer showing the pre-search text.
                let restored = self
                    .search
                    .as_ref()
                    .map(|s| s.saved_buffer.clone())
                    .unwrap_or_default();
                self.load(restored);
            }
        }
    }

    /// Leave search mode, keeping whatever match is in the buffer.
    fn accept_search(&mut self) {
        self.search = None;
        self.history_index = None;
        self.saved_draft = None;
    }

    /// Leave search mode, restoring the buffer exactly as it was before Ctrl+R.
    fn cancel_search(&mut self) {
        if let Some(search) = self.search.take() {
            self.buffer = search.saved_buffer;
            self.cursor = search.saved_cursor;
        }
    }

    /// Feed one key event to the editor and report what the driver should do.
    pub fn apply(&mut self, key: KeyEvent) -> EditorAction {
        if self.search.is_some() {
            return self.apply_in_search(key);
        }
        self.apply_in_edit(key)
    }

    fn apply_in_edit(&mut self, key: KeyEvent) -> EditorAction {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            KeyCode::Char('a') if ctrl => self.move_home(),
            KeyCode::Char('e') if ctrl => self.move_end(),
            KeyCode::Char('k') if ctrl => self.kill_to_end(),
            KeyCode::Char('u') if ctrl => self.kill_to_start(),
            KeyCode::Char('l') if ctrl => return EditorAction::Redraw,
            KeyCode::Char('r') if ctrl => {
                self.start_search();
                return EditorAction::Continue;
            }
            KeyCode::Char('g') if ctrl => {
                self.show_help = !self.show_help;
                return EditorAction::Continue;
            }
            KeyCode::Char('c') if ctrl => {
                return EditorAction::Finish(EditorOutcome::Cancelled);
            }
            KeyCode::Char(ch) => self.insert_char(ch),
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete(),
            KeyCode::Left => self.move_left(),
            KeyCode::Right => self.move_right(),
            KeyCode::Home => self.move_home(),
            KeyCode::End => self.move_end(),
            KeyCode::Up => self.history_prev(),
            KeyCode::Down => self.history_next(),
            KeyCode::Esc => return EditorAction::Finish(EditorOutcome::Cancelled),
            // Reserved: SPEC-03 makes Alt+Enter insert a line break. Swallow it
            // now rather than submitting, so the key never means "send" to
            // anyone who finds it early.
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {}
            KeyCode::Enter => {
                // An empty or whitespace-only buffer is not submittable; stay open.
                if self.buffer.trim().is_empty() {
                    return EditorAction::Continue;
                }
                return EditorAction::Finish(EditorOutcome::Submitted(self.buffer.clone()));
            }
            _ => {}
        }

        EditorAction::Continue
    }

    fn apply_in_search(&mut self, key: KeyEvent) -> EditorAction {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            KeyCode::Char('r') if ctrl => self.search_next(),
            KeyCode::Char('g') if ctrl => self.show_help = !self.show_help,
            KeyCode::Char('c') if ctrl => {
                return EditorAction::Finish(EditorOutcome::Cancelled);
            }
            KeyCode::Char(ch) => {
                if let Some(search) = self.search.as_mut() {
                    search.query.push(ch);
                }
                self.search_refresh();
            }
            KeyCode::Backspace => {
                if let Some(search) = self.search.as_mut() {
                    search.query.pop();
                }
                self.search_refresh();
            }
            KeyCode::Esc => self.cancel_search(),
            // Accepting the match returns to editing; the match stays in the buffer.
            KeyCode::Enter | KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                self.accept_search()
            }
            _ => {}
        }

        EditorAction::Continue
    }
}

/// Restores the terminal when the editor loop ends, however it ends.
///
/// Raw mode is acquired on construction and released on drop, so an early
/// return or a panic still leaves the terminal usable.
struct RawModeGuard;

impl RawModeGuard {
    fn acquire() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let mut out = io::stderr();
        let _ = execute!(out, cursor::Show);
        let _ = out.flush();
    }
}

/// The indicator drawn at the start of the prompt line.
const PROMPT_INDICATOR: &str = "sai> ";

/// Display width of a string, counting wide characters as two columns.
///
/// Kept local rather than pulling in `unicode-width`: the editor only needs to
/// place a cursor on one line, and the CJK/emoji ranges below cover the wide
/// characters that actually show up in prompts.
fn display_width(text: &str) -> usize {
    text.chars().map(char_width).sum()
}

fn char_width(ch: char) -> usize {
    let code = ch as u32;
    let wide = matches!(code,
        0x1100..=0x115F
        | 0x2E80..=0x303E
        | 0x3041..=0x33FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE6F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1F64F
        | 0x1F900..=0x1F9FF
        | 0x20000..=0x3FFFD
    );
    if wide {
        2
    } else {
        1
    }
}

/// Draw the prompt line (and the search status line when searching), then park
/// the terminal cursor at the logical cursor position.
/// The expanded key-binding panel shown by Ctrl+G.
const HELP_LINES: &[&str] = &[
    "  Move    \u{2190} \u{2192}   Home/End   ^A start   ^E end",
    "  Edit    Bksp   Del        ^K kill-to-end   ^U kill-to-start",
    "  Recall  \u{2191}\u{2193} history           ^R reverse search",
    "  Screen  ^L redraw",
    "  Send    Enter             Cancel  Esc / ^C",
];

/// Tracks how tall the prompt area was last drawn, so a redraw can erase
/// exactly what it wrote before — the area grows and shrinks as the help panel
/// is toggled and as search mode comes and goes.
#[derive(Default)]
struct PromptArea {
    rows: u16,
}

/// The prompt line itself, and the cursor column within it.
fn prompt_line(state: &EditorState) -> (String, usize) {
    if state.is_searching() {
        let query = state.search_query().unwrap_or_default();
        let status = if state.search_failed() {
            format!("(failed reverse-i-search)`{}': ", query)
        } else {
            format!("(reverse-i-search)`{}': ", query)
        };
        let column = display_width(&status) + display_width(state.buffer());
        return (format!("{}{}", status, state.buffer()), column);
    }

    // The cursor column is a display width, not a character count, so wide
    // glyphs before the cursor push it right by two columns each.
    let before: String = state.buffer().chars().take(state.cursor()).collect();
    let column = display_width(PROMPT_INDICATOR) + display_width(&before);
    (
        format!("{}{}", PROMPT_INDICATOR, state.buffer()),
        column,
    )
}

/// The dim lines drawn under the prompt: the hint, plus the help panel when open.
fn guide_lines(state: &EditorState) -> Vec<String> {
    let mut lines = Vec::new();
    if state.show_help() {
        lines.extend(HELP_LINES.iter().map(|line| line.to_string()));
    }
    lines.push(state.hint().to_string());
    lines
}

/// Draw the prompt area and park the terminal cursor on the prompt line.
fn render(state: &EditorState, out: &mut impl Write, area: &mut PromptArea) -> io::Result<()> {
    queue!(out, cursor::Hide)?;

    // Climb back to the top of whatever was drawn last time, then wipe it.
    if area.rows > 1 {
        queue!(out, cursor::MoveUp(area.rows - 1))?;
    }
    queue!(
        out,
        cursor::MoveToColumn(0),
        terminal::Clear(ClearType::FromCursorDown)
    )?;

    let (line, column) = prompt_line(state);
    write!(out, "{}", line)?;

    let guides = guide_lines(state);
    for guide in &guides {
        // Dimmed so the guidance never competes with the prompt itself.
        write!(out, "\r\n")?;
        queue!(out, SetForegroundColor(Color::DarkGrey))?;
        write!(out, "{}", guide)?;
        queue!(out, ResetColor)?;
    }

    // Writing the guide lines left the cursor below the prompt; come back up.
    if !guides.is_empty() {
        queue!(out, cursor::MoveUp(guides.len() as u16))?;
    }
    queue!(out, cursor::MoveToColumn(column as u16), cursor::Show)?;
    out.flush()?;

    area.rows = 1 + guides.len() as u16;
    Ok(())
}

/// Erase the prompt area on the way out, so the editor leaves no residue above
/// whatever the caller prints next.
fn clear_area(out: &mut impl Write, area: &PromptArea) -> io::Result<()> {
    if area.rows > 1 {
        queue!(out, cursor::MoveUp(area.rows - 1))?;
    }
    queue!(
        out,
        cursor::MoveToColumn(0),
        terminal::Clear(ClearType::FromCursorDown),
        cursor::Show
    )?;
    out.flush()
}

/// Run the interactive editor to completion.
///
/// Returns `Ok(None)` when the terminal cannot support raw mode, so the caller
/// can fall back to a plain single-line read rather than failing the run.
pub fn compose(prefill: Option<String>, history: Vec<String>) -> Result<Option<EditorOutcome>> {
    let guard = match RawModeGuard::acquire() {
        Ok(guard) => guard,
        Err(err) => {
            eprintln!(
                "Interactive editor unavailable ({}); falling back to single-line input.",
                err
            );
            return Ok(None);
        }
    };

    let mut state = EditorState::new(prefill, history);
    let mut out = io::stderr();
    let mut area = PromptArea::default();
    let outcome = run_loop(&mut state, &mut out, &mut area, || {
        event::read().context("Failed to read a key event from the terminal")
    });
    let _ = clear_area(&mut out, &area);
    drop(guard);
    outcome.map(Some)
}

/// The editor loop, with the event source injected so it can be driven by a
/// scripted sequence of key events in tests as well as by a real terminal.
fn run_loop<E>(
    state: &mut EditorState,
    out: &mut impl Write,
    area: &mut PromptArea,
    mut next_event: E,
) -> Result<EditorOutcome>
where
    E: FnMut() -> Result<Event>,
{
    render(state, out, area)?;

    loop {
        let event = next_event()?;

        let Event::Key(key) = event else {
            // Resizes and mouse events just need a redraw.
            render(state, out, area)?;
            continue;
        };

        // Windows reports both press and release; only act on press.
        if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
            continue;
        }

        match state.apply(key) {
            EditorAction::Continue => render(state, out, area)?,
            EditorAction::Redraw => {
                queue!(out, terminal::Clear(ClearType::All), cursor::MoveTo(0, 0))?;
                // The wipe took the old area with it; nothing to climb back over.
                area.rows = 1;
                render(state, out, area)?;
            }
            EditorAction::Finish(outcome) => return Ok(outcome),
        }
    }
}

/// Read a single line of prompt text from `reader`, for non-interactive contexts.
///
/// Returns an error when no prompt is available at all, so the caller exits
/// non-zero with an explicit message rather than silently doing nothing.
pub fn read_plain_line(reader: &mut dyn BufRead, is_tty: bool) -> Result<String> {
    if is_tty {
        eprint!("{}", PROMPT_INDICATOR);
        io::stderr().flush().ok();
    }

    let mut buf = String::new();
    let read = reader
        .read_line(&mut buf)
        .context("Failed to read the prompt from standard input")?;

    if read == 0 || buf.trim().is_empty() {
        return Err(anyhow!(
            "No prompt provided. Pass one as an argument, pipe it on standard input, or run sai in a terminal to compose one interactively."
        ));
    }

    Ok(buf.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(ch: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(ch), KeyModifiers::CONTROL)
    }

    fn type_text(state: &mut EditorState, text: &str) {
        for ch in text.chars() {
            state.apply(key(KeyCode::Char(ch)));
        }
    }

    fn editor(text: &str) -> EditorState {
        EditorState::new(Some(text.to_string()), Vec::new())
    }

    #[test]
    fn prefill_places_the_cursor_at_the_end() {
        let state = editor("find large files");
        assert_eq!(state.buffer(), "find large files");
        assert_eq!(state.cursor(), 16);
    }

    #[test]
    fn typing_inserts_at_the_cursor() {
        let mut state = EditorState::new(None, Vec::new());
        type_text(&mut state, "find");
        assert_eq!(state.buffer(), "find");
        assert_eq!(state.cursor(), 4);
    }

    #[test]
    fn editing_in_the_middle_of_the_buffer() {
        let mut state = editor("find large fils");
        state.apply(key(KeyCode::Left));
        state.apply(key(KeyCode::Char('e')));
        assert_eq!(state.buffer(), "find large files");
        assert_eq!(state.cursor(), 15);
    }

    #[test]
    fn backspace_at_the_start_is_a_no_op() {
        let mut state = editor("find");
        state.apply(ctrl('a'));
        state.apply(key(KeyCode::Backspace));
        assert_eq!(state.buffer(), "find");
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn backspace_removes_the_previous_character() {
        let mut state = editor("finds");
        state.apply(key(KeyCode::Backspace));
        assert_eq!(state.buffer(), "find");
        assert_eq!(state.cursor(), 4);
    }

    #[test]
    fn delete_removes_the_character_at_the_cursor() {
        let mut state = editor("finds");
        state.apply(ctrl('a'));
        state.apply(key(KeyCode::Delete));
        assert_eq!(state.buffer(), "inds");
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn delete_at_the_end_is_a_no_op() {
        let mut state = editor("find");
        state.apply(key(KeyCode::Delete));
        assert_eq!(state.buffer(), "find");
    }

    #[test]
    fn cursor_movement_clamps_at_both_ends() {
        let mut state = editor("ab");
        for _ in 0..5 {
            state.apply(key(KeyCode::Left));
        }
        assert_eq!(state.cursor(), 0);
        for _ in 0..5 {
            state.apply(key(KeyCode::Right));
        }
        assert_eq!(state.cursor(), 2);
    }

    #[test]
    fn home_and_end_jump_to_the_edges() {
        let mut state = editor("find large files");
        state.apply(key(KeyCode::Home));
        assert_eq!(state.cursor(), 0);
        state.apply(key(KeyCode::End));
        assert_eq!(state.cursor(), 16);
    }

    #[test]
    fn ctrl_a_and_ctrl_e_jump_to_the_edges() {
        let mut state = editor("find large files");
        state.apply(ctrl('a'));
        assert_eq!(state.cursor(), 0);
        state.apply(ctrl('e'));
        assert_eq!(state.cursor(), 16);
    }

    #[test]
    fn ctrl_k_kills_to_end_of_line() {
        let mut state = editor("list all json files");
        state.apply(ctrl('a'));
        for _ in 0..8 {
            state.apply(key(KeyCode::Right));
        }
        state.apply(ctrl('k'));
        assert_eq!(state.buffer(), "list all");
        assert_eq!(state.cursor(), 8);
    }

    #[test]
    fn ctrl_u_kills_to_start_of_line() {
        let mut state = editor("list all json files");
        state.apply(ctrl('a'));
        for _ in 0..8 {
            state.apply(key(KeyCode::Right));
        }
        state.apply(ctrl('u'));
        assert_eq!(state.buffer(), " json files");
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn ctrl_l_requests_redraw_without_touching_the_buffer() {
        let mut state = editor("find large files");
        state.apply(ctrl('a'));
        state.apply(key(KeyCode::Right));
        let action = state.apply(ctrl('l'));
        assert_eq!(action, EditorAction::Redraw);
        assert_eq!(state.buffer(), "find large files");
        assert_eq!(state.cursor(), 1);
    }

    #[test]
    fn enter_submits_the_buffer() {
        let mut state = editor("find large files");
        let action = state.apply(key(KeyCode::Enter));
        assert_eq!(
            action,
            EditorAction::Finish(EditorOutcome::Submitted("find large files".to_string()))
        );
    }

    #[test]
    fn enter_on_an_empty_buffer_keeps_the_editor_open() {
        let mut state = EditorState::new(None, Vec::new());
        assert_eq!(state.apply(key(KeyCode::Enter)), EditorAction::Continue);
    }

    #[test]
    fn enter_on_a_whitespace_only_buffer_keeps_the_editor_open() {
        let mut state = editor("   \t ");
        assert_eq!(state.apply(key(KeyCode::Enter)), EditorAction::Continue);
    }

    #[test]
    fn esc_cancels() {
        let mut state = editor("find large files");
        assert_eq!(
            state.apply(key(KeyCode::Esc)),
            EditorAction::Finish(EditorOutcome::Cancelled)
        );
    }

    #[test]
    fn ctrl_c_cancels() {
        let mut state = editor("find large files");
        assert_eq!(
            state.apply(ctrl('c')),
            EditorAction::Finish(EditorOutcome::Cancelled)
        );
    }

    #[test]
    fn multi_byte_text_survives_editing() {
        let mut state = EditorState::new(None, Vec::new());
        type_text(&mut state, "café 日本語");
        assert_eq!(state.buffer(), "café 日本語");
        assert_eq!(state.cursor(), 8);

        // Delete the last CJK character, then rebuild it.
        state.apply(key(KeyCode::Backspace));
        assert_eq!(state.buffer(), "café 日本");
        state.apply(key(KeyCode::Char('語')));
        assert_eq!(state.buffer(), "café 日本語");

        // Splice in the middle of a multi-byte run.
        state.apply(ctrl('a'));
        for _ in 0..3 {
            state.apply(key(KeyCode::Right));
        }
        state.apply(key(KeyCode::Char('X')));
        assert_eq!(state.buffer(), "cafXé 日本語");
        assert_eq!(state.cursor(), 4);
    }

    #[test]
    fn ctrl_u_and_ctrl_k_are_safe_on_multi_byte_text() {
        let mut state = editor("日本語 files");
        state.apply(ctrl('a'));
        for _ in 0..3 {
            state.apply(key(KeyCode::Right));
        }
        state.apply(ctrl('k'));
        assert_eq!(state.buffer(), "日本語");

        let mut state = editor("日本語 files");
        state.apply(ctrl('a'));
        for _ in 0..4 {
            state.apply(key(KeyCode::Right));
        }
        state.apply(ctrl('u'));
        assert_eq!(state.buffer(), "files");
    }

    #[test]
    fn display_width_counts_wide_characters_as_two_columns() {
        assert_eq!(display_width("find"), 4);
        assert_eq!(display_width("日本語"), 6);
        assert_eq!(display_width("café"), 4);
        assert_eq!(display_width("日本語 files"), 12);
    }

    #[test]
    fn plain_read_returns_the_piped_line() {
        let mut reader = std::io::Cursor::new(b"list json files\n".to_vec());
        let prompt = read_plain_line(&mut reader, false).unwrap();
        assert_eq!(prompt, "list json files");
    }

    #[test]
    fn plain_read_errors_on_immediate_eof() {
        let mut reader = std::io::Cursor::new(Vec::<u8>::new());
        let err = read_plain_line(&mut reader, false).expect_err("EOF must be an error");
        assert!(err.to_string().contains("No prompt provided"));
    }

    #[test]
    fn plain_read_errors_on_a_blank_line() {
        let mut reader = std::io::Cursor::new(b"   \n".to_vec());
        let err = read_plain_line(&mut reader, false).expect_err("blank input must be an error");
        assert!(err.to_string().contains("No prompt provided"));
    }

    /// Drive the real editor loop with a scripted key sequence, rendering into
    /// a buffer instead of a terminal.
    fn drive(prefill: Option<&str>, history: &[&str], keys: Vec<KeyEvent>) -> EditorOutcome {
        let mut state = EditorState::new(
            prefill.map(|s| s.to_string()),
            history.iter().map(|s| s.to_string()).collect(),
        );
        let mut out = Vec::new();
        let mut area = PromptArea::default();
        let mut keys = keys.into_iter();
        run_loop(&mut state, &mut out, &mut area, || {
            keys.next()
                .map(Event::Key)
                .ok_or_else(|| anyhow!("ran out of scripted key events"))
        })
        .expect("the scripted session should finish")
    }

    #[test]
    fn loop_submits_a_composed_prompt() {
        let mut keys: Vec<KeyEvent> = "find large files"
            .chars()
            .map(|ch| key(KeyCode::Char(ch)))
            .collect();
        keys.push(key(KeyCode::Enter));

        assert_eq!(
            drive(None, &[], keys),
            EditorOutcome::Submitted("find large files".to_string())
        );
    }

    #[test]
    fn loop_ignores_empty_submissions_then_submits() {
        let mut keys = vec![key(KeyCode::Enter), key(KeyCode::Enter)];
        keys.extend("ls".chars().map(|ch| key(KeyCode::Char(ch))));
        keys.push(key(KeyCode::Enter));

        assert_eq!(drive(None, &[], keys), EditorOutcome::Submitted("ls".to_string()));
    }

    #[test]
    fn loop_handles_redraw_and_cancel() {
        let keys = vec![ctrl('l'), key(KeyCode::Esc)];
        assert_eq!(drive(Some("draft"), &[], keys), EditorOutcome::Cancelled);
    }

    #[test]
    fn loop_edits_a_recalled_prompt_before_submitting() {
        // Recall the newest entry, trim "100MB", type "1GB", submit.
        let mut keys = vec![key(KeyCode::Up)];
        for _ in 0..5 {
            keys.push(key(KeyCode::Backspace));
        }
        keys.extend("1GB".chars().map(|ch| key(KeyCode::Char(ch))));
        keys.push(key(KeyCode::Enter));

        assert_eq!(
            drive(None, &["find files larger than 100MB"], keys),
            EditorOutcome::Submitted("find files larger than 1GB".to_string())
        );
    }

    #[test]
    fn loop_ignores_key_release_events() {
        let press = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        let release = KeyEvent::new_with_kind(
            KeyCode::Char('y'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        );
        let keys = vec![press, release, key(KeyCode::Enter)];
        assert_eq!(drive(None, &[], keys), EditorOutcome::Submitted("x".to_string()));
    }

    // --- History navigation -------------------------------------------------

    fn with_history(history: &[&str]) -> EditorState {
        EditorState::new(None, history.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn up_recalls_the_most_recent_prompt() {
        let mut state = with_history(&["newest", "middle", "oldest"]);
        state.apply(key(KeyCode::Up));
        assert_eq!(state.buffer(), "newest");
        assert_eq!(state.cursor(), 6, "the cursor should sit at the end");
    }

    #[test]
    fn up_walks_back_and_clamps_at_the_oldest_entry() {
        let mut state = with_history(&["newest", "middle", "oldest"]);
        state.apply(key(KeyCode::Up));
        state.apply(key(KeyCode::Up));
        assert_eq!(state.buffer(), "middle");
        state.apply(key(KeyCode::Up));
        assert_eq!(state.buffer(), "oldest");
        state.apply(key(KeyCode::Up));
        assert_eq!(state.buffer(), "oldest", "should stay on the oldest entry");
    }

    #[test]
    fn down_past_the_newest_restores_the_draft() {
        let mut state = with_history(&["newest", "older"]);
        type_text(&mut state, "count rec");
        state.apply(key(KeyCode::Up));
        state.apply(key(KeyCode::Up));
        assert_eq!(state.buffer(), "older");
        state.apply(key(KeyCode::Down));
        assert_eq!(state.buffer(), "newest");
        state.apply(key(KeyCode::Down));
        assert_eq!(state.buffer(), "count rec");
        assert_eq!(state.cursor(), 9);
    }

    #[test]
    fn down_without_navigating_is_a_no_op() {
        let mut state = with_history(&["newest"]);
        type_text(&mut state, "draft");
        state.apply(key(KeyCode::Down));
        assert_eq!(state.buffer(), "draft");
    }

    #[test]
    fn navigation_with_empty_history_is_a_no_op() {
        let mut state = EditorState::new(None, Vec::new());
        type_text(&mut state, "draft");
        state.apply(key(KeyCode::Up));
        assert_eq!(state.buffer(), "draft");
        state.apply(key(KeyCode::Down));
        assert_eq!(state.buffer(), "draft");
    }

    #[test]
    fn a_recalled_prompt_is_editable() {
        let mut state = with_history(&["find files larger than 100MB"]);
        state.apply(key(KeyCode::Up));
        for _ in 0..5 {
            state.apply(key(KeyCode::Backspace));
        }
        type_text(&mut state, "1GB");
        assert_eq!(state.buffer(), "find files larger than 1GB");

        let action = state.apply(key(KeyCode::Enter));
        assert_eq!(
            action,
            EditorAction::Finish(EditorOutcome::Submitted(
                "find files larger than 1GB".to_string()
            ))
        );
    }

    // --- Reverse search -----------------------------------------------------

    #[test]
    fn ctrl_r_finds_the_most_recent_match() {
        let mut state = with_history(&["count records", "list json files", "find large files"]);
        state.apply(ctrl('r'));
        assert!(state.is_searching());
        type_text(&mut state, "json");
        assert_eq!(state.search_match(), Some("list json files"));
        assert_eq!(state.buffer(), "list json files");
        assert_eq!(state.search_query(), Some("json"));
    }

    #[test]
    fn ctrl_r_cycles_to_the_next_older_match() {
        let mut state = with_history(&["files newest", "unrelated", "files oldest"]);
        state.apply(ctrl('r'));
        type_text(&mut state, "files");
        assert_eq!(state.search_match(), Some("files newest"));
        state.apply(ctrl('r'));
        assert_eq!(state.search_match(), Some("files oldest"));
        // No older match remains, so the current one stays put.
        state.apply(ctrl('r'));
        assert_eq!(state.search_match(), Some("files oldest"));
    }

    #[test]
    fn accepting_a_match_returns_it_to_the_buffer_for_editing() {
        let mut state = with_history(&["find files larger than 100MB"]);
        state.apply(ctrl('r'));
        type_text(&mut state, "larger");
        state.apply(key(KeyCode::Enter));

        assert!(!state.is_searching());
        assert_eq!(state.buffer(), "find files larger than 100MB");

        for _ in 0..5 {
            state.apply(key(KeyCode::Backspace));
        }
        type_text(&mut state, "1GB");
        assert_eq!(state.buffer(), "find files larger than 1GB");
    }

    #[test]
    fn a_failed_search_reports_no_match_and_keeps_the_query() {
        let mut state = with_history(&["find large files"]);
        state.apply(ctrl('r'));
        type_text(&mut state, "zzz");
        assert!(state.search_failed());
        assert_eq!(state.search_match(), None);
        assert_eq!(state.search_query(), Some("zzz"));

        // Backspacing back to a matching query recovers the match.
        state.apply(key(KeyCode::Backspace));
        state.apply(key(KeyCode::Backspace));
        state.apply(key(KeyCode::Backspace));
        type_text(&mut state, "large");
        assert!(!state.search_failed());
        assert_eq!(state.search_match(), Some("find large files"));
    }

    #[test]
    fn cancelling_search_restores_the_pre_search_buffer() {
        let mut state = with_history(&["find large files"]);
        type_text(&mut state, "my draft");
        state.apply(ctrl('a'));
        state.apply(key(KeyCode::Right));
        let cursor_before = state.cursor();

        state.apply(ctrl('r'));
        type_text(&mut state, "large");
        assert_eq!(state.buffer(), "find large files");

        state.apply(key(KeyCode::Esc));
        assert!(!state.is_searching());
        assert_eq!(state.buffer(), "my draft");
        assert_eq!(state.cursor(), cursor_before);
    }

    #[test]
    fn ctrl_c_cancels_the_whole_editor_from_search_mode() {
        let mut state = with_history(&["find large files"]);
        state.apply(ctrl('r'));
        assert_eq!(
            state.apply(ctrl('c')),
            EditorAction::Finish(EditorOutcome::Cancelled)
        );
    }

    #[test]
    fn search_over_a_full_history_is_fast() {
        let history: Vec<String> = (0..5_000).map(|i| format!("prompt number {}", i)).collect();
        let mut state = EditorState::new(None, history);

        let start = std::time::Instant::now();
        state.apply(ctrl('r'));
        type_text(&mut state, "number 4999");
        let elapsed = start.elapsed();

        assert_eq!(state.search_match(), Some("prompt number 4999"));
        assert!(
            elapsed < std::time::Duration::from_millis(100),
            "reverse search over a full history took {:?}",
            elapsed
        );
    }

    // --- On-screen help -----------------------------------------------------

    #[test]
    fn the_hint_line_names_the_essential_keys() {
        let state = EditorState::new(None, Vec::new());
        let hint = state.hint();
        for key in ["history", "^R", "Enter", "Esc", "^G"] {
            assert!(hint.contains(key), "hint should mention {}: {}", key, hint);
        }
    }

    #[test]
    fn the_hint_line_follows_the_mode() {
        let mut state = with_history(&["find large files"]);
        state.apply(ctrl('r'));
        let hint = state.hint();
        assert!(hint.contains("next match"), "search hint should say what ^R does now: {}", hint);
        assert!(hint.contains("accept"), "search hint should say how to accept: {}", hint);
    }

    #[test]
    fn ctrl_g_toggles_the_key_panel() {
        let mut state = EditorState::new(None, Vec::new());
        assert!(!state.show_help());

        state.apply(ctrl('g'));
        assert!(state.show_help());
        assert_eq!(state.hint(), "^G hide keys");

        state.apply(ctrl('g'));
        assert!(!state.show_help());
    }

    #[test]
    fn ctrl_g_toggles_the_key_panel_during_search() {
        let mut state = with_history(&["find large files"]);
        state.apply(ctrl('r'));
        type_text(&mut state, "large");

        state.apply(ctrl('g'));
        assert!(state.show_help());
        // Toggling help must not disturb the search itself.
        assert!(state.is_searching());
        assert_eq!(state.search_match(), Some("find large files"));
    }

    #[test]
    fn ctrl_g_does_not_alter_the_buffer() {
        let mut state = editor("find large files");
        state.apply(ctrl('a'));
        state.apply(key(KeyCode::Right));
        state.apply(ctrl('g'));
        assert_eq!(state.buffer(), "find large files");
        assert_eq!(state.cursor(), 1);
    }

    #[test]
    fn the_key_panel_covers_every_binding_the_editor_implements() {
        let panel = HELP_LINES.join(" ");
        for binding in ["^A", "^E", "^K", "^U", "^L", "^R", "Home/End", "Enter", "Esc"] {
            assert!(
                panel.contains(binding),
                "the key panel should document {}",
                binding
            );
        }
    }

    #[test]
    fn rendering_tracks_how_tall_the_prompt_area_is() {
        let mut state = EditorState::new(None, Vec::new());
        let mut out = Vec::new();
        let mut area = PromptArea::default();

        // Prompt plus the hint line.
        render(&state, &mut out, &mut area).unwrap();
        assert_eq!(area.rows, 2);

        // Prompt, the help panel, then the hint line.
        state.apply(ctrl('g'));
        render(&state, &mut out, &mut area).unwrap();
        assert_eq!(area.rows, 2 + HELP_LINES.len() as u16);

        // Closing the panel shrinks the area back.
        state.apply(ctrl('g'));
        render(&state, &mut out, &mut area).unwrap();
        assert_eq!(area.rows, 2);
    }

    #[test]
    fn rendering_draws_the_prompt_and_the_hint() {
        let state = editor("find large files");
        let mut out = Vec::new();
        let mut area = PromptArea::default();
        render(&state, &mut out, &mut area).unwrap();

        let drawn = String::from_utf8(out).unwrap();
        assert!(drawn.contains("sai> find large files"));
        assert!(drawn.contains("^G keys"));
    }

    #[test]
    fn alt_enter_is_reserved_and_does_not_submit() {
        let mut state = editor("find large files");
        let action = state.apply(KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT));
        assert_eq!(action, EditorAction::Continue, "Alt+Enter must not submit");
        assert_eq!(state.buffer(), "find large files");

        // Plain Enter still submits.
        assert_eq!(
            state.apply(key(KeyCode::Enter)),
            EditorAction::Finish(EditorOutcome::Submitted("find large files".to_string()))
        );
    }
}
