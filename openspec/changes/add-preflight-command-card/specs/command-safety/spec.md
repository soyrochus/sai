## ADDED Requirements

### Requirement: A preflight card precedes every confirmation

Wherever SAI asks the user to confirm a generated command, it SHALL first present a preflight card: a single compact block summarizing the command and the decisions that produced it. The card SHALL be presented after any explanation and immediately before the confirmation prompt, so it is the last thing the user reads before deciding. An invocation that executes without asking for confirmation SHALL NOT present a card. The card SHALL be written to the error stream, leaving the executed command's own output on the standard stream untouched.

#### Scenario: Card shown before an ordinary confirmation

- **WHEN** SAI asks the user to confirm a command under `--confirm`
- **THEN** a preflight card is presented immediately before the confirmation prompt

#### Scenario: Card shown before an unrestricted confirmation

- **WHEN** SAI asks the user to confirm a command under `--unrestricted`
- **THEN** a preflight card is presented immediately before the confirmation prompt, followed by the unrestricted mode's own announcement and its typed-affirmative prompt

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

## RENAMED Requirements

- FROM: `### Requirement: Unrestricted invocations carry deterministic risk markers`
- TO: `### Requirement: Confirmations carry deterministic risk markers`

## MODIFIED Requirements

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
