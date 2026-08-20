## Purpose

Defines how a verified generated command is frozen into an executable script on the user's `PATH` and run thereafter without a model — or SAI — in the loop: the script artifact and its provenance header, where scripts are stored, how they are listed, and what SAI refuses to freeze.

## ADDED Requirements

### Requirement: A verified command can be frozen as an executable script

SAI SHALL provide a way to freeze a generated command it has produced into a single executable script under a user-supplied name. Freezing SHALL be available both for a command generated earlier in a previous invocation and for one generated and frozen in a single step. Freezing SHALL write exactly one script file and SHALL create nothing else — no index, registry, or companion metadata file. The command's identity as a frozen artifact SHALL live entirely in that file, so copying, moving, editing, deleting or version-controlling the file is sufficient to manage it.

#### Scenario: Freezing the previously generated command

- **WHEN** the user generates a command in one invocation and then runs `sai --save cleanlogs`
- **THEN** an executable script named `cleanlogs` is written to the commands directory carrying that command

#### Scenario: Generating and freezing in one step

- **WHEN** the user runs `sai --save cleanlogs "remove old build logs"`
- **THEN** the command is generated and reviewed, and on acceptance the script is written

#### Scenario: Freezing writes only the script

- **WHEN** a command is frozen
- **THEN** the commands directory gains one file and no index or metadata file is created anywhere

#### Scenario: The script is executable

- **WHEN** a script is written on a platform with an executable permission bit
- **THEN** the file is created executable by its owner, so it runs from `PATH` without further action

#### Scenario: Nothing to freeze

- **WHEN** the user runs `sai --save cleanlogs` with no previously generated command available
- **THEN** SAI reports that there is nothing to freeze, exits non-zero, and writes no file

### Requirement: A frozen command runs without SAI

A frozen command SHALL be executable directly by the user's shell as an ordinary program resolved through `PATH`. Running it SHALL NOT invoke SAI, contact a model, or require a network. The text of the command it runs SHALL be identical on every run for as long as the file is unmodified.

#### Scenario: Invoked as an ordinary command

- **WHEN** the commands directory is on `PATH` and the user types `cleanlogs`
- **THEN** the frozen command runs, with SAI taking no part in the invocation

#### Scenario: Offline execution

- **WHEN** a frozen command is run with no network available
- **THEN** it runs normally, since no model is consulted

#### Scenario: Byte-identical across runs

- **WHEN** an unmodified frozen command is run repeatedly
- **THEN** the command text executed is byte-identical each time

### Requirement: Each script carries its own provenance

Every emitted script SHALL record, as comments in its own header, the natural-language intent that produced the command, the time it was frozen, the safety mode it was frozen under, the tools it was permitted, and which prompt configuration supplied that permission. When risk markers were computed at freeze time, the header SHALL record them too. The header SHALL be readable as plain text without SAI, and SHALL travel with the file when it is copied or committed.

#### Scenario: Intent is recorded

- **WHEN** a command frozen from the prompt `remove old build logs` is inspected with an ordinary file viewer
- **THEN** its header states that intent

#### Scenario: Safety mode is recorded

- **WHEN** a command generated under `--unrestricted` is frozen
- **THEN** its header records the unrestricted safety mode

#### Scenario: Risk markers are recorded

- **WHEN** a command carrying risk markers is frozen
- **THEN** its header records those markers

#### Scenario: Provenance survives copying

- **WHEN** a frozen script is copied to another machine
- **THEN** its intent, freeze time, safety mode, tools and prompt configuration are all still readable from the file

### Requirement: The intent behind a frozen command is recoverable

So that a command frozen from a prompt composed in the interactive editor carries its intent, SAI SHALL record the submitted natural-language prompt in each invocation-history entry alongside the command that was generated from it. The field SHALL tolerate its own absence, so entries written before it existed still parse and are still usable for everything that does not need the intent.

#### Scenario: Intent recovered for an editor-composed prompt

- **WHEN** the user composes a prompt in the interactive editor, accepts the generated command, and then freezes it in a later invocation
- **THEN** the frozen script's header carries the composed prompt as its intent

#### Scenario: Intent recovered for an argument-supplied prompt

