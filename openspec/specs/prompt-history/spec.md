# Prompt History Specification

## Purpose

Defines persistent storage of the natural-language prompts a user submits, and how those prompts are recalled and reused during prompt composition through sequential navigation and reverse search.

## Requirements

### Requirement: Submitted prompts persist across sessions

SAI SHALL record every prompt it submits for command generation in a prompt-history store, regardless of the input mode that produced it. Each record SHALL contain the prompt text and the submission timestamp. Records SHALL survive process exit and be available to subsequent invocations. The store SHALL be distinct from the existing execution history log, which continues to record invocations and outcomes.

#### Scenario: Prompt recorded after submission

- **WHEN** a user submits the prompt `find files larger than 100MB` and generation proceeds
- **THEN** an entry containing that text and a timestamp is appended to the prompt-history store

#### Scenario: Prompt available in a later session

- **WHEN** the user starts a new `sai` session after previously submitting prompts
- **THEN** those earlier prompts are available for navigation and search in the editor

#### Scenario: Argument-supplied prompts are recorded too

- **WHEN** the user runs `sai "list json files"` with the prompt as an argument
- **THEN** the prompt is recorded in the prompt-history store

#### Scenario: Cancelled composition is not recorded

- **WHEN** the user cancels the editor with Esc or `Ctrl+C` before submitting
- **THEN** nothing is appended to the prompt-history store

#### Scenario: Recording failure does not block the user

- **WHEN** the prompt-history store cannot be written, for example because its directory is read-only
- **THEN** SAI reports the problem on the error stream and continues with generation and execution normally

### Requirement: Prompt history storage is bounded and readable

The prompt-history store SHALL live in SAI's existing per-user configuration directory. Its size SHALL be bounded: once the store exceeds a defined limit, older entries SHALL be rotated out so growth stays bounded across long-term use. Rotation SHALL NOT lose the most recent entries. Each entry SHALL be independently parseable so that a single corrupt entry does not render the store unusable.

#### Scenario: Store exceeds its size limit

- **WHEN** the prompt-history store grows past its configured size limit
- **THEN** older content is rotated out, the store is reduced below the limit, and the most recent prompts remain available

#### Scenario: Corrupt entry encountered

- **WHEN** the store contains a line that cannot be parsed as a prompt entry
- **THEN** that entry is skipped and the remaining entries are still loaded and usable

#### Scenario: Store does not exist yet

- **WHEN** SAI runs for the first time and no prompt-history store is present
- **THEN** history navigation and search report an empty history and the store is created on first submission

### Requirement: Consecutive duplicate prompts are collapsed

When a submitted prompt is identical to the most recently recorded prompt, SAI SHALL NOT append a second record for it. Non-consecutive repeats SHALL be recorded normally.

#### Scenario: Immediate repeat

- **WHEN** the user submits `list json files` twice in a row
- **THEN** the prompt-history store contains a single entry for that text from those two submissions

#### Scenario: Repeat after a different prompt

- **WHEN** the user submits `list json files`, then `count records`, then `list json files` again
- **THEN** all three submissions are recorded

### Requirement: Multi-line prompts round-trip through history

A submitted prompt containing line breaks SHALL be recorded in the prompt-history store with those breaks preserved, and SHALL be restored into the editor buffer with the same line structure when recalled by navigation or by reverse search. A single stored prompt SHALL remain a single history entry however many lines it holds.

#### Scenario: Multi-line prompt is stored intact

- **WHEN** the user submits a three-line prompt
- **THEN** one history entry is recorded holding all three lines with their breaks in the composed positions

#### Scenario: Multi-line prompt is recalled intact

- **WHEN** the user recalls a stored three-line prompt with Up
- **THEN** the editor buffer holds all three lines, the prompt area draws three rows, and the cursor is at the end of the last line

#### Scenario: A multi-line entry is recalled as one entry

- **WHEN** the user presses Up once with a three-line prompt as the most recent entry
- **THEN** the whole three-line prompt is loaded in one operation with the cursor at its end; subsequent Up presses move through its lines, and only an Up press from its first line loads the entry before it

#### Scenario: Reverse search matches across the whole entry

- **WHEN** the user searches for text that appears only on the second line of a stored multi-line prompt
- **THEN** that entry is reported as a match and accepting it loads all of its lines into the buffer

