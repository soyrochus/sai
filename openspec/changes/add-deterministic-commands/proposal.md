## Why

Every useful command SAI generates is discarded. A command that took three attempts and a careful review to get right cannot be recalled except as prompt text — which regenerates it, differently, at cost, and requiring review again. Deterministic commands make the model a **compile-time dependency rather than a runtime one**: consulted once to author a command, then not consulted again.

The full rationale, including why this is not a shell alias and why the artifact is a script rather than an internal runner, is in [specs/deterministic-commands-spec.md](specs/deterministic-commands-spec.md).

## What Changes

- Add `sai --save <name>` to freeze a verified generated command as an **executable bash script** in a commands directory the user puts on `PATH`. The frozen command is then invoked as `cleanlogs`, with SAI entirely out of the execution path.
- Add `sai --save <name> "<prompt>"` to generate, review, and freeze in one step.
- Add `sai --list-commands`, which scans the commands directory and parses each script's header. There is no registry and no index — the script file is the single source of truth, so nothing can drift out of sync.
- Add `sai --commands-path`, printing the directory so it can be put on `PATH`. `--init` prints the `export PATH=...` line to add. SAI never edits shell startup files.
- Add an optional `commands.dir` config key overriding the default location (`<config dir>/bin`).
- Emit provenance into each script as `# sai:<field>` header comments: intent, freeze time, safety mode, permitted tools, prompt config, and risk markers when present. Copying or committing the script carries its provenance with it.
- Emit a **confirmation guard into the script** when risk markers were present at freeze time, so the decision is made once by a human at freeze and then travels with the artifact.
- **Move the unrestricted kill switch to freeze time**: `safety.allow_unrestricted: false` refuses to *create* a script from an unrestricted-mode command. It can no longer gate execution, because SAI is not in the execution path.
- Refuse a name that already resolves on `PATH`, and never overwrite an existing script without explicit confirmation.
- **BREAKING (log format, additive)**: record the submitted prompt in each `history.log` entry. Without it, `sai --save <name>` cannot recover the intent of an editor-composed prompt, since `argv` is just `["sai"]`. The field is `#[serde(default)]` like the existing `unrestricted` field, so entries written by older versions still parse.
- Windows is explicitly out of scope for this change: `--save` fails there with a clear message. See Decisions.

### Decisions carried from clarification

- **Intent comes from a new `prompt` field in the history log**, not from correlating `history.log` and `prompt_history.log` by timestamp. Timestamp correlation is a guess that breaks on concurrent shells, on a generation that failed after the prompt was recorded, and on a confirmation the user declined. The field also improves `--analyze`, which today infers the request from `argv`.
- **Unix first; Windows follows in its own change.** Unsigned `.ps1` files are blocked by the default execution policy, and `.cmd` has quoting rules unlike either bash or PowerShell. Neither question is settled, and neither should hold up the design that is.
- **`--list-commands` flags tools that have left `PATH`.** These commands are meant to be long-lived, so a check that surfaces a command which will fail before it is run is worth more here than in `--list-tools`, where the same mechanism already exists.

## Capabilities

### New Capabilities

- `deterministic-commands`: Freezing a verified generated command into an executable script on `PATH` — the artifact's format and provenance header, the quoting rule that preserves the frozen safety mode's semantics, the commands directory and its configuration, listing, and the freeze-time refusals.

### Modified Capabilities

- `command-safety`: `safety.allow_unrestricted: false` gains a second effect — refusing to freeze an unrestricted-mode command — and the requirement that it govern execution is narrowed to SAI's own execution path, since a frozen script runs without SAI. The preflight card requirement gains freezing as a second occasion on which the card is shown.

## Impact

- **Code**: new `src/commands.rs` (script emission, header parsing, directory scan, name checks); [src/cli.rs](src/cli.rs) gains `--save`, `--list-commands`, `--commands-path`; [src/app.rs](src/app.rs) gains a freeze path reusing the existing `PreflightCard`; [src/config.rs](src/config.rs) gains `commands.dir`; [src/history.rs](src/history.rs) gains the `prompt` field; [src/ops.rs](src/ops.rs) gains the `--init` PATH hint and can share its existing `PATH` lookup ([src/ops.rs:352](src/ops.rs#L352)).
- **Reused as-is**: `PreflightCard` and `ExplainSource` ([src/app.rs](src/app.rs)), `risk_markers` ([src/safety.rs:183](src/safety.rs#L183)), `SafetyMode` ([src/safety_mode.rs](src/safety_mode.rs)), and the NDJSON read/rotate pattern in [src/history.rs](src/history.rs).
- **Unchanged**: generation, tool restriction, operator blocking, the interactive editor, prompt history, and the execution path for ordinary runs.
- **Dependencies**: none added. `shell-words` is already present and provides the quoting primitive.
- **Filesystem**: one new directory under the config root, holding user-executable files. This is the first time SAI writes an executable.
- **Docs**: a new `commands` help topic, README, CHANGELOG, and [docs/TECHSPEC.md](docs/TECHSPEC.md).
