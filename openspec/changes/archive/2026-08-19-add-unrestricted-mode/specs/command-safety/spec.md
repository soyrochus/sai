## Purpose

Defines the layered controls that sit between a generated command and its execution — tool restriction, operator blocking, explanation, and confirmation — together with the flags and configuration that select between them, including the unrestricted mode that lifts the restrictions in exchange for mandatory inspection.

## ADDED Requirements

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

Under `--unrestricted` SAI SHALL always produce an explanation of the generated command and SHALL always require confirmation before executing it. No flag, configuration value, profile, or per-tool setting SHALL be able to suppress either. Where a configured default would reduce scrutiny, the mandatory inspection SHALL take precedence.

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

### Requirement: Unrestricted invocations carry deterministic risk markers

Because the explanation is produced by the same model that generated the command, it is not an independent check. Under `--unrestricted` SAI SHALL therefore also present risk markers computed locally from the command text, without consulting a model. Markers SHALL cover at minimum shell operators, recursive or forced deletion, and wildcard breadth. Marker computation SHALL be deterministic, SHALL NOT execute the command or any part of it, and SHALL be advisory — it informs the confirmation rather than replacing it.

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
