# SAI Deterministic Commands Spec

Purpose: define a capability that lets a verified generated command be frozen into a real shell script on `PATH`, and re-executed deterministically without a model — or SAI — in the loop.

Status: Proposed
Priority: P0 (the next capability after v1.2.0)

---

## Rationale

SAI's natural-language-to-command core is not differentiated: several agent CLIs do the same thing, often better, because they read the project. What none of them offer is a way to stop asking. They re-derive on every invocation, so today's answer and tomorrow's may differ, each run costs tokens and latency, and none of it works offline.

Deterministic commands change what the model is for. It becomes a **compile-time dependency, not a runtime one**: consulted once to author a command, then not consulted again. The command that ran last week is byte-identical to the one that runs today.

This also resolves a tension in the current design rather than patching it. The tool whitelist constrains generation, which degrades output — a request to show Rust items from recently changed files returned only the `find` half, because the model was being obedient — and users learn to reach for `--unrestricted`. If the broad generation happens **once**, under full inspection, and is then frozen, the safety cost is paid once instead of on every invocation. It also removes confirmation fatigue: reviewing carefully one time is worth more than approving the same command for the fiftieth time.

### Why a script on PATH, not an internal runner

If the model is a compile-time dependency, then `sai --save` is a compiler, and the output belongs on disk in the target's native format. Compilers emit a file and get out of the way; they do not keep a private registry and a `--run` verb to dispatch through.

So a deterministic command **is a shell script** — `bash` on Unix, PowerShell on Windows — written to a configurable directory that the user puts on `PATH`. It is invoked as `cleanlogs`, not `sai --run cleanlogs`.

Three consequences, each of which is the actual argument for this design:

- **The per-invocation cost goes to zero.** The premise of the capability is to stop paying a cost every time. A mandatory `sai --run` prefix keeps one.
- **It composes with everything.** A script on `PATH` works in pipes, `xargs`, cron, Makefiles, other scripts, shell completion, `which` and `type`. An internal runner composes with none of that without additional work.
- **Half the management surface stops needing to exist.** `--show` is `cat`. `--edit` is `$EDITOR`. `--delete` is `rm`. Re-implementing those inside SAI would be rebuilding the filesystem.

It is also the design that matches SAI's stated philosophy. The first principle in the README is *"the shell remains in control — sai generates commands, it does not become a shell itself."* Emitting an artifact the shell owns and then stepping out of the execution path is that principle. Building an internal runner would quietly contradict it.

### Why this is not a shell alias

An alias is deterministic, versionable and needs no tool, so the capability has to earn its place on something else. It earns it on **provenance**.

A frozen command carries, in its own header comments, the natural-language intent that produced it, the tools it was permitted, the safety mode it was frozen under, and when it was verified:

```bash
#!/usr/bin/env bash
# sai:intent      show fn and struct from rust files changed in the last 30 min
# sai:frozen      2026-08-20T14:22:11Z
# sai:safety      unrestricted
# sai:tools       rg, find
# sai:prompt-cfg  global default
```

That gives the one thing an alias structurally cannot: **re-derivation**. When a frozen command stops working — a tool changed its flags, a path moved — the original intent is still there to regenerate from. The intent is the durable artifact; the command is a cache of it.

Because the provenance lives in the file rather than in a database, it survives every ordinary operation the user might perform: copying the script to another machine, committing the directory to git, or reading it six months later with `cat`.

### Why there is no registry

An earlier draft of this spec proposed a registry of saved commands that SAI would keep in sync with the emitted scripts. That is the wrong shape, and the sync is the reason.

Two sources of truth drift. The moment a user hand-edits a script — which is the entire point of the artifact being a script — a registry's copy of the command text is stale, and the design now owes an answer to reconciliation, conflict rules, and which side wins.

**The file is the single source of truth.** There is no registry, no index, and nothing to synchronize, because nothing is duplicated. `sai --list-commands` scans the directory and parses headers. This is strictly less machinery than a registry, and it is also strictly more durable, since a copied or committed script carries everything about itself.

---

## SPEC-09: Deterministic Commands

### Problem

Every useful command SAI generates is discarded. A command that took three attempts and a careful review to get right cannot be recalled except as prompt text, which regenerates it — differently, at cost, and requiring review again.

### User Value

- Repeated work costs no tokens, no latency, and no network
- A verified command behaves identically every time it runs
- Careful review happens once, at the moment attention is worth spending
- The frozen command is invoked like any other command on the system
- The intent behind a command survives, so it can be regenerated when it breaks
- A vetted catalog is an ordinary directory: inspectable, editable, and version-controllable

### Scope

**Authoring.** A deterministic command is created from a command SAI has already generated and the user has accepted, whether the prompt came from a command-line argument or was composed in the interactive editor. Both paths produce the same artifact.

**The artifact.** A single executable script per command, written to the commands directory. See *The script artifact* below.

**Execution.** The user runs the script directly from their shell. SAI is not in the execution path, performs no generation, and contacts no model.