- **WHEN** the user supplies the prompt as a command-line argument and freezes the command in a later invocation
- **THEN** the frozen script's header carries that prompt as its intent

#### Scenario: Older history entries still parse

- **WHEN** the invocation history contains entries written before the prompt was recorded
- **THEN** those entries load normally and report no intent, rather than failing to parse

#### Scenario: Freezing an entry with no recorded intent

- **WHEN** the command being frozen comes from a history entry that carries no prompt
- **THEN** the script is still written, its header marking the intent as unavailable rather than omitting the field silently

### Requirement: Emitted commands preserve the semantics of the mode they were frozen under

A frozen script runs under a shell, whereas a command generated in default mode did not. The emitted command text SHALL therefore preserve the semantics in force at freeze time rather than the text alone.

A command frozen under a mode that executed through a shell SHALL be emitted verbatim, with its operators intact. A command frozen under default mode SHALL be emitted with each argument quoted, except arguments containing glob metacharacters, which SHALL be emitted unquoted so the shell performs the same expansion SAI performed. In neither case SHALL the emitted script grant the command word splitting, command substitution, or operator handling that it did not have when it was verified.

#### Scenario: Operators survive for a shell-executed mode

- **WHEN** a command containing a pipe, frozen under `--unsafe`, is emitted
- **THEN** the script carries the pipe and the shell interprets it as it did at generation time

#### Scenario: Globs still expand for a default-mode command

- **WHEN** a default-mode command whose argument is `src/*` is emitted
- **THEN** that argument is left unquoted so the shell expands it, matching the expansion SAI applied when the command was verified

#### Scenario: Ordinary arguments are quoted

- **WHEN** a default-mode command has an argument containing a space or a shell metacharacter other than a glob
- **THEN** that argument is quoted, so the shell cannot split or reinterpret it

#### Scenario: No new shell capability is granted

- **WHEN** a default-mode command containing a character the shell would treat as an operator is emitted
- **THEN** the emitted script does not let the shell act on it, because default mode did not

### Requirement: Risky frozen commands carry their own confirmation

When risk markers were present at freeze time, the emitted script SHALL contain a confirmation prompt that runs before the command and aborts on anything other than an affirmative answer. The guard SHALL be part of the script text, visible and removable by anyone reading the file. A command with no risk markers SHALL NOT carry a guard.

#### Scenario: Guard emitted for a risky command

- **WHEN** a command carrying a destructive risk marker is frozen
- **THEN** the emitted script asks for confirmation before running the command

#### Scenario: Declining the guard runs nothing

- **WHEN** the user answers anything other than the affirmative at a guarded script's prompt
- **THEN** the script exits without running the command

#### Scenario: No guard without markers

- **WHEN** a command carrying no risk markers is frozen
- **THEN** the emitted script runs the command directly, with no confirmation prompt

#### Scenario: The guard is editable

- **WHEN** a user removes the guard from a frozen script
- **THEN** the script runs the command directly thereafter, SAI having no further say

### Requirement: Frozen commands live in a configurable directory the user puts on PATH

Scripts SHALL be written to a dedicated directory under SAI's existing per-user configuration directory, overridable by a configuration setting. SAI SHALL provide a way to print that directory so it can be added to `PATH`, and SHALL surface the line to add during initialization. SAI SHALL NOT modify the user's shell startup files.

#### Scenario: Default location

- **WHEN** no override is configured
- **THEN** scripts are written to a dedicated directory beside the existing configuration, history and prompt-history files

#### Scenario: Configured override

- **WHEN** the configuration names a different commands directory
- **THEN** scripts are written there and listed from there

#### Scenario: Directory reported for shell configuration

- **WHEN** the user asks SAI for the commands path
- **THEN** SAI prints the directory, suitable for use in a `PATH` assignment

#### Scenario: Initialization surfaces the PATH step

- **WHEN** the user initializes SAI's configuration
- **THEN** the line to add to `PATH` is printed

#### Scenario: Startup files are never modified

- **WHEN** SAI writes a script or prints the `PATH` line
- **THEN** no shell startup file is read, written, or modified

#### Scenario: Directory created on first freeze

- **WHEN** the first command is frozen and the commands directory does not exist
- **THEN** it is created

### Requirement: Frozen commands can be listed with their provenance

