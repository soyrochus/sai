## Context

See `proposal.md` — Why.

The constraints come from how the safety layers are currently arranged:

- The tool restriction is enforced **twice**. [src/prompt.rs:26](src/prompt.rs#L26) writes `"You may ONLY use the following tools:"` into the system prompt, and [src/safety.rs:17-23](src/safety.rs#L17-L23) rejects a command whose first token is not in `allowed_tools`. The prompt-side restriction is the one that actually shapes output.
- Operator blocking is already conditional on `unsafe_mode` ([src/safety.rs:25-33](src/safety.rs#L25-L33)), and `ShellCommandExecutor` already branches on it to run through `sh -c` versus direct exec with glob expansion ([src/executor.rs:44-60](src/executor.rs#L44-L60)). Unrestricted mode needs the same execution path, so it can reuse the existing `unsafe_mode` plumbing rather than adding a parallel one.
- `confirm()` accepts `y` or `yes` ([src/app.rs:366](src/app.rs#L366)) and reads through the injected `&mut dyn BufRead`, so a stricter variant is testable the same way the existing one is.
- `GlobalConfig` has no `safety` section yet ([src/config.rs](src/config.rs)); every field is `Option` with `#[serde(default)]`, so adding one is backward compatible by construction.
- `HistoryEntry` is a flat serde struct with no `#[serde(default)]` on its fields, which matters for reading logs written before a new field existed.

## Goals / Non-Goals

**Goals:**

- Lift the restriction in both places it is enforced, since lifting only validation produces a flag that changes nothing observable.
- Make the mandatory inspection structurally impossible to suppress, rather than merely defaulted on.
- Reuse the existing `unsafe_mode` execution path instead of introducing a second shell-execution branch.
- Keep every non-unrestricted invocation byte-for-byte unchanged.

**Non-Goals:**

- The full preflight command card (SPEC-04). This change specifies only the risk markers `--unrestricted` needs and renders them inline in its own confirmation.
- Profiles (SPEC-08). The requirement that configuration cannot reduce scrutiny is written so profiles inherit it when they land, but no profile machinery is built here.
- Any sandboxing, dry-run, or rollback of executed commands.

## Decisions

### A single `SafetyMode` replaces two loosely related booleans

`unsafe_mode: bool` threaded through generation, validation and execution becomes an enum — `Default`, `Unsafe`, `Unrestricted` — carried on the run. `Unsafe` and `Unrestricted` both select shell execution and skip operator blocking; only `Unrestricted` lifts the tool restriction and forces inspection.

Rationale: `--unrestricted` implies `--unsafe`, so representing them as two independent booleans invites the combination `unrestricted && !unsafe`, which must never exist. An enum makes that state unrepresentable rather than merely avoided. It also gives the history field and the confirmation text a single source of truth.

Alternative considered: add `unrestricted: bool` alongside `unsafe_mode` and set both. Rejected — every call site would have to remember the invariant, and `executor.execute(.., unsafe_mode)` would start meaning "unsafe or unrestricted", which is exactly the kind of implicit coupling that goes wrong later.

### The system prompt gets an unrestricted variant, not a tweak

`build_system_prompt` takes the mode and, under `Unrestricted`, omits the tools listing and the "ONLY" instruction, replacing them with the configured tool descriptions as *guidance* rather than a closed set.

Rationale: this is the decision that makes the flag do anything at all. The tool descriptions still carry useful domain knowledge (how the user likes `jq` invoked, for instance), so discarding them entirely would make unrestricted output worse, not freer. Keeping them as non-binding guidance preserves that knowledge while removing the ceiling.

Open consequence worth stating: unrestricted generation is less predictable by design. That is the trade the user is making, and it is why inspection is mandatory rather than defaulted.

### Mandatory inspection is computed, never read from configuration

The effective explain and confirm decisions become a function of the mode: under `Unrestricted` they are `true` unconditionally, with no configuration or flag consulted. The existing `effective_explain`/`effective_confirm` calculation in [src/app.rs](src/app.rs) keeps its current behaviour for the other modes.

Rationale: the requirement is that nothing can suppress inspection. A default that configuration could override would satisfy the letter and miss the point. Computing it from the mode means a future profile cannot accidentally turn it off, because there is no input for it to turn off.

### The strict confirmation is a separate function, not a parameter

`confirm_unrestricted()` sits alongside `confirm()` rather than adding a `strict: bool` to it.

Rationale: the two prompts differ in text, in what they display (risk markers), and in what they accept. A boolean parameter would produce a function with two modes and interleaved branches. Two functions sharing a small helper for reading the answer is clearer, and it keeps the existing `confirm()` — which many tests already exercise — untouched.

The strict variant compares against `yes` after trimming and lowercasing, so `YES` and `Yes` are accepted; anything else, including `y`, is not.

### Risk markers are pure string analysis over the command text

Markers are computed by a function from `&str` to a list of markers, reusing `detect_forbidden_operator`'s existing quote-aware scanner for the operator marker and adding flag and wildcard checks. No filesystem access, no execution, no model.

Rationale: the marker's whole value is being an independent signal, so it must not depend on the model that wrote the command. Purity also makes it exhaustively testable and makes SPEC-04's card a matter of rendering the same data differently.

Wildcard breadth is judged by the path the wildcard sits in — a pattern rooted at `/`, `~`, or a parent traversal is broad; one rooted in the working directory is not. This is a heuristic and is labelled as advisory in the spec, deliberately: a marker that overstates is an annoyance, one that understates is a hazard, so it should err toward marking.

### The config gate is checked before generation

`safety.allow_unrestricted` is read from `GlobalConfig` and checked immediately after the config loads, before the system prompt is built or the model is contacted.

Rationale: a forbidden mode should cost nothing — no tokens, no latency, no history entry describing a command that was never permitted to exist. Checking early also gives an error that names the config file, which is the actionable part.

`Option<bool>` defaulting to `true` when absent keeps existing configs working; only an explicit `false` forbids.

### `HistoryEntry` gains `#[serde(default)]` on the new field

Rationale: `read_latest_entry` parses each line with `serde_json::from_str` and skips lines that fail. Without a default, every pre-existing entry would fail to parse and `--analyze` would silently report no history rather than reading the log it has. The default makes older entries load as not-unrestricted, which is what they were.

## Risks / Trade-offs

- **The forced explanation is not an independent check** — it comes from the model that wrote the command, so a destructive command can be described calmly → mitigated, not solved, by the locally computed risk markers. Stated plainly in the spec and in the help topic rather than papered over. Full mitigation is SPEC-04's preflight card.
- **Unrestricted generation is less predictable, which is the point** → mandatory inspection plus a typed `yes` is the compensating control; the config gate exists for environments where that trade is not acceptable at all.
- **Wildcard-breadth marking is heuristic and will produce false positives** → tuned to over-mark rather than under-mark, and labelled advisory in the spec so it is never mistaken for a guarantee.
- **`--unrestricted` could become a habit, eroding the whitelist's value** → the typed `yes` is deliberate friction. Worth revisiting if usage data ever shows it being reached for routinely, which would more likely indicate an under-configured tool list than a flag problem.
- **Reusing `unsafe_mode`'s execution path means unrestricted commands run through `sh -c`** → already true for `--unsafe`, so no new exposure; the change is which commands can reach that path.
- **The `SafetyMode` refactor touches signatures across `prompt.rs`, `safety.rs`, `executor.rs` and `app.rs`** → mechanical, and the existing test suite covers the default and unsafe paths, so a regression in them should surface immediately.

## Migration Plan

No data migration. The flag is additive and every existing invocation is unchanged. `HistoryEntry`'s new field defaults for older log lines, so `--analyze` keeps working against logs written before this change.

Rollback is removing the flag, the config field and the mode enum; a `history.log` containing the extra field stays readable by an older build, since serde ignores unknown fields by default.

## Open Questions

- Whether the risk-marker list should be extended with tool-specific markers (for example `chmod` on a broad path, or `curl … | sh`). Deferrable: adding markers later changes what is displayed, not the spec's requirement that markers exist and are advisory.
- Whether `--unrestricted` should also be reachable as a config-level default for a future `expert` profile. Deferred to SPEC-08 deliberately — the requirement that configuration cannot *reduce* scrutiny already constrains how a profile may interact with it.
