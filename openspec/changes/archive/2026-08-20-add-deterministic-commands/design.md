## Context

See proposal.md — Why, and [specs/deterministic-commands-spec.md](specs/deterministic-commands-spec.md) for the full rationale behind emitting a script rather than building an internal runner.

What already exists and constrains the approach:

- `PreflightCard` and its pure `render() -> String` ([src/app.rs](src/app.rs)) are exactly the review instrument freezing needs, and they are already decoupled from the confirmation that follows them.
- `risk_markers(cmd_line) -> Vec<RiskMarker>` ([src/safety.rs:183](src/safety.rs#L183)) is pure and mode-agnostic.
- `validate_and_split_command` returns the shell-words token vector, which is what the quoting rule operates on.
- The executor's default path ([src/executor.rs:8](src/executor.rs#L8)) applies glob expansion to arguments containing `*`, `?` or `[`, falling back to the literal argument when nothing matches. This is the behavior the emitted script must reproduce, and it is the reason the naive "quote everything" rule is wrong.
- `config::config_root_dir()` ([src/config.rs:153](src/config.rs#L153)) resolves the per-user directory and has a thread-local test override, which every storage test in the project already uses.
- `ops.rs` already walks `PATH` to report tool presence ([src/ops.rs:352](src/ops.rs#L352)); the shadowing check and the staleness flag both need the same lookup.
- `HistoryEntry` has no `prompt` field, so the intent of an editor-composed prompt is unrecoverable from `history.log` — `argv` is just `["sai"]`.
- `shell-words` is already a dependency and provides quoting as well as splitting.

## Goals / Non-Goals

**Goals:**

- Emit an artifact that is correct under a shell for the mode it was frozen under — the one place where a subtle mistake silently changes what a verified command does.
- Keep the file the single source of truth. No index, no cache, nothing that can disagree with what is on disk.
- Reuse the existing review path rather than building a second one: the same card, the same markers, the same safety-mode vocabulary.
- Make every check that matters happen at freeze time, since nothing can happen at run time.

**Non-Goals:**

- Windows script emission. Deferred to its own change; `--save` refuses there.
- Parameters, composition, or any construct that turns the catalog into a language. What the *user* subsequently edits into their own file is their business.
- Observing, logging, or gating runs of a frozen command. Structurally impossible under this design and accepted as the cost of it.
- Re-derivation from the recorded intent. The header makes it possible later; nothing here performs it.

## Decisions

### The header is the data model

There is no `commands.json`. Provenance is written as `# sai:<field>` comment lines and read back by parsing them. `--list-commands` scans the directory, reads each file's leading comment block, and reports what it finds.

The `#` comment convention holds for bash and PowerShell alike, so the same header format will serve the deferred Windows work without redesign.

*Why not a registry:* two sources of truth drift the moment a user hand-edits a script, which is the entire point of the artifact being a script. A registry would then owe reconciliation rules, conflict resolution, and a "which side wins" answer. Parsing the file has none of those questions, and gives a property a registry cannot: a copied or committed script carries everything about itself.

*Cost:* listing is O(files) file reads rather than one index read, and a hand-mangled header degrades that entry's listing. Both are acceptable at the scale of a personal command catalog, and the spec requires an unparseable file to be skipped rather than to break the listing.

### Quoting follows the frozen mode, and globs are the exception

This is the subtlest part of the change and the easiest to get wrong.

- **Shell-executed modes** (`--unsafe`, `--unrestricted`): emit `cmd_line` verbatim. The operators are load-bearing; they were reviewed as such.
- **Default mode**: emit `shell_words::quote` on each token **except** tokens containing `*`, `?` or `[`, which are emitted bare.

The exception is not a nicety. Default mode never ran a shell, but it *did* glob-expand those arguments itself, so quoting everything would break `wc -l src/*` — a command that works today. Leaving glob tokens unquoted hands that expansion to the shell instead, and bash without `nullglob` has the same literal-fallback-on-no-match behavior the `glob` crate has, so the two agree.

*Residual difference, accepted:* an unquoted glob token containing whitespace would additionally be word-split by the shell. This is pathological in generated commands, and covering it would require `nullglob`-style shell configuration that changes other behavior.

*Alternative considered:* emit default-mode commands through `sai --run`-style re-execution to preserve exact semantics. Rejected — it puts SAI back in the execution path, which is the whole thing this design removes.

### The risk guard is emitted, not enforced

When `risk_markers` is non-empty at freeze time, the generated script carries a `read -rp` prompt that aborts on anything but an affirmative. It is script text like any other: visible on `cat`, removable by the user.

This answers "should a frozen command confirm before running?" once, at freeze, for exactly the commands that warrant it — rather than either gating everything (defeating the purpose) or gating nothing (a destructive frozen command running with no gate at all). It is also the only form of run-time protection available, since SAI is not there.

### Intent comes from a new history field

Add `prompt: Option<String>` to `HistoryEntry` with `#[serde(default)]`, mirroring exactly how `unrestricted` was added — the precedent is in the file, including its comment explaining why.

*Alternative considered:* correlating `history.log` and `prompt_history.log` by timestamp. Rejected: the pairing is a guess that breaks under concurrent shells, when generation failed after the prompt was recorded, and when the user declined at the confirmation. A wrong intent in a provenance header is worse than an absent one, because it will be trusted.

The field also improves `--analyze`, which currently infers the request from `argv` and therefore sees nothing for editor-composed prompts.

### Freezing reuses the confirmation path, not a new one

Freezing builds the same `PreflightCard` and asks for confirmation before writing. For a command recovered from history the card is built from the recorded entry — command, safety mode, scope, and now intent — rather than from a fresh generation.

The unrestricted-mode announcement and its typed-`yes` rule are **not** reused for the freeze confirmation. Freezing writes a file; it does not execute anything. The `allow_unrestricted` refusal is what governs unrestricted commands here, and it happens before the card is ever built.

### Writing is atomic, and refusals write nothing

Write to a temporary file in the target directory, `chmod` it, then rename into place. A partially written executable on `PATH` is a genuinely bad failure mode — the shell would happily run a truncated script — and rename-into-place costs one line to avoid it.

All refusals (`PATH` shadowing, existing name declined, `allow_unrestricted`, unsupported platform, nothing to freeze) are checked before the temporary file is created.

### Platform gating is a compile-time boundary

Script emission lives behind `#[cfg(unix)]`, with the non-Unix path returning the "not yet supported" refusal. This keeps the Windows question genuinely open rather than half-answered by a bash script nothing can run, and it means the deferred change adds a branch rather than rewriting one.

## Risks / Trade-offs

- **SAI writes an executable onto the user's `PATH` for the first time** → The blast radius of a bug here is larger than anywhere else in the codebase: a malformed or mis-quoted script runs with the user's full authority, repeatedly, without further review. Mitigation: atomic write, the shadowing refusal, the freeze-time card, and quoting rules that are unit-tested against the executor's actual glob behavior rather than assumed.

- **The `PATH` shadowing check is a point-in-time test** → A binary installed after the freeze can be shadowed by a script that was legitimate when written. Mitigation: none available at run time, by design. `--list-commands` flagging tools that have *left* `PATH` gives a partial view; a full answer would require SAI in the execution path.

- **Quoting correctness is load-bearing and easy to regress** → A future refactor that "simplifies" the glob exception would silently break every frozen command using a wildcard. Mitigation: the exception is a spec requirement with its own scenario, and the tests should assert the emitted text for a glob argument specifically, with a comment naming the executor behavior it mirrors.

- **`history.log` gains a field carrying natural-language text** → Prompts can contain host names, paths and other environment detail. `prompt_history.log` is already created owner-only for exactly this reason; `history.log` is not. Worth deciding whether it should be, as a small follow-on.

- **Freeze-time-only enforcement is a real reduction in what configuration can guarantee** → `allow_unrestricted: false` no longer means "no unrestricted command runs on this machine". The proposal states this plainly rather than leaving it to be discovered; the spec encodes the narrowed guarantee so it cannot be quietly re-broadened.

## Migration Plan

Additive throughout. `history.log` entries written by older versions parse unchanged via `serde(default)`, reading as having no recorded intent; a command frozen from such an entry gets a header marking the intent unavailable.

No commands directory exists until the first freeze, so an installation that never uses the feature is untouched. Rollback is reverting the commit; any scripts already emitted keep working, since they do not depend on SAI.
