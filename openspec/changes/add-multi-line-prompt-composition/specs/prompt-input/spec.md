## ADDED Requirements

### Requirement: Prompt buffer holds multiple lines

The interactive editor SHALL allow the prompt buffer to contain line breaks. `Alt+Enter` and `Ctrl+J` SHALL each insert a line break at the cursor, splitting the current line and leaving the cursor at the start of the new line. `Ctrl+J` SHALL remain available as a terminal-portable alternative when Alt/Option mappings do not distinguish `Alt+Enter`. There SHALL be no limit on the number of lines other than available memory. A buffer containing line breaks SHALL be rendered across one terminal row per buffer line, with rows after the first carrying a continuation indicator distinguishable from the prompt indicator, and the visible cursor SHALL sit on the row and column matching its logical position.

#### Scenario: Inserting a line break

- **WHEN** the buffer is `find rust files` with the cursor at the end and the user presses `Alt+Enter`
- **THEN** the buffer holds two lines, `find rust files` and an empty second line, and the cursor is at the start of the second line

#### Scenario: Splitting a line mid-way

- **WHEN** the buffer is `find rust files changed this week` with the cursor after `files` and the user presses `Alt+Enter`
- **THEN** the buffer holds `find rust files` and ` changed this week` as two lines, and the cursor is at the start of the second line

#### Scenario: Inserting a line break without Alt/Option

- **WHEN** the user presses `Ctrl+J` while composing
- **THEN** a line break is inserted at the cursor exactly as for `Alt+Enter`, and the editor stays open

#### Scenario: Continuation rows are marked

- **WHEN** the buffer holds three lines
- **THEN** the prompt area draws three rows, the first carrying the prompt indicator and the other two a continuation indicator, and the cursor appears on the row holding it

#### Scenario: Backspace at the start of a line joins it to the previous one

- **WHEN** the cursor is at the start of the second line and the user presses Backspace
- **THEN** the line break is removed, the two lines become one, and the cursor sits where the break was

#### Scenario: Delete at the end of a line joins the next one

- **WHEN** the cursor is at the end of the first of two lines and the user presses Delete
- **THEN** the line break is removed and the two lines become one

### Requirement: Editor reports line position and buffer size

While composing, the editor SHALL display an indicator reporting the line the cursor is on, the total number of lines in the buffer, and the total size of the buffer in characters. The indicator SHALL update as the buffer and cursor change, SHALL be visually subordinate to the prompt, and SHALL NOT itself alter the buffer or cursor.

#### Scenario: Indicator on a single-line buffer

- **WHEN** the buffer holds `list json files` on one line
- **THEN** the indicator reports line 1 of 1 and a size of 16 characters

#### Scenario: Indicator tracks the cursor across lines

- **WHEN** the buffer holds three lines and the user moves the cursor to the second
- **THEN** the indicator reports line 2 of 3

#### Scenario: Indicator counts characters, not bytes

- **WHEN** the buffer holds `café`
- **THEN** the indicator reports a size of 4 characters

### Requirement: Multi-line prompts submit as a single payload

Enter SHALL submit a multi-line buffer as one prompt string with its line breaks preserved verbatim. The submitted text SHALL reach command generation, and be recorded in prompt history, with the same line structure the user composed. Line breaks SHALL NOT be reinterpreted as separate prompts, as command separators, or as shell syntax.

#### Scenario: Line breaks survive submission

- **WHEN** the user composes a three-line prompt and presses Enter
- **THEN** the prompt sent for generation is a single string containing both line breaks in their composed positions

#### Scenario: One generation per submission

- **WHEN** a multi-line prompt is submitted
- **THEN** exactly one command is generated, as it would be for a single-line prompt, and the editor does not reopen

#### Scenario: Whitespace-only multi-line buffer is rejected

- **WHEN** the user presses Enter with a buffer containing only line breaks and spaces
- **THEN** no generation occurs and the editor remains open

## MODIFIED Requirements

### Requirement: Editor supports line editing within the prompt buffer

The interactive editor SHALL support editing the prompt buffer before submission. It SHALL support cursor movement left, right, to line start, and to line end; character insertion at the cursor; backspace deleting the character before the cursor; and delete removing the character at the cursor. Left at the start of a line SHALL move to the end of the previous line, and Right at the end of a line SHALL move to the start of the next, so the cursor traverses the whole buffer. Up and Down SHALL move the cursor between buffer lines, preserving the column where the target line is long enough and clamping to its end where it is not. Rendering SHALL keep the visible cursor position consistent with the logical cursor position in the buffer, including for multi-byte UTF-8 and wide characters.

