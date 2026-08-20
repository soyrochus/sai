## Context

See proposal.md — Why.

The constraints that shape this design come from the existing confirmation path in [src/app.rs](src/app.rs):

- `run_with_reader` computes everything the card needs and then discards most of it. `validate_and_split_command` returns `tokens: Vec<String>` whose `tokens[0]` is the program the whitelist checked; `safety_mode` is in scope; `effective_explain` is derived from three distinct sources that are collapsed into one boolean; `cli.scope` and `prompt_source` are in scope.
- `print_confirm_context` takes six positional arguments and is called by both `confirm` and `confirm_unrestricted`. Adding fields by extending that signature is the obvious move and the wrong one — it is already at the limit where positional arguments stop being readable.
- `risk_markers(cmd_line) -> Vec<RiskMarker>` in [src/safety.rs:183](src/safety.rs#L183) is pure and already documented as such: no model, no filesystem, no execution. It is currently called from exactly one place, `confirm_unrestricted` ([src/app.rs:428](src/app.rs#L428)).
- `should_force_explain` ([src/prompt.rs:67](src/prompt.rs#L67)) matches the command's first whitespace-delimited token against tool configs. Note it splits on whitespace, while `validate_and_split_command` uses `shell_words`; for the card's primary-tool field the shell-words token is the correct one, since that is what the whitelist actually checked.
- Confirmation output goes to stderr throughout, so the card inherits stream separation for free.

## Goals / Non-Goals

**Goals:**

- Build the card from data already resolved in `run_with_reader`, computing nothing new except formatting.
- Make card content a pure function of a struct, so it is unit-testable as a string without a terminal or a reader.
- Preserve `confirm` and `confirm_unrestricted` as separate functions with their separate affirmative rules. The card is a shared prelude, not a merge of the two.
- Keep the unrestricted path's distinguishing announcement outside the card, so a future edit to card formatting cannot quietly weaken it.

**Non-Goals:**

- Any card on the no-confirmation path. See proposal.md — the decision and its reasoning.
- Colour, boxes, or terminal-width-aware layout. Plain aligned text on stderr, which stays readable when redirected to a file or read by a screen reader.
- Making the card configurable — no field toggles, no `--no-card`. Adding a flag that suppresses inspection output would cut against the existing rule in [src/cli.rs](src/cli.rs) that no flag may reduce scrutiny, which has a test asserting no such flag exists.
- Deriving new risk analysis. The card displays `risk_markers` as it stands; extending marker coverage is separate work.

## Decisions

### A `PreflightCard` struct built at one call site, rendered by a pure function

Introduce a struct carrying borrowed views of what the card reports — prompt, command, primary tool, scope, safety mode, explain source, markers, config provenance — plus `fn render(&self) -> String`. `run_with_reader` builds it once at the point where all the inputs are in scope; `confirm` and `confirm_unrestricted` each take `&PreflightCard` in place of the five context arguments they take today.

*Alternative considered:* extend `print_confirm_context` with four more positional parameters. Rejected: ten positional arguments of which several are `Option<&str>` is a call site nobody can read, and it leaves the content untestable except by capturing stderr.

*Why `render` returns a `String` rather than writing to a stream:* the assertions worth making are about content — "the card names `rg` as the primary tool", "the card states no markers were found" — and those are string assertions. Printing stays a one-line responsibility of the caller.

### Explain source becomes an enum, replacing a collapsed boolean

`effective_explain` currently loses the distinction between its three causes the moment it is computed:

```
safety_mode.forces_inspection() || cli.explain || tool_requires_explain
```

Replace it with an `ExplainSource` enum — `None`, `Flag`, `ToolConfig(String)`, `UnrestrictedMode` — resolved in the same precedence order, with `effective_explain` derived from it as "not `None`". Precedence matters and should be explicit: an unrestricted run that also passes `--explain` reports the mode, because the mode is the reason that cannot be removed.

This also retires the loose `Note: This tool requires explanation mode` line printed today at [src/app.rs:300](src/app.rs#L300). That note exists precisely because the boolean lost the information; the card carries it in a structured field instead.

### The primary tool comes from `tokens[0]`, not from re-splitting

`validate_and_split_command` already produced the token vector, and `tokens[0]` is by construction the string the whitelist was checked against. Re-deriving it with `split_whitespace`, as `should_force_explain` does, would risk the card naming a different tool than the one that was actually validated — for a quoted or path-prefixed program the two disagree. Pass the tokens through.

*Follow-on worth noting but not fixing here:* `should_force_explain`'s whitespace split has the same latent disagreement. Out of scope; the card will simply report what `force_explain` actually matched on.

### Markers generalize by moving the call, not by changing the function

`risk_markers` is already pure and mode-agnostic. Generalizing to every confirmation means calling it when the card is built rather than inside `confirm_unrestricted`, and rendering "none found" for the empty case. No change to [src/safety.rs](src/safety.rs) at all.

The explicit "none found" matters: a blank field is ambiguous between "computed, clean" and "not computed", and the whole value of a locally-computed check is that the user can tell it ran.

### Layout: aligned label column, single-line values, markers indented beneath

A fixed-width label column with one value per line, and multiple risk markers listed under the Risk label. Fields that do not apply — scope on an invocation with no scope hint, explain source on a confirmation with no explanation — are omitted entirely rather than rendered with a placeholder, keeping the block short in the common case.

*Alternative considered:* a bordered box. Rejected: it costs horizontal room, degrades when the command is longer than the terminal is wide, and adds nothing a label column does not.

The generated command is not truncated, however long it is. A card that hides part of what is about to run would defeat its own purpose.

## Risks / Trade-offs

- **The confirmation's visible output changes for every user who confirms anything** → This is a UX change, not just an addition; someone reading SAI's output with a script would see different text. Mitigation: the change is stderr-only, no exit code or history entry moves, and the CHANGELOG entry calls it out explicitly. There is no machine-readable contract on this output today to break.

- **Field creep making the card not compact** → Eight fields is already at the limit of what reads at a glance, and every future feature will want a row. Mitigation: the spec fixes the minimum field set, and omitting inapplicable fields keeps the typical card to five or six lines. Any addition should have to argue against the word "compact" in SPEC-04.

- **`ExplainSource` precedence encoding a wrong answer** → If an unrestricted run that also passed `--explain` reported `Flag`, a user could conclude that dropping the flag would drop the explanation, which is false. Mitigation: the precedence is asserted in a test naming that exact combination.

- **The unrestricted announcement drifting into the card** → A later refactor could reasonably decide the "no whitelist in effect" line is redundant with the card's safety-mode row and delete it, weakening a deliberate safety statement. Mitigation: the spec makes the separate announcement a requirement with its own scenario, so removing it fails validation rather than review.

## Migration Plan

No data migration and no persistent state. The change is confined to presentation on the confirmation path, and rollback is reverting the commit.