SAI SHALL list the frozen commands by reading the scripts in the commands directory and parsing their headers, consulting no index. The listing SHALL report each command's name and provenance, SHALL mark commands frozen under unrestricted mode, and SHALL flag a command whose recorded tools are no longer present on `PATH`. A file that is not a SAI-emitted script, or whose header cannot be parsed, SHALL be skipped without preventing the rest from listing.

#### Scenario: Listing reads the files

- **WHEN** the user lists frozen commands after editing a script's intent by hand
- **THEN** the listing reports the edited intent, because it read the file

#### Scenario: Unrestricted commands are marked

- **WHEN** the listing includes a command frozen under unrestricted mode
- **THEN** that command is visibly marked as such

#### Scenario: Missing tools are flagged

- **WHEN** a frozen command records a tool that is no longer on `PATH`
- **THEN** the listing flags that command as one that will fail

#### Scenario: Unparseable file does not break the listing

- **WHEN** the commands directory contains a file that is not a SAI-emitted script
- **THEN** it is skipped and the remaining commands are still listed

#### Scenario: Empty or absent directory

- **WHEN** no commands have been frozen
- **THEN** the listing reports that there are none, rather than failing

### Requirement: Freezing refuses names that would shadow or overwrite

SAI SHALL refuse to freeze a command under a name that already resolves to an executable on `PATH`, naming the conflicting program, because such a script would shadow it for every program the user runs. SAI SHALL NOT overwrite an existing frozen command without explicit confirmation. A refusal SHALL write no file.

#### Scenario: Name shadows an existing program

- **WHEN** the user tries to freeze a command as `find` while `find` resolves on `PATH`
- **THEN** SAI refuses, names the conflicting program, and writes nothing

#### Scenario: Name already frozen

- **WHEN** the user freezes a command under a name that already exists in the commands directory
- **THEN** SAI requires explicit confirmation before replacing it

#### Scenario: Declining a replacement

- **WHEN** the user declines to replace an existing frozen command
- **THEN** the existing script is left byte-identical and no new file is written

#### Scenario: A refused freeze leaves nothing behind

- **WHEN** freezing is refused for any reason
- **THEN** no file is created, modified, or partially written

### Requirement: Freezing is a reviewed step

Freezing SHALL present the preflight card for the command being frozen and require confirmation before writing the script. This holds however the command reached the freeze — generated in the same invocation or recovered from an earlier one — because freezing is the moment a command stops being reviewed per use.

#### Scenario: Card shown before writing

- **WHEN** a command is about to be frozen
- **THEN** the preflight card is presented and confirmation requested before any file is written

#### Scenario: Declining the freeze

- **WHEN** the user declines at the freeze confirmation
- **THEN** no script is written and the invocation ends reporting that nothing was frozen

#### Scenario: Review applies to a recovered command too

- **WHEN** the command being frozen was generated in an earlier invocation
- **THEN** the card is still presented, showing that command and its recorded safety mode, before anything is written

### Requirement: Saved names never shadow prompt text

The way a frozen command is named and invoked SHALL NOT make any natural-language prompt ambiguous. Text supplied as a prompt SHALL continue to be treated as a prompt regardless of what has been frozen.

#### Scenario: A prompt resembling a saved name

- **WHEN** the user runs `sai run the tests` while a frozen command exists
- **THEN** the text is treated as a natural-language prompt, as it always has been

#### Scenario: Freezing does not change ordinary invocation

- **WHEN** any number of commands have been frozen
- **THEN** ordinary generation, safety validation, confirmation and execution are unaffected

### Requirement: Freezing is unavailable on platforms without a supported script format

On a platform for which SAI does not emit a script format, freezing SHALL refuse with a message stating that the platform is not yet supported, rather than writing a script that cannot run. Every other SAI capability SHALL remain available on that platform.

#### Scenario: Freezing on an unsupported platform

- **WHEN** the user tries to freeze a command on a platform with no supported script format
- **THEN** SAI reports that freezing is not yet supported there, exits non-zero, and writes nothing

#### Scenario: The rest of SAI still works

- **WHEN** SAI runs on a platform where freezing is unavailable
- **THEN** generation, safety validation, the interactive editor, prompt history and execution all work normally
