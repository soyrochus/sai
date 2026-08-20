## MODIFIED Requirements

### Requirement: Unrestricted mode can be forbidden by configuration

The global configuration SHALL support a setting that disables unrestricted mode. When the setting forbids it, `--unrestricted` SHALL refuse to run, SHALL exit with a non-zero status, and SHALL name the configuration file responsible so the user can find it. When the setting is absent, unrestricted mode SHALL be available, preserving behaviour for existing configurations.

The setting SHALL additionally refuse to freeze a command that was generated under unrestricted mode, so a forbidden mode cannot be laundered into a permanent artifact by saving it first. Because a frozen command runs without SAI, the setting SHALL NOT be understood as governing the execution of a script that already exists: its guarantee is that this machine does not produce unwhitelisted commands, not that no such command can ever run on it.

#### Scenario: Unrestricted mode disabled by configuration

- **WHEN** the configuration forbids unrestricted mode and the user passes `--unrestricted`
- **THEN** SAI refuses to run, exits non-zero, and names the configuration file that forbade it

#### Scenario: Refusal happens before anything is generated

- **WHEN** the configuration forbids unrestricted mode
- **THEN** SAI refuses before contacting the model, so no command is generated and nothing is recorded as executed

#### Scenario: Setting absent

- **WHEN** the configuration does not mention unrestricted mode
- **THEN** `--unrestricted` is available

#### Scenario: Ordinary invocations are unaffected by the setting

- **WHEN** the configuration forbids unrestricted mode and the user runs SAI without `--unrestricted`
- **THEN** the invocation proceeds normally

#### Scenario: Freezing an unrestricted command is refused

- **WHEN** the configuration forbids unrestricted mode and the user tries to freeze a command recorded as generated under that mode
- **THEN** SAI refuses, names the configuration file responsible, and writes no script

#### Scenario: Freezing an ordinary command is unaffected

- **WHEN** the configuration forbids unrestricted mode and the user freezes a command generated under default or unsafe mode
- **THEN** the command is frozen normally

### Requirement: A preflight card precedes every confirmation

Wherever SAI asks the user to confirm a generated command, it SHALL first present a preflight card: a single compact block summarizing the command and the decisions that produced it. This SHALL include the confirmation that precedes freezing a command into a permanent artifact, which is the point at which per-use review stops. The card SHALL be presented after any explanation and immediately before the confirmation prompt, so it is the last thing the user reads before deciding. An invocation that executes without asking for confirmation SHALL NOT present a card. The card SHALL be written to the error stream, leaving the executed command's own output on the standard stream untouched.

#### Scenario: Card shown before an ordinary confirmation

- **WHEN** SAI asks the user to confirm a command under `--confirm`
- **THEN** a preflight card is presented immediately before the confirmation prompt

#### Scenario: Card shown before an unrestricted confirmation

- **WHEN** SAI asks the user to confirm a command under `--unrestricted`
- **THEN** a preflight card is presented immediately before the confirmation prompt, followed by the unrestricted mode's own announcement and its typed-affirmative prompt

#### Scenario: Card shown before freezing

- **WHEN** SAI asks the user to confirm freezing a command into a script
- **THEN** a preflight card for that command is presented immediately before the confirmation prompt

#### Scenario: No confirmation means no card

- **WHEN** an invocation carries no flag, mode, or tool setting that requires confirmation and so executes directly
- **THEN** no preflight card is presented and the invocation's output is unchanged from what it produces today

#### Scenario: Card follows the explanation

- **WHEN** an explanation is produced for the command
- **THEN** the explanation is presented first and the card second, so the card sits adjacent to the confirmation prompt

#### Scenario: Card goes to the error stream

- **WHEN** a card is presented and the user confirms
- **THEN** the card appears on the error stream and the standard stream carries only the executed command's own output
