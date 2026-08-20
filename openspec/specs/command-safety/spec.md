# Command Safety Specification

## Purpose

Defines the layered controls that sit between a generated command and its execution — tool restriction, operator blocking, explanation, and confirmation — together with the flags and configuration that select between them, including the unrestricted mode that lifts the restrictions in exchange for mandatory inspection.

## Requirements

### Requirement: Generated commands are restricted to configured tools by default

By default SAI SHALL constrain generation and execution to the tools configured for the invocation. The restriction SHALL be applied both when instructing the model and when validating the returned command, so that the model is told which tools it may use and a command naming any other tool is rejected before execution.

#### Scenario: Command uses a configured tool

- **WHEN** the generated command's program is one of the configured tools
- **THEN** the command proceeds to the remaining safety checks

#### Scenario: Command uses an unconfigured tool

- **WHEN** the generated command's program is not among the configured tools
- **THEN** SAI rejects it before execution and reports which tools are allowed

#### Scenario: The model is told what it may use

- **WHEN** SAI builds the instructions sent to the model in default mode
- **THEN** those instructions name the configured tools as the only permitted ones

### Requirement: Unrestricted mode lifts tool restriction and operator blocking

The `--unrestricted` flag SHALL lift, for that invocation only, both the tool restriction and the operator blocking that would otherwise reject pipes, redirection, chaining, and substitution, and SHALL execute the command through the shell. The restriction SHALL be lifted for generation as well as validation: the instructions sent to the model SHALL NOT confine it to the configured tools.

#### Scenario: A command using an unconfigured tool is permitted

- **WHEN** the user runs `sai --unrestricted` and the generated command names a tool that is not configured
- **THEN** SAI does not reject the command for its choice of tool

#### Scenario: The model is not confined to configured tools

- **WHEN** SAI builds the instructions sent to the model under `--unrestricted`
- **THEN** those instructions do not restrict it to the configured tools, so the model can reach for whatever the task needs

#### Scenario: Shell operators are permitted

- **WHEN** a command generated under `--unrestricted` contains a pipe, redirection, chaining, or command substitution
- **THEN** SAI does not reject it for containing that operator, and executes it through the shell

#### Scenario: Malformed commands are still rejected

- **WHEN** a command generated under `--unrestricted` cannot be parsed into a program and arguments
- **THEN** SAI rejects it, since lifting the restrictions does not lift the requirement that the command be well-formed

#### Scenario: Default behaviour is untouched

- **WHEN** the user runs any invocation without `--unrestricted`
- **THEN** tool restriction, operator blocking, confirmation, and explanation behave exactly as they did before the flag existed

### Requirement: Unrestricted mode forces inspection that cannot be suppressed

Under `--unrestricted` SAI SHALL always produce an explanation of the generated command and SHALL always require confirmation before executing it. No flag, configuration value, profile, or per-tool setting SHALL be able to suppress either. Where a configured default would reduce scrutiny, the mandatory inspection SHALL take precedence. The preflight card SHALL report the unrestricted mode as the safety mode in effect, and SHALL attribute the explanation to the mode's mandatory inspection; this reporting SHALL be in addition to, and SHALL NOT replace, the explicit statement that no tool restriction is in effect.

#### Scenario: Explanation is always produced

- **WHEN** a command is generated under `--unrestricted` without `--explain`
- **THEN** SAI explains the command before asking for confirmation

#### Scenario: Confirmation is always required

- **WHEN** a command is generated under `--unrestricted` without `--confirm`
- **THEN** SAI asks for confirmation before executing

#### Scenario: Configuration cannot reduce scrutiny

- **WHEN** the invocation runs under `--unrestricted` with a configured default that would otherwise skip explanation or confirmation
- **THEN** the explanation and confirmation still occur

#### Scenario: The mode is announced before the decision

- **WHEN** SAI asks for confirmation under `--unrestricted`
- **THEN** it states that no tool restriction is in effect, so the user knows which mode they are approving

#### Scenario: The card does not absorb the announcement

- **WHEN** the preflight card reports the unrestricted safety mode
- **THEN** the explicit statement that no tool restriction is in effect is still presented before the typed-affirmative prompt

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

### Requirement: The preflight card reports the inputs that shaped the command

The preflight card SHALL report, at minimum: the natural-language prompt that was submitted; the generated command; the primary tool the command invokes; the scope hint, when one was supplied; the safety mode in effect; why an explanation was required, when one was; the risk markers computed for the command; and which configuration supplied the prompt definition. A field whose value does not apply to the invocation SHALL be omitted or shown as explicitly absent rather than shown blank. Every value SHALL reflect what SAI actually resolved for this invocation, not what was requested — where a flag was overridden or a setting forced a behavior, the card SHALL report the effective outcome.

#### Scenario: Primary tool is named

- **WHEN** the generated command is `rg --files-with-matches TODO src/`
- **THEN** the card names `rg` as the primary tool

#### Scenario: Scope hint is reported when used

- **WHEN** the invocation supplied `--scope ./logs`
- **THEN** the card reports that scope hint

#### Scenario: Scope hint is absent when not supplied

- **WHEN** the invocation supplied no scope hint
- **THEN** the card omits the scope field or marks it explicitly absent, rather than showing an empty value

#### Scenario: Safety mode is reported

- **WHEN** the invocation runs under `--unsafe`
- **THEN** the card reports the unsafe mode rather than the default mode