#### Scenario: Editing in the middle of the buffer

- **WHEN** the buffer contains `find large fils` and the user moves the cursor left once and types `e`
- **THEN** the buffer contains `find large files` and the cursor sits after the inserted character

#### Scenario: Backspace at the start of the buffer

- **WHEN** the cursor is at position 0 and the user presses Backspace
- **THEN** the buffer is unchanged and no error is shown

#### Scenario: Non-ASCII input

- **WHEN** the user types text containing multi-byte characters such as `café` or `日本語`
- **THEN** the buffer holds the characters intact and the rendered cursor stays aligned with the logical cursor position

#### Scenario: Horizontal movement crosses line breaks

- **WHEN** the cursor is at the start of the second line and the user presses Left
- **THEN** the cursor moves to the end of the first line

#### Scenario: Vertical movement preserves the column

- **WHEN** the cursor is at column 10 of the second line and the user presses Up onto a first line at least 10 characters long
- **THEN** the cursor sits at column 10 of the first line

#### Scenario: Vertical movement clamps to a shorter line

- **WHEN** the cursor is at column 20 of the second line and the user presses Up onto a first line of 5 characters
- **THEN** the cursor sits at the end of the first line

### Requirement: Editor supports control-key shortcuts

The interactive editor SHALL support the following shortcuts, each acting on the line the cursor is currently on: `Ctrl+A` moves the cursor to the start of the current line; `Ctrl+E` moves it to the end of the current line; `Ctrl+K` deletes from the cursor to the end of the current line, leaving the following lines and the line break intact; `Ctrl+U` deletes from the start of the current line to the cursor, leaving the preceding lines and the line break intact. `Ctrl+L` SHALL clear and redraw the prompt area without altering the buffer. None of these shortcuts SHALL remove a line break or affect text on another line.

#### Scenario: Kill to end of line

- **WHEN** the buffer is `list all json files` with the cursor after `all` and the user presses `Ctrl+K`
- **THEN** the buffer becomes `list all` and the cursor stays at the same position

#### Scenario: Kill to start of line

- **WHEN** the buffer is `list all json files` with the cursor after `all` and the user presses `Ctrl+U`
- **THEN** the buffer becomes ` json files` and the cursor is at position 0

#### Scenario: Redraw preserves buffer

- **WHEN** the user presses `Ctrl+L`
- **THEN** the prompt area is cleared and redrawn with the buffer contents and cursor position unchanged

#### Scenario: Kill to end of a middle line leaves later lines alone

- **WHEN** the buffer holds three lines, the cursor sits mid-way through the second, and the user presses `Ctrl+K`
- **THEN** the second line is truncated at the cursor and the first and third lines are unchanged

#### Scenario: Line start is not buffer start

- **WHEN** the cursor is on the second of two lines and the user presses `Ctrl+A`
- **THEN** the cursor moves to the start of the second line, not to the start of the buffer

### Requirement: Submission and cancellation are unambiguous

Enter SHALL submit the current buffer. A submitted buffer that is empty or contains only whitespace SHALL NOT be sent for generation; the editor SHALL remain open awaiting input. `Alt+Enter` and `Ctrl+J` SHALL insert a line break rather than submit, whatever the cursor position and however many lines the buffer already holds. Esc and `Ctrl+C` SHALL cancel composition, leaving no generated command, no execution, and no new prompt-history entry, and SHALL exit with a success status distinguishable from an error. Cancellation SHALL discard a multi-line buffer exactly as it discards a single-line one, with no confirmation step.

#### Scenario: Submitting an empty buffer

- **WHEN** the user presses Enter with an empty or whitespace-only buffer
- **THEN** no generation occurs and the editor remains open

#### Scenario: Cancelling with Esc

- **WHEN** the user presses Esc while composing
- **THEN** SAI reports the cancellation, performs no generation or execution, records no prompt-history entry, and exits cleanly

#### Scenario: Cancelling with Ctrl+C

- **WHEN** the user presses `Ctrl+C` while composing
- **THEN** SAI behaves as it does for Esc

#### Scenario: `Alt+Enter` does not submit

- **WHEN** the user presses `Alt+Enter`
- **THEN** a line break is inserted at the cursor and the editor stays open, no submission occurring

#### Scenario: `Ctrl+J` does not submit

- **WHEN** the user presses `Ctrl+J`
- **THEN** a line break is inserted at the cursor and the editor stays open, no submission occurring

#### Scenario: Enter submits from any line

