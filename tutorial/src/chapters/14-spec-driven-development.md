# Chapter 14 — Specification-driven development with AI

You have built the major pieces of an AI-powered Rust application. The final skill is controlling a change that crosses those pieces without letting chat history become the only source of truth.

## Product goal

Plan, specify, implement, verify, and archive one feature through a durable OpenSpec change.

Choose a feature small enough to finish but broad enough to expose a real boundary. Good candidates include:

- `--dry-run` with machine-readable output;
- configurable history redaction;
- a timeout for generated processes;
- importing and validating a frozen command;
- a second model provider behind `CommandGenerator`.

## Rust concepts

The capstone consolidates cross-module ownership, exhaustive enum changes, serialization compatibility, trait boundaries, regression tests, and compiler-guided API design.

## Build

Use a short kebab-case change name, for example `add-history-redaction`. Create:

```text
openspec/changes/add-history-redaction/
  proposal.md
  design.md
  tasks.md
  specs/
    invocation-history/
      spec.md
```

The proposal answers why, what changes, capabilities, and impact. It should name security or compatibility effects rather than hiding them in implementation notes.

The delta specification describes behavior:

```markdown
## ADDED Requirements

### Requirement: Prompt text can be omitted from history

The application SHALL omit prompt text from new history records when history
redaction is enabled, while retaining command and exit-status fields.

#### Scenario: Redacted invocation

- **WHEN** history redaction is enabled and a command completes
- **THEN** the written entry contains no prompt text

#### Scenario: Existing history remains readable

- **WHEN** history contains records written before redaction was configured
- **THEN** those records still deserialize normally
```

The design records decisions and rejected alternatives. The task list turns requirements into small, ordered, verifiable units:

```markdown
## 1. Configuration

- [ ] 1.1 Add a typed redaction setting with a privacy-preserving default.
- [ ] 1.2 Test configuration parsing and precedence.

## 2. History behavior

- [ ] 2.1 Apply redaction while constructing `HistoryEntry`.
- [ ] 2.2 Test redacted, unredacted, and old-record cases.

## 3. Verification and documentation

- [ ] 3.1 Run formatting, tests, and Clippy.
- [ ] 3.2 Update user and technical documentation.
```

Keep requirements about observable behavior, design about engineering choices, and tasks about work. Mixing them makes future review much harder.

This repository’s completed example is [`openspec/changes/archive/2026-08-20-add-deterministic-commands`](../../../openspec/changes/archive/2026-08-20-add-deterministic-commands/).

## AI collaboration script

Use one role at a time:

> Explore this feature without editing code. Trace its data flow, identify affected capabilities, compatibility constraints, and safety questions. Separate facts found in the repository from recommendations.

Then:

> Create an OpenSpec proposal, delta specification, design, and ordered task list for `add-history-redaction`. Every requirement needs observable scenarios. Do not implement code yet.

Review and revise the artifacts before implementation:

> Challenge the artifacts for contradictions, missing failure scenarios, accidental scope expansion, and tasks that cannot be verified independently. Update all affected artifacts coherently.

Only then:

> Implement the approved tasks in order. After each group, run its focused tests and update the checkbox only when evidence exists. Preserve unrelated working-tree changes.

Finish with:

> Compare the implementation against every scenario and task. Run the repository quality gates. Report deviations before syncing the delta spec and archiving the change.

OpenSpec integrations differ: some installations expose agent workflows, others a CLI. Use the propose, apply, sync, and archive operations supplied by your installation; keep the artifacts above as the durable source of truth.

## Compiler conversation

A cross-module change will often produce ownership errors that reveal unclear APIs. Consider:

```rust
let effective_ai = resolve_ai_config(global_config.ai)?;
write_history(&global_config, entry)?;
```

If `ai` is an owned, non-`Copy` field, the first call partially moves `global_config`, so the later borrow fails with E0382. Do not add `.clone()` reflexively. Compare three designs:

```rust
resolve_ai_config(&global_config.ai)  // borrow when resolution only reads
resolve_ai_config(global_config.ai.clone()) // duplicate when ownership is useful
resolve_global_config(global_config) // consume the whole value intentionally
```

The correct choice communicates the API’s real ownership needs. Ask AI to explain the diagnostic, but use call sites and data lifetime to choose the repair.

Specifications also help with compiler-driven refactors. When an enum gains a variant, exhaustive matches locate policy decisions; the scenarios tell you what each updated branch must do.

## Tests

Build an evidence matrix before coding:

| Scenario | Test level | Evidence |
|---|---|---|
| redacted invocation | orchestration unit test | recorded `prompt` is `None` |
| unredacted invocation | orchestration unit test | prompt round-trips |
| old record | serialization unit test | missing field deserializes |
| config precedence | configuration unit test | higher source wins |
| user-visible behavior | CLI integration test | exit/status and output |

At completion run the project gates:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Also search for unchecked tasks:

```bash
rg --fixed-strings -- '- [ ]' openspec/changes/add-history-redaction/tasks.md
```

An empty result is necessary but not sufficient. Inspect the diff and connect every checked task to test output, documentation, or a deliberate manual verification.

## Review checklist

- Proposal, specs, design, and tasks agree on scope.
- Every normative requirement has observable scenarios.
- Compatibility and safety decisions are explicit.
- Implementation follows task order with focused evidence.
- Compiler fixes reflect intended ownership, not automatic cloning.
- Tests map back to scenarios.
- Delta specs are synced only after verification.
- Archive happens only when no required work remains.

## Checkpoint

Complete your selected change, then create the final tutorial checkpoint:

```bash
git add openspec src tests README.md docs
git commit -m "tutorial: complete a specification-driven feature"
git tag tutorial-14-spec-driven-development
```

Evidence: all automated gates pass; the task list contains no unjustified open items; manual-only checks are clearly reported; the delta spec is synced; and the completed change is archived with its artifacts intact.

## Stretch exercise

Give a fresh AI session only the archived change and repository—not the original chat. Ask it to explain the feature, its safety model, and its compatibility guarantees. Gaps in the explanation reveal missing durable context.

## Reflection

- Which decisions became clearer when written as scenarios before code?
- Where did compiler feedback improve the design rather than merely delay implementation?
- What evidence allowed you to mark a task complete?
- Can another developer continue the project without access to your AI conversation?

## Further learning

- [The Rust Book — Appendix D: Useful Development Tools](https://doc.rust-lang.org/book/appendix-04-useful-development-tools.html) — rustfmt, Clippy, and rust-analyzer, the tools behind this chapter's quality gate.
- [Brown's interactive Book — Fixing Ownership Errors](https://rust-book.cs.brown.edu/ch04-03-fixing-ownership-errors.html) — the same partial-move scenario this chapter's compiler conversation reuses.
- [Comprehensive Rust — Compiler Lints and Clippy](https://google.github.io/comprehensive-rust/testing/lints.html)

This is the last chapter. Next: the [capstone](../exercises/README.md#capstone), where you specify and implement a feature of your own choosing using the complete workflow.