**Management.** `sai --list-commands` reports what is in the directory. Everything else — inspecting, editing, deleting, moving, version-controlling — is done with ordinary tools on ordinary files.

**Storage.** A dedicated directory alongside `config.yaml`, `history.log` and `prompt_history.log` in the existing configuration directory, overridable by configuration:

| OS | Default |
| --- | --- |
| Linux | `~/.config/sai/bin` |
| macOS | `~/Library/Application Support/sai/bin` |
| Windows | `%APPDATA%\sai\bin` |

```yaml
commands:
  dir: ~/.local/share/sai-commands   # optional override
```

**CLI surface**, following the existing flag style (`--init`, `--analyze`, `--list-tools`) so that saved names can never collide with prompt text — `sai run the tests` remains an ordinary prompt:

- `sai --save <name>` — freeze the most recent successfully generated command
- `sai --save <name> "<prompt>"` — generate, review, and freeze in one step
- `sai --list-commands` — list what is in the commands directory, with provenance
- `sai --commands-path` — print the commands directory, for use in shell configuration

Three verbs, because the filesystem provides the rest.

### The script artifact

**Header.** Every emitted script carries provenance as `# sai:<field>` comments. The `#` comment convention holds for both bash and PowerShell, so one header format serves both platforms. Fields: `intent`, `frozen`, `safety`, `tools`, `prompt-cfg`, and `risk` when markers were present at freeze time.

**Body — quoting follows the safety mode it was frozen under.** This matters, and getting it wrong would silently change what a command does:

- A command frozen under **`--unsafe` or `--unrestricted`** was executed through a shell and its operators are load-bearing. The emitted script writes the command line verbatim.
- A command frozen under **default mode** never ran through a shell. SAI spawned its token vector directly, applying exactly one transformation: glob expansion of arguments containing `*`, `?` or `[`, falling back to the literal argument when nothing matched. Reproducing that under a shell means quoting each token **except** those carrying glob metacharacters, which are emitted unquoted so the shell performs the expansion instead. Bash without `nullglob` also falls back to the literal on no match, so the two agree.

  Quoting every token would suppress the expansion and break commands like `wc -l src/*`, which work today. Quoting none of them would hand the shell word splitting and substitution that default mode never granted. The residual difference is a glob token containing whitespace, which the shell would additionally word-split; this is pathological in generated commands and is accepted rather than worked around.

**Preamble.** Unix scripts open with `#!/usr/bin/env bash` and `set -euo pipefail`. Scripts are written with the executable bit set on platforms that have one.

**Arguments are not forwarded.** The script takes no parameters and does not pass `"$@"` through. A frozen command is one concrete verified command; see *Product Boundary Guardrail*.

**Risk guard.** When risk markers were present at freeze time, the guard is emitted into the script rather than enforced by SAI at run time:

```bash
# sai:risk        [destructive] rm — recursive and forced deletion
read -rp "This deletes files recursively. Continue? [y/N] " ok
[[ $ok == y ]] || exit 1
```

Baked in at freeze time, visible in the file, and removable by a user who disagrees. This is the right place for it: the decision is made once, by a human, at the moment the command is being made permanent — and the artifact then carries its own warning wherever it goes.

### PATH integration

SAI does not modify shell startup files. `sai --init` and `sai --save` print the line to add:

```bash
export PATH="$(sai --commands-path):$PATH"
```

This is a one-time user action, and it is the honest cost of the design: an internal runner would not need it. Editing a user's `.zshrc` or `$PROFILE` silently to avoid that cost is not a trade worth making.

### Safety model

Freezing is the review gate. The preflight card implemented in v1.2.0 is exactly the right instrument at exactly the right moment: a locally computed risk summary is worth most when a command is about to be made permanent, not on the fiftieth repetition of a command already reviewed.

Because SAI is not in the execution path, **every control must act at freeze time.** This is a deliberate relocation, not an oversight:

- **`safety.allow_unrestricted: false` refuses to create a script from a command generated under unrestricted mode.** It gates authoring, not execution. This preserves what the kill switch actually means — *this machine does not produce unwhitelisted commands* — while accepting that a script which already exists is an ordinary file. Gating execution would in any case be theatre: the user has a shell, and could write the same script by hand in ten seconds.
- **A name that already resolves on `PATH` is refused.** Freezing a command as `find` or `ls` would shadow a real binary for every program the user runs, with consequences ranging from confusing to destructive. SAI already performs `PATH`-presence checking for `--list-tools`, so the mechanism exists.
- **An existing script is never silently overwritten.** Re-saving an existing name requires explicit confirmation.
- **Commands frozen under unrestricted mode are visibly marked** in `--list-commands` and in the script's own header, because their provenance is the reason to look at them.

What is knowingly given up: SAI cannot observe, gate, or log runs of a frozen command. That is the point of the design, and the cost is accepted rather than mitigated.

### Acceptance Criteria

