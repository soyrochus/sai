## 1. Safety mode representation

- [ ] 1.1 Add a `SafetyMode` enum (`Default`, `Unsafe`, `Unrestricted`) with helpers for the questions call sites actually ask: does it skip operator blocking, does it lift the tool restriction, does it force inspection, does it execute through the shell.
- [ ] 1.2 Derive the mode from the parsed CLI in one pure function, so `unrestricted && !unsafe` is unrepresentable rather than merely avoided.
- [ ] 1.3 Unit-test the derivation for every flag combination, including `--unrestricted` alone implying shell execution and skipped operator blocking.

## 2. CLI surface

- [ ] 2.1 Add `--unrestricted` to `Cli` in [src/cli.rs](src/cli.rs), documented as lifting the tool whitelist and forcing inspection.
- [ ] 2.2 Ensure it composes with `--unsafe` and `--explain` rather than conflicting, and that no flag can suppress explanation or confirmation under it.
- [ ] 2.3 Add parse tests covering `--unrestricted` alone and combined with `-u`, `-e`, and `-c`.

## 3. Configuration gate

- [ ] 3.1 Add a `safety` section to `GlobalConfig` in [src/config.rs](src/config.rs) with `allow_unrestricted: Option<bool>`, absent meaning allowed.
- [ ] 3.2 Check the gate immediately after the config loads and before the system prompt is built, so a forbidden run costs no tokens and writes no command to history.
- [ ] 3.3 On refusal, exit non-zero with a message naming the config file that forbade it.
- [ ] 3.4 Test: refusal exits non-zero and names the file; refusal happens without contacting the generator; an absent setting allows the mode; the setting does not affect invocations without the flag.

## 4. Lifting the restriction in generation

- [ ] 4.1 Thread `SafetyMode` into `build_system_prompt` in [src/prompt.rs](src/prompt.rs).
- [ ] 4.2 Under `Unrestricted`, omit the tools listing and the "you may ONLY use" instruction, keeping the configured tool descriptions as non-binding guidance so their domain knowledge survives.
- [ ] 4.3 Test that the unrestricted system prompt contains no exclusive-tool instruction while the default one still does, and that tool descriptions are present in both.

## 5. Lifting the restriction in validation

- [ ] 5.1 Make the whitelist check in `validate_and_split_command` ([src/safety.rs](src/safety.rs)) conditional on the mode.
- [ ] 5.2 Keep parsing and the empty-command rejection unconditional, so a malformed command is still rejected under `--unrestricted`.
- [ ] 5.3 Test: an unconfigured tool passes under `Unrestricted` and is rejected under `Default` and `Unsafe`; operators pass under both `Unsafe` and `Unrestricted`; an unparseable command is rejected in every mode.

## 6. Deterministic risk markers

- [ ] 6.1 Add a pure `risk_markers(cmd_line: &str) -> Vec<RiskMarker>` in [src/safety.rs](src/safety.rs) that touches neither the filesystem nor the model.
- [ ] 6.2 Mark shell operators, reusing the existing quote-aware scanner from `detect_forbidden_operator` so quoted text is not misread.
- [ ] 6.3 Mark recursive and forced deletion.
- [ ] 6.4 Mark wildcard breadth, judged by the path the wildcard sits in — rooted at `/`, `~`, or a parent traversal counts as broad — erring toward marking, since an overstated marker is an annoyance and an understated one is a hazard.
- [ ] 6.5 Test each marker class, plus quoted operators not being marked, and assert the function is side-effect free and deterministic across repeated calls.

## 7. Mandatory inspection

- [ ] 7.1 Compute effective explain and confirm from the mode in [src/app.rs](src/app.rs), so under `Unrestricted` both are unconditionally true with no configuration consulted.
- [ ] 7.2 Add `confirm_unrestricted()` alongside the existing `confirm()`, sharing a small helper for reading the answer and leaving `confirm()` untouched.
- [ ] 7.3 Accept only `yes` after trimming and lowercasing; reject a bare `y` and everything else, cancelling without executing.
- [ ] 7.4 State in the prompt that no tool restriction is in effect, and render the risk markers above the question.
- [ ] 7.5 Test: `yes` executes; `y` does not; empty input does not; ordinary `confirm()` still accepts `y`; explanation and confirmation happen under `--unrestricted` even without `--explain` or `--confirm`.

## 8. Execution and audit

- [ ] 8.1 Route `Unrestricted` through the existing shell-execution path in [src/executor.rs](src/executor.rs) rather than adding a second branch.
- [ ] 8.2 Add an `unrestricted` field to `HistoryEntry` in [src/history.rs](src/history.rs), marked `#[serde(default)]` so entries written before this change still parse.
- [ ] 8.3 Populate it from the run's mode via `RunSummary`.
- [ ] 8.4 Test: an unrestricted run is recorded as such, an ordinary run is not, and a history line lacking the field still parses and reads as not unrestricted.

## 9. Documentation

- [ ] 9.1 Add a `sai help unrestricted` topic covering what it lifts, the typed-`yes` confirmation, the config gate, and — stated plainly — that the explanation comes from the same model that wrote the command and is therefore not an independent check.
- [ ] 9.2 Correct `templates/help/unsafe.txt`, which currently states the tool whitelist is always kept and becomes wrong with this change.
- [ ] 9.3 Document the flag and `safety.allow_unrestricted` in [README.md](README.md), including how to forbid the mode on a shared machine.
- [ ] 9.4 Add a CHANGELOG entry.
- [ ] 9.5 Run `cargo test` and `cargo clippy` clean, and verify the end-to-end flow by hand: a command using an unconfigured tool is generated, explained, marked, and requires `yes`.
