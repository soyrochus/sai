## ADDED Requirements

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

## MODIFIED Requirements

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
