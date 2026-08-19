## Purpose

Defines how SAI obtains the natural-language prompt from the user before command generation: as a command-line argument, through an interactive mini editor, or through a plain non-interactive read, along with how the user cancels out of composition.

## ADDED Requirements

### Requirement: Argument-supplied prompt bypasses interactive input

When the user supplies a natural-language prompt on the command line, SAI SHALL use that text verbatim and SHALL NOT open the interactive editor. This preserves existing scripted and piped usage unchanged.

#### Scenario: Simple mode with a prompt argument

- **WHEN** the user runs `sai "find files larger than 100MB"` in a terminal
- **THEN** SAI generates a command from that exact prompt text without displaying an editor

#### Scenario: Advanced mode with a config and a prompt argument

- **WHEN** the user runs `sai jq-prompt.yaml "count records per file"`
- **THEN** SAI loads the per-call prompt config and uses the second argument as the prompt without displaying an editor

#### Scenario: Explicit editor request with an argument present

- **WHEN** the user runs `sai --interactive "find large files"`
- **THEN** SAI opens the editor with `find large files` pre-loaded in the buffer as editable text

### Requirement: Interactive editor is the default when no prompt argument is given

When no natural-language prompt is supplied and standard input is a terminal, SAI SHALL open the interactive mini editor to compose the prompt. The editor SHALL display a visible prompt indicator and the current buffer contents.

#### Scenario: Bare invocation in a terminal

- **WHEN** the user runs `sai` with no arguments from an interactive terminal
- **THEN** SAI opens the mini editor and waits for the user to compose a prompt

#### Scenario: Explicit editor request with a per-call config

- **WHEN** the user runs `sai --interactive jq-prompt.yaml`
- **THEN** SAI loads `jq-prompt.yaml` as the per-call prompt config and opens the editor to compose the prompt

#### Scenario: Submitted prompt enters the normal flow

- **WHEN** the user submits a composed prompt from the editor
- **THEN** SAI proceeds through the unchanged generation, safety-validation, explain, confirmation, and execution flow using the composed text as the prompt

### Requirement: Editor supports line editing within the prompt buffer

The interactive editor SHALL support editing the prompt buffer before submission. It SHALL support cursor movement left, right, to line start, and to line end; character insertion at the cursor; backspace deleting the character before the cursor; and delete removing the character at the cursor. Rendering SHALL keep the visible cursor position consistent with the logical cursor position in the buffer, including for multi-byte UTF-8 and wide characters.

#### Scenario: Editing in the middle of the buffer

- **WHEN** the buffer contains `find large fils` and the user moves the cursor left once and types `e`
- **THEN** the buffer contains `find large files` and the cursor sits after the inserted character

#### Scenario: Backspace at the start of the buffer

- **WHEN** the cursor is at position 0 and the user presses Backspace
- **THEN** the buffer is unchanged and no error is shown

#### Scenario: Non-ASCII input

- **WHEN** the user types text containing multi-byte characters such as `café` or `日本語`
- **THEN** the buffer holds the characters intact and the rendered cursor stays aligned with the logical cursor position

### Requirement: Editor supports control-key shortcuts

The interactive editor SHALL support the following shortcuts: `Ctrl+A` moves the cursor to line start; `Ctrl+E` moves it to line end; `Ctrl+K` deletes from the cursor to line end; `Ctrl+U` deletes from line start to the cursor; `Ctrl+L` clears and redraws the prompt area without altering the buffer.

#### Scenario: Kill to end of line

- **WHEN** the buffer is `list all json files` with the cursor after `all` and the user presses `Ctrl+K`
- **THEN** the buffer becomes `list all` and the cursor stays at the same position

#### Scenario: Kill to start of line

- **WHEN** the buffer is `list all json files` with the cursor after `all` and the user presses `Ctrl+U`
- **THEN** the buffer becomes ` json files` and the cursor is at position 0

#### Scenario: Redraw preserves buffer

- **WHEN** the user presses `Ctrl+L`
- **THEN** the prompt area is cleared and redrawn with the buffer contents and cursor position unchanged

### Requirement: Submission and cancellation are unambiguous

