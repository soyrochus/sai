## Purpose

Defines persistent storage of the natural-language prompts a user submits, and how those prompts are recalled and reused during prompt composition through sequential navigation and reverse search.

## ADDED Requirements

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

### Requirement: Sequential history navigation in the editor

The interactive editor SHALL allow the user to walk backwards through prior prompts with the Up key and forwards with the Down key, most recent first. Navigating loads the selected prompt into the buffer as fully editable text with the cursor at the end. Navigating forward past the newest entry SHALL restore whatever the user had typed before navigation began.

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
