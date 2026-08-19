# SAI Deterministic Commands Spec

Purpose: define a capability that lets a verified generated command be frozen under a name and re-executed deterministically, without a model in the loop.

Status: Proposed
Priority: P0 (ahead of SPEC-03 through SPEC-08 in `feature-list-v1.2.0-specs.md`)

---

## Rationale

SAI's natural-language-to-command core is not differentiated: several agent CLIs do the same thing, often better, because they read the project. What none of them offer is a way to stop asking. They re-derive on every invocation, so today's answer and tomorrow's may differ, each run costs tokens and latency, and none of it works offline.

Deterministic commands change what the model is for. It becomes a **compile-time dependency, not a runtime one**: consulted once to author a command, then not consulted again. The command that ran last week is byte-identical to the one that runs today.

This also resolves a tension in the current design rather than patching it. The tool whitelist constrains generation, which degrades output — a request to show Rust items from recently changed files returned only the `find` half, because the model was being obedient — and users learn to reach for `--unrestricted`. If the broad generation happens **once**, under full inspection, and is then frozen, the safety cost is paid once instead of on every invocation. It also removes confirmation fatigue: reviewing carefully one time is worth more than approving the same command for the fiftieth time.

### Why this is not a shell alias

An alias is deterministic, versionable and needs no tool, so the capability has to earn its place on something else. It earns it on **provenance**. A saved command carries the natural-language intent that produced it, the tools it was permitted, the safety mode it was frozen under, and when it was verified.

That gives the one thing an alias structurally cannot: **re-derivation**. When a frozen command stops working — a tool changed its flags, a path moved — the original intent is still there to regenerate from. The intent is the durable artifact; the command is a cache of it.

---

## SPEC-09: Deterministic Commands

### Problem

Every useful command SAI generates is discarded. A command that took three attempts and a careful review to get right cannot be recalled except as prompt text, which regenerates it — differently, at cost, and requiring review again.

### User Value

- Repeated work costs no tokens, no latency, and no network
- A verified command behaves identically every time it runs
- Careful review happens once, at the moment attention is worth spending
- The intent behind a command survives, so it can be regenerated when it breaks
- A vetted catalog can be shared or version-controlled

### Scope

**Authoring.** A deterministic command is created from a command SAI has already generated and the user has accepted, whether the prompt came from a command-line argument or was composed in the interactive editor. Both paths produce the same artifact.

**Execution.** Running a saved command performs no generation and contacts no model.

**Management.** Saved commands can be listed, inspected, edited, and deleted.

**Storage.** A dedicated file in the existing configuration directory, separate from `config.yaml`, `history.log` and `prompt_history.log`.

CLI surface, following the existing flag style (`--init`, `--analyze`, `--list-tools`) so that saved names can never collide with prompt text — `sai run the tests` remains an ordinary prompt:

- `sai --save <name>` — freeze the most recent successfully generated command
- `sai --save <name> "<prompt>"` — generate, review, and freeze in one step
- `sai --run <name>` — execute the saved command deterministically
- `sai --list-commands` — list saved commands
- `sai --show <name>` — print one command with its full provenance
- `sai --edit <name>` — edit the stored command text
- `sai --delete <name>` — remove a saved command

Each stored entry records: the name, the command text, the natural-language intent that produced it, the safety mode it was frozen under, the tools permitted at freeze time, the prompt config used, and the timestamp.

### Safety model

Execution honors **the safety mode the command was frozen under**, not the configuration in force today. A command frozen under `--unrestricted` keeps working; one frozen under default mode is validated against the whitelist as it stood at freeze time. This is what makes the artifact deterministic — configuration drift must not silently change what a saved command does.

Two controls survive freezing:

- `safety.allow_unrestricted: false` refuses to run a command frozen under unrestricted mode, so the machine-level kill switch is not defeated by saving a command first.
- Commands frozen under unrestricted mode are visibly marked in `--list-commands` and `--show`, because their provenance is the reason to look at them.

Freezing is the review gate. The preflight card (SPEC-04) belongs here: a locally computed risk summary is worth most at the moment a command is being made permanent.

### Acceptance Criteria

- A command generated from either an argument or the interactive editor can be frozen under a name
- Running a saved command produces no model call and no network traffic, and works offline
- The same saved command produces byte-identical output text on every run
- Saved commands survive across sessions and are stored in a dedicated file under the config directory
- List, show, edit and delete all operate on that file
- A saved command records the intent that produced it, and that intent is visible in `--show`
- Execution applies the safety mode recorded at freeze time
- A command frozen under unrestricted mode is refused when configuration forbids unrestricted mode, naming the file responsible
- Deleting a command is confirmed, and removes it permanently
- A name that already exists is not silently overwritten
- Invoking an unknown name fails clearly and suggests `--list-commands`
- Saved-command names never shadow prompt text: `sai run the tests` is still a prompt

### Out of Scope

- **Parameterized commands.** Turning a concrete generated command into a correct template is a separate and harder problem: if the model does it, run-time determinism is lost again; if the user hand-edits it, the result is a shell function. This must be decided on its own merits once the unparameterized form is in use.
- Composition of saved commands, conditionals, or control flow — that is a script runner, which the product boundary excludes.
- Automatic re-derivation when a command fails. The stored intent makes this possible later; nothing here performs it.
- Team sync, remote catalogs, or shared registries.
- Fuzzy or natural-language lookup of saved commands by intent.

---

## Product Boundary Guardrail

A named catalog of verified commands is not shell scripting, and this capability must not become one. The line to hold:

- Allowed: naming, storing, listing, editing, deleting, and executing single verified commands
- Disallowed: parameters, composition, conditionals, loops, or any construct that turns the catalog into a language

The moment saved commands can call each other or take arguments with logic attached, SAI has become the script interpreter that `feature-list-v1.2.0-specs.md` explicitly rules out for this line of work.

---

## Relationship to the Existing Roadmap

- **Supersedes in priority**: SPEC-03 (multi-line composition) and SPEC-05 through SPEC-08. Those improve an interaction that this capability makes rarer, since the point is to stop re-prompting.
- **Pulls forward**: SPEC-04 (preflight command card), which becomes the review gate at freeze time rather than a per-invocation convenience.
- **Builds on**: the prompt history and execution history already implemented, which between them hold the intent, the generated command, the exit code, and the safety mode — most of what an entry needs to record.
- **Depends on**: the `command-safety` capability, for the safety mode recorded at freeze time and honored at execution.

---

## Open Questions

- Whether `--edit` opens the stored command in the interactive mini editor or in `$EDITOR`. The mini editor is already built and keeps SAI self-contained; `$EDITOR` is the stronger convention for editing stored text.
- Whether a saved command's exit code and last-run time should be recorded back into its entry, which would make a stale or failing command visible in `--list-commands` without running it.
- Whether execution should require confirmation by default. The command was reviewed at freeze time, which argues no; but a destructive frozen command runs with no further gate, which argues for confirming at least those carrying risk markers.