Enter SHALL submit the current buffer. A submitted buffer that is empty or contains only whitespace SHALL NOT be sent for generation; the editor SHALL remain open awaiting input. Esc and `Ctrl+C` SHALL cancel composition, leaving no generated command, no execution, and no new prompt-history entry, and SHALL exit with a success status distinguishable from an error.

#### Scenario: Submitting an empty buffer

- **WHEN** the user presses Enter with an empty or whitespace-only buffer
- **THEN** no generation occurs and the editor remains open

#### Scenario: Cancelling with Esc

- **WHEN** the user presses Esc while composing
- **THEN** SAI reports the cancellation, performs no generation or execution, records no prompt-history entry, and exits cleanly

#### Scenario: Cancelling with Ctrl+C

- **WHEN** the user presses `Ctrl+C` while composing
- **THEN** SAI behaves as it does for Esc

#### Scenario: Terminal state is restored

- **WHEN** the editor exits by submission, by cancellation, or because an error occurred during composition
- **THEN** the terminal is returned to its pre-editor mode with the cursor visible

### Requirement: Non-interactive contexts fall back to plain input

When standard input is not a terminal, or when the user passes `--no-interactive`, SAI SHALL NOT enter the interactive editor. It SHALL instead read a single line of prompt text from standard input. If no prompt is available from arguments or standard input in a non-interactive context, SAI SHALL exit with a non-zero status and an explicit message stating that no prompt was provided.

#### Scenario: Piped input

- **WHEN** the user runs `echo "list json files" | sai` with standard input redirected
- **THEN** SAI reads the piped line as the prompt and generates a command without attempting to enter raw terminal mode

#### Scenario: Legacy single-line mode forced in a terminal

- **WHEN** the user runs `sai --no-interactive` in a terminal
- **THEN** SAI reads one line of prompt text from standard input without opening the mini editor

#### Scenario: No prompt available at all

- **WHEN** SAI runs in a non-interactive context with no prompt argument and standard input reaches end-of-file immediately
- **THEN** SAI exits non-zero with a message stating that no prompt was provided

#### Scenario: Conflicting mode flags

- **WHEN** the user passes both `--interactive` and `--no-interactive`
- **THEN** SAI rejects the invocation with an error explaining that the flags conflict

### Requirement: Prompt input mode does not alter downstream behavior

The choice of prompt input mode SHALL NOT change command generation, tool restriction, safety validation, explain behavior, confirmation behavior, execution, or execution-history logging. A prompt composed in the editor SHALL produce the same outcome as the identical prompt supplied as an argument.

#### Scenario: Equivalent outcomes across input modes

- **WHEN** the same prompt text is supplied once as a command-line argument and once composed in the editor, with all other flags equal
- **THEN** both invocations follow the same generation, safety, confirmation, and execution path and write equivalent execution-history entries

#### Scenario: Safety flags still apply

- **WHEN** a prompt composed in the editor is submitted with `--unsafe` or `--explain` set
- **THEN** the corresponding confirmation and explanation behavior applies exactly as it does for an argument-supplied prompt

### Requirement: Interactive editing works across supported platforms

The interactive editor SHALL function on macOS, Linux, and Windows terminals. Where a key or rendering capability is unavailable on a given terminal, SAI SHALL degrade to the non-interactive single-line read rather than fail.

#### Scenario: Terminal cannot enter raw mode

- **WHEN** SAI attempts to open the editor and the terminal refuses raw mode
- **THEN** SAI reports the limitation, falls back to reading a single line from standard input, and continues

### Requirement: Editor scope is limited to prompt composition

The editor SHALL accept only natural-language prompt text. It SHALL NOT interpret shell syntax, execute commands, provide pipes, redirection, job control, or any shell runtime semantics, and it SHALL NOT reopen for another prompt after a submitted prompt has been processed.

#### Scenario: Shell metacharacters are literal text

- **WHEN** the user types `list files | sort` into the editor and submits
- **THEN** the entire string is sent to the model as prompt text and no pipeline is constructed by the editor

#### Scenario: One prompt per invocation

- **WHEN** a submitted prompt has completed the generation and execution flow
- **THEN** SAI exits without reopening the editor
