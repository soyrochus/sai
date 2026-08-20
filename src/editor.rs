//! Interactive mini editor for composing natural language prompts.
//!
//! The editing logic lives in [`EditorState`], a pure state machine driven by
//! [`crossterm`] key events. It never touches the terminal, so every editing,
//! shortcut, navigation and search behaviour can be unit tested by feeding it
//! `KeyEvent`s directly. The terminal driver sits in `driver.rs`-style code at
//! the bottom of this module and only renders what the state reports.

use anyhow::{Context, Result, anyhow};
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
            "\u{2191}\u{2193} move/history \u{b7} ^R search \u{b7} Alt+Enter/^J line break \u{b7} Enter send \u{b7} Esc cancel \u{b7} ^G keys"
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

    /// Character-index bounds for every logical line. The end excludes the
    /// line break, so a cursor on that boundary belongs to the line it ends.
    fn line_bounds(&self) -> Vec<(usize, usize)> {
        let mut bounds = Vec::new();
        let mut start = 0;

        for (index, ch) in self.buffer.chars().enumerate() {
            if ch == '\n' {
                bounds.push((start, index));
                start = index + 1;
            }
        }
        bounds.push((start, self.char_count()));
        bounds
    }

    fn cursor_line(&self) -> usize {
        self.line_bounds()
            .iter()
            .position(|&(_, end)| self.cursor <= end)
            .unwrap_or_else(|| self.line_bounds().len().saturating_sub(1))
    }

    fn cursor_column(&self) -> usize {
        let bounds = self.line_bounds();
        let line = bounds
            .iter()
            .position(|&(_, end)| self.cursor() <= end)
            .unwrap_or_else(|| bounds.len().saturating_sub(1));
        self.cursor().saturating_sub(bounds[line].0)
    }

    fn current_line_bounds(&self) -> (usize, usize) {
        let bounds = self.line_bounds();
        bounds[self.cursor_line()]
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
        let (_, line_end) = self.current_line_bounds();
        let start = self.byte_offset(self.cursor);
        let end = self.byte_offset(line_end);
        self.buffer.replace_range(start..end, "");
    }

    fn kill_to_start(&mut self) {
        let (line_start, _) = self.current_line_bounds();
        let start = self.byte_offset(line_start);
        let end = self.byte_offset(self.cursor);
        self.buffer.replace_range(start..end, "");
        self.cursor = line_start;
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
        self.cursor = self.current_line_bounds().0;
    }

    fn move_end(&mut self) {
        self.cursor = self.current_line_bounds().1;
    }

    fn move_up(&mut self) {
        let line = self.cursor_line();
        if line == 0 {
            return;
        }
        let column = self.cursor_column();
        let (start, end) = self.line_bounds()[line - 1];
        self.cursor = start + column.min(end - start);
    }

    fn move_down(&mut self) {
        let bounds = self.line_bounds();
        let line = self.cursor_line();
        let Some(&(start, end)) = bounds.get(line + 1) else {
            return;
        };
        self.cursor = start + self.cursor_column().min(end - start);
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
            KeyCode::Char('j') if ctrl => self.insert_char('\n'),
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
            KeyCode::Up if self.cursor_line() > 0 => self.move_up(),
            KeyCode::Up => self.history_prev(),
            KeyCode::Down if self.cursor_line() + 1 < self.line_bounds().len() => self.move_down(),
            KeyCode::Down => self.history_next(),
            KeyCode::Esc => return EditorAction::Finish(EditorOutcome::Cancelled),
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => self.insert_char('\n'),
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
/// Keeps continuation text aligned with the first prompt row.
const CONTINUATION_INDICATOR: &str = "  |  ";
const TRUNCATION_ROW: &str = "  …  prompt rows omitted";

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
    if wide { 2 } else { 1 }
}