#### Scenario: Duplicate collapsing accounts for line structure

- **WHEN** the user submits a two-line prompt and then submits the same text with the line break in a different position
- **THEN** both are recorded, the differing line structure making them distinct prompts

### Requirement: Sequential history navigation in the editor

The interactive editor SHALL allow the user to walk backwards through prior prompts with the Up key and forwards with the Down key, most recent first. Up SHALL navigate history only when the cursor is on the first line of the buffer; otherwise it SHALL move the cursor to the previous buffer line. Down SHALL navigate history only when the cursor is on the last line of the buffer; otherwise it SHALL move the cursor to the next buffer line. For a single-line buffer both conditions hold, so Up and Down navigate history exactly as they did before multi-line composition. Navigating loads the selected prompt into the buffer as fully editable text with the cursor at the end. Navigating forward past the newest entry SHALL restore whatever the user had typed before navigation began, including its line breaks.

#### Scenario: Recall the previous prompt

- **WHEN** the user presses Up in the editor with a non-empty history
- **THEN** the most recent prior prompt fills the buffer with the cursor at the end

#### Scenario: Walk further back

- **WHEN** the user presses Up repeatedly
- **THEN** successively older prompts are loaded, and pressing Up at the oldest entry keeps that entry displayed

#### Scenario: Return to the draft in progress

- **WHEN** the user has typed `count rec`, navigates up through history, then presses Down past the newest entry
- **THEN** the buffer is restored to `count rec`

#### Scenario: Navigation with empty history

- **WHEN** the user presses Up with no prompt history recorded
- **THEN** the buffer is unchanged and no error is shown

#### Scenario: Up moves within the buffer before reaching history

- **WHEN** the buffer holds two composed lines and the cursor is on the second, and the user presses Up
- **THEN** the cursor moves to the first line and no history entry is loaded

#### Scenario: Up reaches history from the first line

- **WHEN** the cursor is already on the first line of a multi-line buffer and the user presses Up again
- **THEN** the most recent prior prompt is loaded, replacing the whole buffer

#### Scenario: Down moves within the buffer before reaching history

- **WHEN** the buffer holds two composed lines and the cursor is on the first, and the user presses Down
- **THEN** the cursor moves to the second line and history is not advanced

#### Scenario: A multi-line draft is restored on returning from history

- **WHEN** the user composes a two-line draft, presses Up from its first line to enter history, then presses Down past the newest entry
- **THEN** the buffer is restored to the two-line draft with its line break intact

#### Scenario: Navigating with an empty history still moves the cursor

- **WHEN** the cursor is on the first line of a multi-line buffer with no prompt history recorded and the user presses Up
- **THEN** the buffer and cursor are unchanged and no error is shown

### Requirement: Reverse incremental search over prompt history

The interactive editor SHALL provide a reverse search mode entered with `Ctrl+R`. While in search mode, typed characters SHALL form a query matched against prior prompts, showing the most recent match as the query grows. Pressing `Ctrl+R` again SHALL step to the next older match. Accepting a match SHALL place it in the editor buffer as editable text; cancelling search SHALL restore the buffer as it was before search began. Search SHALL return results without perceptible delay for a history at its size limit.

#### Scenario: Find a prior prompt by substring

- **WHEN** the user presses `Ctrl+R` and types `json`
- **THEN** the most recent prior prompt containing `json` is displayed as the current match

#### Scenario: Cycle through matches

- **WHEN** several prior prompts match the current query and the user presses `Ctrl+R` again
- **THEN** the next older matching prompt is displayed

#### Scenario: Accept a match for editing

- **WHEN** the user accepts the currently displayed match
- **THEN** search mode exits, the matched text becomes the editor buffer, and the user can edit it before submitting

#### Scenario: Edit a recalled prompt before submit

- **WHEN** the user recalls `find files larger than 100MB`, changes `100MB` to `1GB`, and presses Enter
- **THEN** the edited text is what gets submitted for generation and what gets recorded in history

#### Scenario: No match for the query

- **WHEN** the query matches no prior prompt
- **THEN** the editor indicates that there is no match and the query remains editable

#### Scenario: Cancel search

- **WHEN** the user cancels reverse search
- **THEN** search mode exits and the buffer holds exactly what it held before search was entered