- **WHEN** the cursor is on the first of three lines and the user presses Enter
- **THEN** the whole three-line buffer is submitted, not just the line the cursor is on

#### Scenario: Cancelling a multi-line buffer

- **WHEN** the user presses Esc with several composed lines in the buffer
- **THEN** composition is cancelled immediately with no confirmation prompt and nothing is recorded

#### Scenario: Terminal state is restored

- **WHEN** the editor exits by submission, by cancellation, or because an error occurred during composition
- **THEN** the terminal is returned to its pre-editor mode with the cursor visible

### Requirement: Editor documents its own key bindings on screen

The editor SHALL always display a hint line beneath the prompt naming, at minimum, how to submit, how to insert a line break, how to cancel, how to reach history, and how to open the full key list. The hint SHALL describe the keys that apply to the current mode. `Ctrl+G` SHALL toggle an expanded panel listing every binding the editor implements, including line-break insertion and vertical cursor movement. Guidance SHALL be visually subordinate to the prompt, SHALL NOT alter the buffer or cursor, and SHALL leave no residue on screen once the editor exits.

#### Scenario: Hint line is visible while composing

- **WHEN** the editor is open
- **THEN** a hint line beneath the prompt names submission, line-break insertion, cancellation, history navigation, reverse search, and the key that opens the full list

#### Scenario: Expanded key panel

- **WHEN** the user presses `Ctrl+G`
- **THEN** a panel listing every implemented binding appears beneath the prompt, and pressing `Ctrl+G` again hides it leaving no residue

#### Scenario: Help does not disturb composition

- **WHEN** the user toggles the key panel while composing or while in reverse search
- **THEN** the buffer, the cursor position, and any active search are unchanged

#### Scenario: Hint follows the current mode

- **WHEN** the user enters reverse search
- **THEN** the hint line describes the search keys — stepping to the next match, accepting, and cancelling the search

#### Scenario: Prompt area is erased on exit

- **WHEN** the editor exits by submission or cancellation
- **THEN** the prompt line, every continuation row, the indicator, and all guidance lines are erased, leaving the terminal clean for whatever is printed next

### Requirement: Non-interactive contexts fall back to plain input

When standard input is not a terminal, or when the user passes `--no-interactive`, SAI SHALL NOT enter the interactive editor. It SHALL instead read a single line of prompt text from standard input, unchanged by the availability of multi-line composition in the editor. If no prompt is available from arguments or standard input in a non-interactive context, SAI SHALL exit with a non-zero status and an explicit message stating that no prompt was provided.

#### Scenario: Piped input

- **WHEN** the user runs `echo "list json files" | sai` with standard input redirected
- **THEN** SAI reads the piped line as the prompt and generates a command without attempting to enter raw terminal mode

#### Scenario: Legacy single-line mode forced in a terminal

- **WHEN** the user runs `sai --no-interactive` in a terminal
- **THEN** SAI reads one line of prompt text from standard input without opening the mini editor

#### Scenario: Multi-line piped input is still read as one line

- **WHEN** the user pipes several lines into `sai --no-interactive`
- **THEN** SAI reads the first line as the prompt, exactly as it does today, and multi-line composition does not apply

#### Scenario: No prompt available at all

- **WHEN** SAI runs in a non-interactive context with no prompt argument and standard input reaches end-of-file immediately
- **THEN** SAI exits non-zero with a message stating that no prompt was provided

#### Scenario: Conflicting mode flags

- **WHEN** the user passes both `--interactive` and `--no-interactive`
- **THEN** SAI rejects the invocation with an error explaining that the flags conflict

### Requirement: Editor scope is limited to prompt composition

The editor SHALL accept only natural-language prompt text. It SHALL NOT interpret shell syntax, execute commands, provide pipes, redirection, job control, or any shell runtime semantics, and it SHALL NOT reopen for another prompt after a submitted prompt has been processed. A line break in the buffer SHALL carry no meaning beyond text layout — it SHALL NOT be treated as a command separator, a statement terminator, or any other shell construct.

#### Scenario: Shell metacharacters are literal text

- **WHEN** the user types `list files | sort` into the editor and submits
- **THEN** the entire string is sent to the model as prompt text and no pipeline is constructed by the editor

#### Scenario: Line breaks carry no shell meaning

- **WHEN** the user composes `list files` and `sort them` on two lines and submits
- **THEN** both lines are sent as one natural-language prompt and the editor constructs no sequence of commands from them

#### Scenario: One prompt per invocation

- **WHEN** a submitted prompt has completed the generation and execution flow
- **THEN** SAI exits without reopening the editor