/// The expanded key-binding panel shown by Ctrl+G.
const HELP_LINES: &[&str] = &[
    "  Move    \u{2190} \u{2192} \u{2191} \u{2193}   Home/End   ^A start   ^E end",
    "  Edit    Bksp   Del        ^K kill-to-end   ^U kill-to-start",
    "  Compose Alt+Enter / ^J line break",
    "  Recall  \u{2191}\u{2193} at buffer edges   ^R reverse search",
    "  Screen  ^L redraw",
    "  Send    Enter             Cancel  Esc / ^C",
];

/// Rendered prompt rows and the cursor's row/display-column within them.
fn prompt_rows(state: &EditorState) -> (Vec<String>, (usize, usize)) {
    if state.is_searching() {
        let query = state.search_query().unwrap_or_default();
        let status = if state.search_failed() {
            format!("(failed reverse-i-search)`{}': ", query)
        } else {
            format!("(reverse-i-search)`{}': ", query)
        };
        let preview = state.buffer().replace('\n', " ↵ ");
        let column = display_width(&status) + display_width(&preview);
        return (vec![format!("{}{}", status, preview)], (0, column));
    }

    let rows = state
        .buffer()
        .split('\n')
        .enumerate()
        .map(|(index, line)| {
            let indicator = if index == 0 {
                PROMPT_INDICATOR
            } else {
                CONTINUATION_INDICATOR
            };
            format!("{}{}", indicator, line)
        })
        .collect();

    let cursor_row = state.cursor_line();
    let cursor_line = state
        .buffer()
        .split('\n')
        .nth(cursor_row)
        .unwrap_or_default();
    let before: String = cursor_line.chars().take(state.cursor_column()).collect();
    let cursor_column = display_width(PROMPT_INDICATOR) + display_width(&before);
    (rows, (cursor_row, cursor_column))
}

/// The dim lines drawn under the prompt: the hint, plus the help panel when open.
fn guide_lines(state: &EditorState) -> Vec<String> {
    let mut lines = vec![format!(
        "line {}/{} · {} chars",
        state.cursor_line() + 1,
        state.line_bounds().len(),
        state.char_count()
    )];
    if state.show_help() {
        lines.extend(HELP_LINES.iter().map(|line| line.to_string()));
    }
    lines.push(state.hint().to_string());
    lines
}

/// Keep a prompt taller than the available rows anchored and its cursor visible.
fn cap_prompt_rows(
    rows: Vec<String>,
    cursor: (usize, usize),
    max_rows: usize,
) -> (Vec<String>, (usize, usize)) {
    let max_rows = max_rows.max(1);
    if rows.len() <= max_rows {
        return (rows, cursor);
    }

    if max_rows == 1 {
        let mut row = rows[cursor.0].clone();
        row.push_str("  …");
        return (vec![row], (0, cursor.1));
    }

    let content_rows = max_rows - 1;
    if cursor.0 < content_rows {
        let mut visible = rows[..content_rows].to_vec();
        visible.push(TRUNCATION_ROW.to_string());
        return (visible, cursor);
    }

    if cursor.0 >= rows.len() - content_rows {
        let start = rows.len() - content_rows;
        let mut visible = vec![TRUNCATION_ROW.to_string()];
        visible.extend_from_slice(&rows[start..]);
        return (visible, (cursor.0 - start + 1, cursor.1));
    }

    if max_rows == 2 {
        let mut row = rows[cursor.0].clone();
        row.push_str("  …");
        return (vec![TRUNCATION_ROW.to_string(), row], (1, cursor.1));
    }

    let middle_rows = max_rows - 2;
    let start = cursor.0.saturating_sub(middle_rows / 2);
    let mut visible = vec![TRUNCATION_ROW.to_string()];
    visible.extend_from_slice(&rows[start..start + middle_rows]);
    visible.push(TRUNCATION_ROW.to_string());
    (visible, (cursor.0 - start + 1, cursor.1))
}