#### Scenario: Explanation required by flag

- **WHEN** an explanation was produced because the user passed `--explain`
- **THEN** the card attributes the explanation to that flag

#### Scenario: Explanation required by tool configuration

- **WHEN** an explanation was produced because the command's tool has `force_explain` enabled and the user did not pass `--explain`
- **THEN** the card attributes the explanation to the tool's configuration rather than to a flag

#### Scenario: Explanation required by unrestricted mode

- **WHEN** an explanation was produced because the invocation is unrestricted
- **THEN** the card attributes the explanation to the mode's mandatory inspection

#### Scenario: Configuration provenance is reported

- **WHEN** the prompt definition came from a per-call prompt config file rather than the global default
- **THEN** the card identifies that file as the source

#### Scenario: The card reports effective values

- **WHEN** a setting forces a behavior the user's flags did not request
- **THEN** the card reports the behavior that is actually in effect

### Requirement: The preflight card does not alter execution semantics

Presenting the preflight card SHALL NOT change which command runs, whether it runs, how it is validated, what is recorded in the execution history, or the invocation's exit code. Computing the card's contents SHALL NOT execute the command or any part of it, SHALL NOT consult a model, and SHALL have no side effects. The card SHALL be advisory: it informs the decision the confirmation governs and neither approves nor blocks anything on its own.

#### Scenario: Identical outcomes with and without a card

- **WHEN** the same command is confirmed and executed
- **THEN** the exit code and the execution-history entry are the same as they would be without the card

#### Scenario: Card computation runs nothing

- **WHEN** a card is built for a command
- **THEN** no part of that command is executed and no model is consulted

#### Scenario: The card cannot approve a command

- **WHEN** a card reports no risk markers at all
- **THEN** the confirmation is still asked and still governs whether the command runs

### Requirement: Unrestricted confirmation requires an unambiguous affirmative

The confirmation shown under `--unrestricted` SHALL require the user to type `yes` in full. Input that would confirm an ordinary prompt, including a bare `y`, SHALL NOT execute the command. Anything other than the full affirmative SHALL cancel without executing.

#### Scenario: Typing the full affirmative

- **WHEN** the user types `yes` at the unrestricted confirmation
- **THEN** the command executes

#### Scenario: A bare `y` does not execute

- **WHEN** the user types `y` at the unrestricted confirmation
- **THEN** the command does not execute and the invocation ends without running anything

#### Scenario: Declining

- **WHEN** the user types anything else, or provides no input
- **THEN** the command does not execute and the invocation reports that it was cancelled

#### Scenario: Ordinary confirmations are unchanged

- **WHEN** the user is asked to confirm an invocation that is not unrestricted
- **THEN** a bare `y` still confirms it, as it does today

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

### Requirement: Confirmations carry deterministic risk markers

Because the explanation is produced by the same model that generated the command, it is not an independent check. SAI SHALL therefore also present risk markers computed locally from the command text, without consulting a model, on the preflight card of every confirmation — not only under `--unrestricted`. Markers SHALL cover at minimum shell operators, recursive or forced deletion, and wildcard breadth. Marker computation SHALL be deterministic, SHALL NOT execute the command or any part of it, and SHALL be advisory — it informs the confirmation rather than replacing it. When a command carries no markers, the card SHALL say so explicitly rather than leaving the field blank, so an absent marker is distinguishable from a marker that was never computed.

#### Scenario: Destructive flags are marked

- **WHEN** a command generated under `--unrestricted` contains recursive or forced deletion
- **THEN** the confirmation presents a marker naming it

#### Scenario: Operators are marked

- **WHEN** a command generated under `--unrestricted` contains pipes, redirection, chaining, or substitution
- **THEN** the confirmation presents a marker naming them

#### Scenario: Broad wildcards are marked

- **WHEN** a command generated under `--unrestricted` targets a wildcard covering a broad path
- **THEN** the confirmation presents a marker naming the breadth

#### Scenario: Markers never execute the command

- **WHEN** risk markers are computed for any command
- **THEN** no part of the command is executed, and computing the markers has no side effects

#### Scenario: Markers are advisory

- **WHEN** a command carries risk markers
- **THEN** the confirmation still governs whether it runs, and markers alone neither block nor approve it

#### Scenario: Markers appear on an unsafe confirmation

- **WHEN** a command confirmed under `--unsafe` contains a pipe
- **THEN** the card presents an operator marker naming it

#### Scenario: Markers appear on an ordinary confirmation

- **WHEN** a command confirmed under `--confirm` alone contains recursive or forced deletion
- **THEN** the card presents a destructive marker naming it

#### Scenario: No markers is stated explicitly

- **WHEN** a confirmed command carries no risk markers
- **THEN** the card states that none were found rather than showing an empty field

### Requirement: Unrestricted invocations are identifiable in the history log

The execution history SHALL record whether an invocation ran unrestricted, so that later review can distinguish those runs. Existing history entries written before this field existed SHALL remain readable.

#### Scenario: Unrestricted run is recorded as such

- **WHEN** an invocation runs under `--unrestricted`
- **THEN** its history entry identifies it as unrestricted

#### Scenario: Ordinary run is recorded as such

- **WHEN** an invocation runs without `--unrestricted`
- **THEN** its history entry identifies it as not unrestricted

#### Scenario: Older history remains readable

- **WHEN** SAI reads a history entry written before this field existed
- **THEN** the entry loads successfully and is treated as not unrestricted
