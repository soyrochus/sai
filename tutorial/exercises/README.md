# Exercises and capstone

Exercises intentionally omit a final implementation. Use the chapter workflow: define, ask, inspect, compile, test, refactor, reflect.

## Part I exercises

1. Add `--version-details` that reports the package version and Rust target without changing ordinary prompt parsing.
2. Introduce a `Prompt` newtype that rejects blank input at construction. Compare this with validating inside `run`.
3. Add an error variant or context message that distinguishes missing input from invalid UTF-8 assumptions.

## Part II exercises

4. Add a fixture for a valid model response and three malformed responses. Keep all tests offline.
5. Add a second config source and write down its precedence before implementing it.
6. Expand validation with one new deterministic rule. Explain why the system prompt alone is insufficient.
7. Add a dry-run executor that displays the exact program and argument vector without spawning it.

## Part III exercises

8. Implement a fake generator that records every prompt it receives. Use it to test scope context.
9. Add a safety-mode truth table to the tests before adding any new mode.
10. Ask AI for twenty adversarial operator strings. Select only cases whose expected result you can justify independently.

## Part IV exercises

11. Add a new optional history field while keeping a legacy JSON fixture readable.
12. Add one editor command as a pure state transition before wiring its terminal key.
13. Add a `--list-commands` warning for a missing tool without introducing an index file.

## Capstone

Choose one feature that is valuable but absent from your course implementation. Examples:

- A provider-neutral request timeout.
- Redaction of sensitive values in history.
- A `--json` output mode for automation.
- A command provenance verifier.
- Windows frozen-command emission.
- A local, non-AI fallback for common requests.

Your capstone must include:

1. A proposal explaining why the feature matters and what remains unchanged.
2. Behavioral requirements with normal, failure, and compatibility scenarios.
3. A design identifying model-controlled and deterministic decisions.
4. An ordered task list.
5. Tests written before or alongside implementation.
6. A review that maps every requirement to evidence.
7. A short retrospective: what AI accelerated, what it got wrong, and what Rust exposed.

The capstone is complete only when another learner can read the artifacts, run the checks, and explain why the feature is safe.

