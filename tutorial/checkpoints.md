# Checkpoints

Every chapter ends with a working program. Run the listed acceptance check, then create your own tag:

```bash
git add .
git commit -m "Complete chapter NN"
git tag tutorial-NN-short-name
```

Do not tag a checkpoint with failing tests or unreviewed AI-generated changes.

| Chapter | Product evidence | Automated evidence | Suggested tag |
|---|---|---|---|
| 1 | `sai-course "list Rust files"` prints the prompt | CLI parsing unit test | `tutorial-01-minimal-cli` |
| 2 | Parsing and running are separate | `RunSummary` equality tests | `tutorial-02-application-types` |
| 3 | Empty input returns a useful error | success and error-path tests | `tutorial-03-errors` |
| 4 | A stubbed model response becomes a displayed command | HTTP parsing tests use fixtures, not the network | `tutorial-04-first-model-call` |
| 5 | YAML changes provider and allowed tools | default, file, and environment precedence tests | `tutorial-05-configuration` |
| 6 | An unconfigured command is refused | configured, unconfigured, malformed, and operator tests | `tutorial-06-validation` |
| 7 | A validated command returns its exit code | executor tests use harmless commands and temp files | `tutorial-07-execution` |
| 8 | Full flow runs with fake generator and executor | no test contacts a model or executes a real generated command | `tutorial-08-traits-and-tests` |
| 9 | Default, unsafe, and unrestricted behavior differ explicitly | table-driven safety-mode tests | `tutorial-09-safety-modes` |
| 10 | Confirmations show deterministic risk markers | adversarial scanner tests | `tutorial-10-risk-analysis` |
| 11 | History survives restart and older records parse | temp-directory round trip and compatibility tests | `tutorial-11-history` |
| 12 | Multiline input, movement, recall, and cancel work | pure editor-state transition tests | `tutorial-12-terminal-editor` |
| 13 | A frozen script matches executor output | exact-text and executable-script tests, including a glob | `tutorial-13-deterministic-commands` |
| 14 | A learner-owned feature is specified and implemented | spec validation, tests, Clippy, and review evidence | `tutorial-14-spec-driven-development` |

## Standard quality gate

Run this at every checkpoint:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

From Chapter 4 onward, also prove tests are offline: temporarily remove the model API key and run `cargo test` again. From Chapter 7 onward, review every test command and confirm it can only affect a temporary directory or print harmless output.

## Checkpoint journal

For each tag, record four short answers in your repository:

```markdown
## Chapter NN

- Human decision:
- AI contribution:
- Rust/compiler feedback:
- Evidence that the checkpoint works:
```

This journal matters more than the exact prompts. It records the reasoning that should survive after the chat session is gone.