- A command generated from either an argument or the interactive editor can be frozen under a name
- Freezing writes one executable script to the commands directory, and nothing else
- The script carries its intent, freeze time, safety mode, permitted tools, and prompt config as header comments
- Running the script produces no model call and no network traffic, and works offline
- The same frozen command produces byte-identical output text on every run
- A command frozen under default mode is emitted with each token quoted except those carrying glob metacharacters, so the shell reproduces the glob expansion default mode performed and grants nothing further
- A command frozen under `--unsafe` or `--unrestricted` is emitted verbatim, with its operators intact
- A command carrying risk markers at freeze time emits a confirmation guard into the script
- `--list-commands` reports every script in the directory with its provenance, reading the files themselves and consulting no index
- Freezing is refused when the name already resolves on `PATH`, naming the conflicting binary
- Freezing is refused for an unrestricted-mode command when `safety.allow_unrestricted: false`, naming the file responsible
- An existing script is not overwritten without explicit confirmation
- `--commands-path` prints the directory, and `--init` prints the `PATH` line to add
- Saved-command names never shadow prompt text: `sai run the tests` is still a prompt
- SAI never edits shell startup files

### Out of Scope

- **Parameterized commands.** Turning a concrete generated command into a correct template is a separate and harder problem: if the model does it, run-time determinism is lost again; if the user hand-edits it, the result is a shell function — which, notably, the user is now free to do, because the artifact is a script they own. SAI does not generate parameters.
- Composition of saved commands, conditionals, or control flow.
- Automatic re-derivation when a command fails. The header keeps the intent, which makes this possible later; nothing here performs it.
- Recording last-run time or exit codes back into the artifact. SAI is not in the execution path and cannot observe runs. This is settled, not deferred.
- Team sync, remote catalogs, or shared registries. Committing the commands directory to git already covers most of what a shared catalog would offer.
- Fuzzy or natural-language lookup of saved commands by intent.

---

## Product Boundary Guardrail

A directory of verified single-command scripts is not shell scripting, and this capability must not become one. The line to hold is about **what SAI generates**, not what the user subsequently does with a file they own:

- Allowed: SAI naming, emitting, and listing single verified commands
- Disallowed: SAI generating parameters, composition, conditionals, loops, or any construct that turns the catalog into a language

The moment SAI emits scripts that call each other or take arguments with logic attached, it has become the script interpreter that the v1.2.0 feature list explicitly rules out.

A user who opens a frozen script and adds `"$@"` to it has written a shell script. That is their prerogative and their maintenance burden, and it is precisely the freedom that emitting a real file rather than a registry entry is meant to give them. It is not SAI becoming an interpreter.

---

## Relationship to the Existing Roadmap

- **Builds on what shipped in v1.2.0.** The preflight card (SPEC-04) becomes the review gate at freeze time. Prompt history and execution history between them already hold the intent, the generated command, the exit code and the safety mode — most of what a header needs to record.
- **Depends on** the `command-safety` capability, for the safety mode recorded at freeze time and for the quoting rule that follows from it.
- **Supersedes in priority** SPEC-05 through SPEC-08 of the (now deprecated) v1.2.0 feature list. Those improve an interaction that this capability makes rarer, since the point is to stop re-prompting.
- **Smaller than the previous draft.** Dropping `--run`, `--show`, `--edit` and `--delete`, along with the storage format and its sync logic, removes most of the implementation surface. What remains is script emission, a directory scan, and the freeze-time checks.

---

## Design Decisions Recorded

Settled during design review; listed so they are not silently reopened.

- **A script on `PATH`, not an internal `--run` dispatcher.** Zero per-invocation cost, composes with the rest of the system, and removes four CLI verbs. Consistent with "the shell remains in control".
- **No registry; the file is the single source of truth.** A registry would require sync with hand-edited scripts, and sync means drift, reconciliation and conflict rules. Provenance lives in header comments instead, and travels with the file.
- **`allow_unrestricted: false` gates creation, not execution.** SAI cannot enforce anything at run time under this design. Moving the gate to freeze time preserves the switch's real meaning; gating execution would be unenforceable and, given the user has a shell, largely symbolic.
- **The risk guard is emitted into the script.** It answers "should execution confirm?" once, at freeze time, for exactly the commands that warrant it, and leaves the answer visible and editable in the artifact.
- **Quoting follows the frozen safety mode.** Default-mode commands never saw a shell; emitting them unquoted into one would change their meaning.
- **`--edit` is not a SAI verb.** The prior open question of mini-editor versus `$EDITOR` dissolves once the artifact is a file: the user's editor, whichever it is.

---

## Open Questions

- Whether `--save` with no prior command in the session should offer the most recent entry from `history.log` rather than failing, and whether that is too implicit to be safe.
- Whether `--list-commands` should flag scripts whose recorded `tools` are no longer present on `PATH`, which would surface a command that is going to fail before it is run.
- Whether Windows should emit `.ps1` (needing an execution-policy story) or `.cmd` (more portable to invoke, more awkward to generate). This affects only the Windows artifact, not the design.
