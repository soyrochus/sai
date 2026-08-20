## 1. Explain source becomes structured

- [ ] 1.1 Add an `ExplainSource` enum — `None`, `Flag`, `ToolConfig(String)`, `UnrestrictedMode` — to [src/app.rs](src/app.rs), with a resolver taking the safety mode, `cli.explain`, and the `should_force_explain` result.
- [ ] 1.2 Derive `effective_explain` from the enum as "not `None`" so `effective_confirm` and `RunSummary.explain` keep their current values exactly.
- [ ] 1.3 Test the precedence explicitly, including `--unrestricted` combined with `--explain` reporting the mode rather than the flag, and a `force_explain` tool under a plain run reporting the tool.
- [ ] 1.4 Remove the free-standing `Note: This tool requires explanation mode` line at [src/app.rs:300](src/app.rs#L300), now that the card carries it as a field.

## 2. The card structure and its renderer

- [ ] 2.1 Add a `PreflightCard` struct holding borrowed views of the prompt, command, primary tool, scope hint, safety mode, explain source, risk markers, and config provenance.
- [ ] 2.2 Implement `render(&self) -> String` as a pure function with a fixed-width label column, one value per line, and multiple risk markers listed and indented under the Risk label.
- [ ] 2.3 Omit inapplicable fields entirely — no scope hint, no explanation — rather than rendering a placeholder or a blank value.
- [ ] 2.4 Render "none found" for an empty marker list, so a clean command is distinguishable from a card where markers were never computed.
- [ ] 2.5 Emit the generated command in full, never truncated, however long it is.
- [ ] 2.6 Unit-test `render` as string assertions covering each field present, each field absent, multiple markers, no markers, and a command longer than a typical terminal width.

## 3. Wire the card into the confirmation path

- [ ] 3.1 Build the `PreflightCard` in `run_with_reader` where all inputs are in scope, taking the primary tool from `tokens[0]` — the string the whitelist actually checked — rather than re-splitting the command line.
- [ ] 3.2 Move the `risk_markers` call out of `confirm_unrestricted` to the card construction site, leaving [src/safety.rs](src/safety.rs) untouched.
- [ ] 3.3 Replace `print_confirm_context` with printing the rendered card to stderr, and change `confirm` and `confirm_unrestricted` to take `&PreflightCard` in place of their five context arguments.
- [ ] 3.4 Keep the card construction inside the `effective_confirm` branch so a run that executes without confirming produces no card and its output is byte-identical to today.
- [ ] 3.5 Verify the card prints after the explanation and immediately before the confirmation prompt.
- [ ] 3.6 Keep `confirm` and `confirm_unrestricted` separate, preserving the bare-`y` versus typed-`yes` distinction, and keep the "no tool restriction is in effect" announcement outside the card and before the typed-affirmative prompt.

## 4. Behavioral verification

- [ ] 4.1 Test that a card appears before an ordinary `--confirm` confirmation and before an unrestricted one, and that no card appears on a run that executes without confirming.
- [ ] 4.2 Test that risk markers now appear on ordinary and `--unsafe` confirmations, not only unrestricted ones — a piped `--unsafe` command and an `rm -rf` under plain `--confirm`.
- [ ] 4.3 Test that the card reports the scope hint when `--scope` was passed and omits the field when it was not.
- [ ] 4.4 Test that the card identifies a per-call prompt config file as the source when one was used, and the global default otherwise.
- [ ] 4.5 Confirm every existing confirmation test in `app.rs` still passes, adjusting only assertions that match the old header text — not any assertion about exit codes, execution, or cancellation.
- [ ] 4.6 Test that exit codes and execution-history entries are unchanged with the card present, and that building a card executes nothing and consults no model.

## 5. Documentation and close-out

- [ ] 5.1 Update the `safety` and `explain` help topics in [src/help.rs](src/help.rs) and their templates under [templates/help/](templates/help/) to describe the card and the fields it reports.
- [ ] 5.2 Update [README.md](README.md) with a sample card as it appears before a confirmation.
- [ ] 5.3 Add a CHANGELOG entry noting that confirmation output changed shape and that risk markers now appear on every confirmation rather than only under `--unrestricted`.
- [ ] 5.4 Run `cargo test` and `cargo clippy` clean.
- [ ] 5.5 Manually run a confirmation in each mode — `--confirm`, `--explain`, `--unsafe`, `--unrestricted` — and check the card reads at a glance, stays compact when fields are omitted, and remains legible when stderr is redirected to a file.
