## Why

The confirmation prompt asks the user to approve a command while showing them config file paths and their own prompt echoed back — everything except a straight answer to "what is this about to do, and how much should I trust it?". The facts that would answer that are all computed already: [src/safety.rs](src/safety.rs) derives risk markers from the command text, `SafetyMode` knows which restrictions are lifted, and `app.rs` knows whether explanation was demanded by a flag or forced by a tool. None of it reaches the screen except under `--unrestricted`. SPEC-04 puts those facts in one compact block at the moment the decision is made.

## What Changes

- Replace the confirmation header printed by `print_confirm_context` with a **preflight card**: a compact, aligned block carrying the natural-language prompt, the generated command, the primary tool detected, the scope hint if one was used, the safety mode, why explanation is happening (flag versus `force_explain` versus mode), the risk markers, and the config provenance.
- **Show risk markers on every confirmation**, not only under `--unrestricted`. A `--unsafe` run containing a pipe and an `--explain` run of `rm -rf` currently get no locally-computed check at all, even though the markers are already calculated and cost nothing.
- Add a **primary tool** field — the resolved program name from the validated token list, which the user never sees today despite it being the thing the whitelist actually governs.
- Add an **explain source** field naming why an explanation was produced: the `--explain` flag, a tool's `force_explain` setting, or the mandatory inspection of `--unrestricted`. Today a `force_explain` tool prints a loose "Note:" line and the other sources print nothing.
- Keep the unrestricted confirmation's distinguishing behavior intact: it still announces that no whitelist is in effect and still requires typing `yes` in full. The card is what precedes that line, not a replacement for it.
- The card **does not alter execution semantics**: it is printed to the error stream immediately before the confirmation prompt, changes no decision, and blocks nothing.

### Decisions carried from clarification

- **The card appears only where a confirmation is about to be shown.** A plain `sai "list json files"` executes straight through today with just its `>> command` line, and stays that quiet. SPEC-04's own acceptance criterion is "appears consistently before confirmation step"; a card on a run with no decision to make is output nobody reads.
- **The card replaces the existing confirmation header rather than being appended below it.** Appending would print the generated command three times in one screen — once on the `>>` line, once under "LLM output", once on the card — and "compact" is the point of the feature. The header's genuinely useful fields (prompt, scope, config provenance) are absorbed into the card.
- **Risk markers become a standard card field.** Restricting them to `--unrestricted` would make the card's shape vary by mode for no defensible reason: the markers are deterministic text analysis, equally valid in every mode.

## Capabilities

### New Capabilities

_None._ `command-safety` already declares confirmation and explanation as within its purpose ("tool restriction, operator blocking, explanation, and confirmation"), so what the confirmation presents belongs there rather than in a capability of its own.

### Modified Capabilities

- `command-safety`: Adds requirements for the preflight card's presence, its field set, and its non-interference with execution semantics. Generalizes the existing "Unrestricted invocations carry deterministic risk markers" requirement to cover every confirmation. Amends "Unrestricted mode forces inspection that cannot be suppressed" so the mode announcement is stated as part of the card rather than as a free-standing line.

## Impact

- **Code**: [src/app.rs](src/app.rs) — `print_confirm_context` becomes the card renderer; `confirm` and `confirm_unrestricted` call it with a struct rather than six positional arguments; the `tool_requires_explain` "Note:" line folds into the card's explain-source field. The primary tool comes from the `tokens` that `validate_and_split_command` already returns, so no new parsing.
- **Data already available, nothing new computed**: `risk_markers` and `RiskKind::label` ([src/safety.rs:183](src/safety.rs#L183)), `SafetyMode` ([src/safety_mode.rs](src/safety_mode.rs)), `cli.scope`, `cli.explain`, `should_force_explain`, and the resolved config paths.
- **Unchanged**: generation, tool restriction, operator blocking, execution, history logging, and every exit code. `RunSummary` gains nothing.
- **Dependencies**: none added.
- **Testing**: the card is a pure function from its inputs to a string, so its content is unit-testable without a terminal; the existing confirmation tests in `app.rs` already drive `confirm` through an injected reader.
- **Docs**: the `safety` and `explain` help topics in [src/help.rs](src/help.rs), README, and a CHANGELOG entry noting the changed confirmation output.