/// Draw the prompt area and park the terminal cursor on its logical row.
///
/// The top-row anchor is saved independently of the visible cursor. Each render
/// restores it, reserves enough terminal rows for the full area, then saves the
/// possibly scroll-adjusted anchor before drawing. Every drawing descent is
/// undone before parking on the logical cursor row. This keeps repeated renders
/// from drifting even when the prompt starts near the bottom of the viewport.
fn render(state: &EditorState, out: &mut impl Write) -> io::Result<()> {
    let terminal_rows = terminal::size()
        .map(|(_, rows)| rows as usize)
        .unwrap_or(usize::MAX);
    render_with_height(state, out, terminal_rows)
}

fn render_with_height(
    state: &EditorState,
    out: &mut impl Write,
    terminal_rows: usize,
) -> io::Result<()> {
    let guides = guide_lines(state);
    let max_prompt_rows = terminal_rows.saturating_sub(guides.len()).max(1);
    let (rows, (cursor_row, cursor_column)) = prompt_rows(state);
    let (rows, (cursor_row, cursor_column)) =
        cap_prompt_rows(rows, (cursor_row, cursor_column), max_prompt_rows);

    queue!(
        out,
        cursor::RestorePosition,
        cursor::Hide,
        cursor::MoveToColumn(0),
        terminal::Clear(ClearType::FromCursorDown)
    )?;

    // Drawing below a cursor near the terminal's bottom scrolls the viewport.
    // Reserve the full area first, then save the anchor after any such scroll,
    // so the next keypress restores to the same visible prompt row.
    let area_rows = rows.len() + guides.len();
    for _ in 1..area_rows {
        write!(out, "\r\n")?;
    }
    if area_rows > 1 {
        queue!(out, cursor::MoveUp((area_rows - 1) as u16))?;
    }
    queue!(out, cursor::MoveToColumn(0), cursor::SavePosition)?;

    write!(out, "{}", rows[0])?;
    for row in rows.iter().skip(1) {
        write!(out, "\r\n{}", row)?;
    }
    for guide in &guides {
        // Dimmed so the guidance never competes with the prompt itself.
        write!(out, "\r\n")?;
        queue!(out, SetForegroundColor(Color::DarkGrey))?;
        write!(out, "{}", guide)?;
        queue!(out, ResetColor)?;
    }

    // Undo every row descended while drawing, then park on the logical row.
    let descended = rows.len().saturating_sub(1) + guides.len();
    if descended > 0 {
        queue!(out, cursor::MoveUp(descended as u16))?;
    }
    if cursor_row > 0 {
        queue!(out, cursor::MoveDown(cursor_row as u16))?;
    }
    queue!(
        out,
        cursor::MoveToColumn(cursor_column as u16),
        cursor::Show
    )?;
    out.flush()?;
    Ok(())
}

/// Erase the prompt area on the way out, so the editor leaves no residue above
/// whatever the caller prints next.
fn clear_area(out: &mut impl Write) -> io::Result<()> {
    // Restore the saved top-row anchor before erasing the variable-height area.
    queue!(
        out,
        cursor::RestorePosition,
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
    let outcome = run_loop(&mut state, &mut out, || {
        event::read().context("Failed to read a key event from the terminal")
    });
    let _ = clear_area(&mut out);
    drop(guard);
    outcome.map(Some)
}

/// The editor loop, with the event source injected so it can be driven by a
/// scripted sequence of key events in tests as well as by a real terminal.
fn run_loop<E>(
    state: &mut EditorState,
    out: &mut impl Write,
    mut next_event: E,
) -> Result<EditorOutcome>
where
    E: FnMut() -> Result<Event>,
{
    queue!(out, cursor::SavePosition)?;
    render(state, out)?;

    loop {
        let event = next_event()?;

        let Event::Key(key) = event else {
            // Resizes and mouse events just need a redraw.
            render(state, out)?;
            continue;
        };

        // Windows reports both press and release; only act on press.
        if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
            continue;
        }

        match state.apply(key) {
            EditorAction::Continue => render(state, out)?,
            EditorAction::Redraw => {
                queue!(
                    out,
                    terminal::Clear(ClearType::All),
                    cursor::MoveTo(0, 0),
                    cursor::SavePosition
                )?;
                render(state, out)?;
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
    fn line_helpers_cover_empty_single_and_unicode_buffers() {
        let empty = editor("");
        assert_eq!(empty.line_bounds(), vec![(0, 0)]);
        assert_eq!(empty.cursor_line(), 0);
        assert_eq!(empty.cursor_column(), 0);

        let single = editor("café 日本");
        assert_eq!(single.line_bounds(), vec![(0, 7)]);
        assert_eq!(single.cursor_line(), 0);
        assert_eq!(single.cursor_column(), 7);

        let wide = editor("日本\ncafé");
        assert_eq!(wide.line_bounds(), vec![(0, 2), (3, 7)]);
        assert_eq!(wide.cursor_line(), 1);
        assert_eq!(wide.cursor_column(), 4);
    }

    #[test]
    fn line_helpers_assign_each_newline_boundary_to_the_line_it_ends() {
        let mut trailing = editor("ab\n");
        assert_eq!(trailing.line_bounds(), vec![(0, 2), (3, 3)]);
        trailing.cursor = 2;
        assert_eq!((trailing.cursor_line(), trailing.cursor_column()), (0, 2));
        trailing.cursor = 3;
        assert_eq!((trailing.cursor_line(), trailing.cursor_column()), (1, 0));

        let mut consecutive = editor("a\n\n界");
        assert_eq!(consecutive.line_bounds(), vec![(0, 1), (2, 2), (3, 4)]);
        for (cursor, expected) in [(1, (0, 1)), (2, (1, 0)), (3, (2, 0)), (4, (2, 1))] {
            consecutive.cursor = cursor;
            assert_eq!(
                (consecutive.cursor_line(), consecutive.cursor_column()),
                expected,
                "cursor {cursor}"
            );
        }
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
    fn backspace_and_delete_join_lines_at_the_break() {
        let mut backspace = editor("first\nsecond");
        backspace.cursor = 6;
        backspace.apply(key(KeyCode::Backspace));
        assert_eq!(backspace.buffer(), "firstsecond");
        assert_eq!(backspace.cursor(), 5);

        let mut delete = editor("first\nsecond");
        delete.cursor = 5;
        delete.apply(key(KeyCode::Delete));
        assert_eq!(delete.buffer(), "firstsecond");
        assert_eq!(delete.cursor(), 5);
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
    fn horizontal_movement_crosses_line_breaks() {
        let mut left = editor("first\nsecond");
        left.cursor = 6;
        left.apply(key(KeyCode::Left));
        assert_eq!(left.cursor(), 5);
        assert_eq!((left.cursor_line(), left.cursor_column()), (0, 5));

        let mut right = editor("first\nsecond");
        right.cursor = 5;
        right.apply(key(KeyCode::Right));
        assert_eq!(right.cursor(), 6);
        assert_eq!((right.cursor_line(), right.cursor_column()), (1, 0));
    }

    #[test]
    fn vertical_movement_preserves_character_column_and_clamps() {
        let mut state = editor("abcdefgh\n日本語abcdef\nxy");
        state.cursor = 9 + 6;
        state.apply(key(KeyCode::Up));
        assert_eq!((state.cursor_line(), state.cursor_column()), (0, 6));

        state.apply(key(KeyCode::Down));
        assert_eq!((state.cursor_line(), state.cursor_column()), (1, 6));
        state.apply(key(KeyCode::Down));
        assert_eq!((state.cursor_line(), state.cursor_column()), (2, 2));
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
    fn home_end_ctrl_a_and_ctrl_e_are_line_relative() {
        for (home, end) in [
            (key(KeyCode::Home), key(KeyCode::End)),
            (ctrl('a'), ctrl('e')),
        ] {
            let mut state = editor("first\nsecond\nthird");
            state.cursor = 9;
            state.apply(home);
            assert_eq!(state.cursor(), 6, "line two starts after the first break");
            state.apply(end);
            assert_eq!(state.cursor(), 12, "line two ends before the next break");
        }
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
    fn ctrl_k_and_ctrl_u_only_change_the_current_line() {
        let original = "first\nsecond middle\nthird";

        let mut kill_end = editor(original);
        kill_end.cursor = 9;
        kill_end.apply(ctrl('k'));
        assert_eq!(kill_end.buffer(), "first\nsec\nthird");
        assert_eq!(kill_end.cursor(), 9);

        let mut kill_start = editor(original);
        kill_start.cursor = 9;
        kill_start.apply(ctrl('u'));
        assert_eq!(kill_start.buffer(), "first\nond middle\nthird");
        assert_eq!(kill_start.cursor(), 6);

        assert!(kill_end.buffer().starts_with("first\n"));
        assert!(kill_end.buffer().ends_with("\nthird"));
        assert!(kill_start.buffer().starts_with("first\n"));
        assert!(kill_start.buffer().ends_with("\nthird"));
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
    fn enter_submits_the_whole_multiline_buffer_from_any_line() {
        let mut state = editor("first\nsecond\nthird");
        state.cursor = 2;
        assert_eq!(
            state.apply(key(KeyCode::Enter)),
            EditorAction::Finish(EditorOutcome::Submitted("first\nsecond\nthird".to_string()))
        );
    }

    #[test]
    fn enter_on_multiline_whitespace_keeps_the_editor_open() {
        let mut state = editor(" \n\n  \t\n");
        assert_eq!(state.apply(key(KeyCode::Enter)), EditorAction::Continue);
        assert_eq!(state.buffer(), " \n\n  \t\n");
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
        assert_eq!(
            display_width(PROMPT_INDICATOR),
            display_width(CONTINUATION_INDICATOR)
        );
    }

    #[test]
    fn prompt_rows_mark_continuations_and_report_the_cursor() {
        let mut state = editor("first\n日本a\nthird");
        state.cursor = 6 + 1;
        let (rows, cursor) = prompt_rows(&state);

        assert_eq!(rows, vec!["sai> first", "  |  日本a", "  |  third"]);
        assert_eq!(cursor, (1, 7), "one wide glyph adds two display columns");
    }

    #[test]
    fn prompt_row_count_matches_logical_line_count_including_trailing_lines() {
        for (buffer, expected) in [("", 1), ("one", 1), ("one\ntwo", 2), ("one\ntwo\n", 3)] {
            assert_eq!(prompt_rows(&editor(buffer)).0.len(), expected, "{buffer:?}");
        }
    }

    #[test]
    fn rendering_caps_tall_prompts_and_keeps_the_cursor_line_visible() {
        let state = editor("first\nsecond\nthird\nfourth");
        let mut out = Vec::new();
        // Two guidance rows leave room for two prompt rows.
        render_with_height(&state, &mut out, 4).unwrap();
        let drawn = String::from_utf8(out).unwrap();

        assert!(drawn.contains(TRUNCATION_ROW));
        assert!(drawn.contains("  |  fourth"));
        assert!(!drawn.contains("  |  second"));
    }

    #[test]
    fn guide_indicator_counts_characters_and_tracks_lines() {
        let cafe = editor("café");
        assert_eq!(guide_lines(&cafe)[0], "line 1/1 · 4 chars");

        let wide = editor("日本語");
        assert_eq!(guide_lines(&wide)[0], "line 1/1 · 3 chars");

        let mut state = editor("one\ntwo");
        assert_eq!(guide_lines(&state)[0], "line 2/2 · 7 chars");
        state.apply(key(KeyCode::Up));
        assert_eq!(guide_lines(&state)[0], "line 1/2 · 7 chars");
        state.cursor = 4;
        state.apply(key(KeyCode::Backspace));
        assert_eq!(guide_lines(&state)[0], "line 1/1 · 6 chars");
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
        let mut keys = keys.into_iter();
        run_loop(&mut state, &mut out, || {
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

        assert_eq!(
            drive(None, &[], keys),
            EditorOutcome::Submitted("ls".to_string())
        );
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
        assert_eq!(
            drive(None, &[], keys),
            EditorOutcome::Submitted("x".to_string())
        );
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
    fn multiline_history_entry_loads_atomically_then_uses_buffer_first_navigation() {
        let multiline = "one\nsecond\nthird";
        let mut state = with_history(&[multiline, "previous entry"]);

        state.apply(key(KeyCode::Up));
        assert_eq!(state.buffer(), multiline);
        assert_eq!(state.cursor(), multiline.chars().count());
        assert_eq!((state.cursor_line(), state.cursor_column()), (2, 5));

        state.apply(key(KeyCode::Up));
        assert_eq!(state.buffer(), multiline);
        assert_eq!((state.cursor_line(), state.cursor_column()), (1, 5));

        state.apply(key(KeyCode::Up));
        assert_eq!(state.buffer(), multiline);
        assert_eq!((state.cursor_line(), state.cursor_column()), (0, 3));

        state.apply(key(KeyCode::Up));
        assert_eq!(state.buffer(), "previous entry");
        assert_eq!(state.cursor(), "previous entry".chars().count());
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
    fn arrows_move_within_multiline_buffers_before_navigating_history() {
        let mut state = with_history(&["previous prompt"]);
        type_text(&mut state, "first\nsecond");

        state.apply(key(KeyCode::Up));
        assert_eq!(state.buffer(), "first\nsecond");
        assert_eq!((state.cursor_line(), state.cursor_column()), (0, 5));

        state.apply(key(KeyCode::Up));
        assert_eq!(state.buffer(), "previous prompt");

        let mut downward = with_history(&["previous prompt"]);
        type_text(&mut downward, "first\nsecond");
        downward.cursor = 2;
        downward.apply(key(KeyCode::Down));
        assert_eq!(downward.buffer(), "first\nsecond");
        assert_eq!((downward.cursor_line(), downward.cursor_column()), (1, 2));
    }

    #[test]
    fn up_on_the_first_line_with_empty_history_is_a_no_op() {
        let mut state = editor("first\nsecond");
        state.cursor = 3;
        let before = (state.buffer().to_string(), state.cursor());
        state.apply(key(KeyCode::Up));
        assert_eq!((state.buffer().to_string(), state.cursor()), before);
    }

    #[test]
    fn multiline_draft_is_restored_intact_after_history_navigation() {
        let mut state = with_history(&["previous prompt"]);
        type_text(&mut state, "first\nsecond");
        state.cursor = 3;

        state.apply(key(KeyCode::Up));
        assert_eq!(state.buffer(), "previous prompt");
        state.apply(key(KeyCode::Down));
        assert_eq!(state.buffer(), "first\nsecond");
        assert_eq!(state.cursor(), "first\nsecond".chars().count());
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
    fn reverse_search_matches_and_accepts_a_multiline_entry() {
        let mut state = with_history(&["first line\nneedle on line two\nthird line"]);
        state.apply(ctrl('r'));
        type_text(&mut state, "needle");

        assert_eq!(
            state.search_match(),
            Some("first line\nneedle on line two\nthird line")
        );
        assert_eq!(state.buffer(), "first line\nneedle on line two\nthird line");
        state.apply(key(KeyCode::Enter));
        assert!(!state.is_searching());
        assert_eq!(state.buffer(), "first line\nneedle on line two\nthird line");
        assert_eq!(state.cursor(), state.buffer().chars().count());
    }

    #[test]
    fn reverse_search_previews_multiline_matches_on_one_visible_row() {
        let mut state = with_history(&["first\nsecond\nthird"]);
        state.apply(ctrl('r'));
        type_text(&mut state, "second");

        let (rows, cursor) = prompt_rows(&state);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].contains("first ↵ second ↵ third"));
        assert!(!rows[0].contains('\n'));
        assert_eq!(cursor.0, 0);
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
        for key in ["history", "^R", "Alt+Enter", "^J", "Enter", "Esc", "^G"] {
            assert!(hint.contains(key), "hint should mention {}: {}", key, hint);
        }
    }

    #[test]
    fn the_hint_line_follows_the_mode() {
        let mut state = with_history(&["find large files"]);
        state.apply(ctrl('r'));
        let hint = state.hint();
        assert!(
            hint.contains("next match"),
            "search hint should say what ^R does now: {}",
            hint
        );
        assert!(
            hint.contains("accept"),
            "search hint should say how to accept: {}",
            hint
        );
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
        for binding in [
            "\u{2190}",
            "\u{2192}",
            "\u{2191}",
            "\u{2193}",
            "^A",
            "^E",
            "^K",
            "^U",
            "^L",
            "^R",
            "Home/End",
            "Alt+Enter",
            "^J",
            "Enter",
            "Esc",
        ] {
            assert!(
                panel.contains(binding),
                "the key panel should document {}",
                binding
            );
        }
    }

    /// Net rows the cursor moves over one render: `\r\n` descends one row,
    /// `CSI n A` climbs n. A render must end where it began.
    fn net_vertical_displacement(bytes: &[u8]) -> i32 {
        let text = String::from_utf8_lossy(bytes);
        let down = text.matches("\r\n").count() as i32;
        let up: i32 = regex_free_move_ups(&text);
        down - up
    }

    /// Sum the `n` of every `CSI n A` (cursor-up) in the stream.
    fn regex_free_move_ups(text: &str) -> i32 {
        let mut total = 0;
        let mut rest = text;
        while let Some(i) = rest.find("\u{1b}[") {
            let after = &rest[i + 2..];
            let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            let tail = &after[digits.len()..];
            if tail.starts_with('A') {
                total += digits.parse::<i32>().unwrap_or(1);
            }
            rest = after;
        }
        total
    }

    #[test]
    fn a_render_returns_the_cursor_to_the_row_it_started_on() {
        // The regression that ate a line of scrollback per keystroke: render
        // climbed on entry for a descent it had already undone, leaving a net
        // displacement of -1 and walking the prompt up the screen.
        let mut state = EditorState::new(None, Vec::new());

        let mut out = Vec::new();
        render(&state, &mut out).unwrap();
        assert_eq!(
            net_vertical_displacement(&out),
            0,
            "a plain render must leave the cursor on its starting row"
        );

        // With the help panel open the area is several rows tall; the descent
        // is larger but must still be fully undone.
        state.apply(ctrl('g'));
        let mut out = Vec::new();
        render(&state, &mut out).unwrap();
        assert_eq!(
            net_vertical_displacement(&out),
            0,
            "an expanded prompt area must still return to its starting row"
        );

        // And in search mode.
        state.apply(ctrl('g'));
        state.apply(ctrl('r'));
        let mut out = Vec::new();
        render(&state, &mut out).unwrap();
        assert_eq!(net_vertical_displacement(&out), 0, "search mode too");
    }

    #[test]
    fn render_reserves_the_prompt_area_before_saving_its_anchor() {
        let state = EditorState::new(None, Vec::new());
        let mut out = Vec::new();
        render_with_height(&state, &mut out, 24).unwrap();
        let text = String::from_utf8(out).unwrap();

        let save = text
            .find("\u{1b}7")
            .expect("render should save its scroll-adjusted top-row anchor");
        let before_save = &text[..save];
        let area_rows = prompt_rows(&state).0.len() + guide_lines(&state).len();

        assert_eq!(
            before_save.matches("\r\n").count(),
            area_rows - 1,
            "all rows below the prompt must be reserved before the anchor is saved"
        );
        assert!(
            !before_save.contains(PROMPT_INDICATOR),
            "prompt drawing must begin only after reservation and anchor save"
        );
    }

    #[test]
    fn repeated_renders_do_not_drift_up_the_screen() {
        // Typing is a render per keystroke; drift accumulates one row each.
        let mut state = EditorState::new(None, Vec::new());
        let mut total = 0;
        for ch in "find large files".chars() {
            state.apply(key(KeyCode::Char(ch)));
            let mut out = Vec::new();
            render(&state, &mut out).unwrap();
            total += net_vertical_displacement(&out);
        }
        assert_eq!(
            total, 0,
            "16 keystrokes drifted {} rows; the prompt must stay anchored",
            total
        );

        let state = editor("first\nsecond\nthird");
        let mut out = Vec::new();
        render(&state, &mut out).unwrap();
        assert_eq!(
            net_vertical_displacement(&out),
            0,
            "drawing a multi-row prompt must return to its saved anchor before parking"
        );
    }

    #[test]
    fn clearing_the_area_on_exit_does_not_climb_above_the_prompt() {
        // clear_area runs with the cursor on the prompt line, so any upward
        // movement would erase the line above the editor.
        let mut out = Vec::new();
        clear_area(&mut out).unwrap();
        assert_eq!(
            regex_free_move_ups(&String::from_utf8_lossy(&out)),
            0,
            "exit cleanup must not move up"
        );
    }

    #[test]
    fn a_render_clears_from_the_cursor_down() {
        // What makes the entry climb unnecessary: the downward wipe removes the
        // whole area, so a shrinking area needs no height bookkeeping.
        let state = EditorState::new(None, Vec::new());
        let mut out = Vec::new();
        render(&state, &mut out).unwrap();
        let text = String::from_utf8_lossy(&out);
        // crossterm emits ED with the default parameter omitted.
        assert!(
            text.contains("\u{1b}[J") || text.contains("\u{1b}[0J"),
            "expected clear-from-cursor-down, got {:?}",
            text
        );
    }

    #[test]
    fn rendering_draws_the_prompt_and_the_hint() {
        let state = editor("find large files");
        let mut out = Vec::new();
        render(&state, &mut out).unwrap();

        let drawn = String::from_utf8(out).unwrap();
        assert!(drawn.contains("sai> find large files"));
        assert!(drawn.contains("^G keys"));
    }

    #[test]
    fn alt_enter_inserts_a_line_break_without_submitting() {
        let mut state = editor("find large files");
        let action = state.apply(KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT));
        assert_eq!(action, EditorAction::Continue, "Alt+Enter must not submit");
        assert_eq!(state.buffer(), "find large files\n");
        assert_eq!((state.cursor_line(), state.cursor_column()), (1, 0));

        let mut split = editor("find rust files changed this week");
        split.cursor = "find rust files".chars().count();
        split.apply(KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT));
        assert_eq!(split.buffer(), "find rust files\n changed this week");
        assert_eq!((split.cursor_line(), split.cursor_column()), (1, 0));

        // Plain Enter still submits.
        assert_eq!(
            state.apply(key(KeyCode::Enter)),
            EditorAction::Finish(EditorOutcome::Submitted("find large files\n".to_string()))
        );
    }

    #[test]
    fn ctrl_j_inserts_a_line_break_without_submitting() {
        let mut state = editor("find large files");
        let action = state.apply(ctrl('j'));

        assert_eq!(action, EditorAction::Continue, "Ctrl+J must not submit");
        assert_eq!(state.buffer(), "find large files\n");
        assert_eq!((state.cursor_line(), state.cursor_column()), (1, 0));
    }
}
