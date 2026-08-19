## 1. CLI surface and prompt-source resolution

- [x] 1.1 Add `--interactive`, `--no-interactive`, and `--prompt-config <PATH>` flags to `Cli` in [src/cli.rs](src/cli.rs), with the two mode flags declared mutually exclusive via `conflicts_with` so clap rejects passing both.
- [x] 1.2 Relax `arg1` from `required_unless_present_any` so a bare `sai` parses, keeping every existing required-argument case that has nothing to do with prompting intact.
- [x] 1.3 Add a `PromptSource` enum (`Argument(String)`, `Editor { prefill: Option<String> }`, `PlainRead`) and a pure `resolve_prompt_source(&Cli, is_tty: bool) -> PromptSource` implementing the five-step precedence in design.md.
- [x] 1.4 Unit-test `resolve_prompt_source` against every mode-selection scenario in the `prompt-input` spec: prompt argument present, bare TTY invocation, `--interactive` with and without a prefill, `--no-interactive` in a TTY, and non-TTY.
- [x] 1.5 Add a test asserting that `--interactive` combined with `--no-interactive` fails to parse.

## 2. Editor state machine

- [x] 2.1 Create `src/editor.rs` with `EditorState` (buffer `String`, character-indexed cursor, history cursor, saved draft, search state) and an `EditorAction`/`EditorOutcome` result type covering continue, submit, and cancel.
- [x] 2.2 Implement character insertion, Backspace, and Delete against a character-indexed cursor, converting to byte offsets only at splice points; assert no panic on multi-byte input.
- [x] 2.3 Implement cursor movement: Left, Right, Home, End, with clamping at both ends of the buffer.
- [x] 2.4 Implement `Ctrl+A`, `Ctrl+E`, `Ctrl+K`, `Ctrl+U`, and `Ctrl+L` (redraw request that leaves buffer and cursor untouched).
- [x] 2.5 Implement Enter as submit, rejecting an empty-or-whitespace buffer by staying open, and Esc / `Ctrl+C` as cancel.
- [x] 2.6 Unit-test the state machine by feeding `KeyEvent` sequences directly, covering every editing and shortcut scenario in the `prompt-input` spec including mid-buffer edits, Backspace at position 0, and non-ASCII text.

## 3. Terminal driver and rendering

- [x] 3.1 Implement an RAII raw-mode guard whose `Drop` disables raw mode and restores cursor visibility, so panics and early returns both leave the terminal clean.
- [x] 3.2 Implement the driver loop: acquire the guard, read `crossterm` key events, feed them to `EditorState`, redraw, and return the `EditorOutcome`.
- [x] 3.3 Implement prompt-area rendering — indicator plus buffer — positioning the visible cursor by display width rather than character count, and handle `Ctrl+L` by clearing and redrawing.
- [x] 3.4 Fall back to `PlainRead` with a stderr note when `enable_raw_mode()` fails, rather than aborting the run.
- [x] 3.5 Implement the `PlainRead` path: read one line from the injected reader, and return an explicit "no prompt provided" error on immediate EOF.

## 4. Wire into the run flow

- [x] 4.1 In `run_with_reader` ([src/app.rs](src/app.rs)), insert prompt acquisition ahead of `build_system_prompt`, replacing the direct `cli.prompt` / `arg1` read with the resolved `PromptSource`.
- [x] 4.2 Resolve the per-call prompt config from `--prompt-config` when present, otherwise from the existing positional rule, leaving `sai foo.yaml` and `sai foo.yaml "text"` meaning exactly what they mean today.
- [x] 4.3 Map `EditorOutcome::Cancelled` to a `RunSummary` with `exit_code: 0` and `notes: Some("cancelled")`, matching the existing declined-confirmation path.
- [x] 4.4 Add tests confirming that an editor-composed prompt and the identical argument-supplied prompt drive the same generation, safety, confirmation, and execution path and yield equivalent execution-history entries.
- [x] 4.5 Add a test confirming that `--unsafe` and `--explain` behave identically for an editor-composed prompt.
- [x] 4.6 Verify the piped-input case (`echo "..." | sai`) never attempts raw mode.

## 5. Prompt history storage

- [x] 5.1 Create `src/prompt_history.rs` with a `{ts, prompt}` entry type, JSONL append to `config_root_dir().join("prompt_history.log")`, and a byte cap with `.bak` rotation mirroring [src/history.rs](src/history.rs).
- [x] 5.2 Create the store with user-only permissions where the platform supports it.
- [x] 5.3 Implement loading into a newest-first `Vec<String>`, skipping malformed lines and returning an empty history when the file is absent.
- [x] 5.4 Suppress an append when the prompt is identical to the most recent recorded entry, while still recording non-consecutive repeats.
- [x] 5.5 Record the prompt at submission time, before generation, so prompts that produce an LLM error remain recallable; record argument-supplied prompts on the same path.
- [x] 5.6 Downgrade write failures to a stderr warning so generation and execution continue.
- [x] 5.7 Unit-test storage against the `prompt-history` spec using the existing `set_config_dir_override_for_tests` helper: persistence, absent store, rotation preserving newest entries, corrupt-line skipping, duplicate collapsing, and cancellation recording nothing.

## 6. History recall in the editor

- [x] 6.1 Load prompt history into `EditorState` at editor startup.
- [x] 6.2 Implement Up/Down navigation, newest-first, loading the selected prompt with the cursor at the end, clamping at the oldest entry, and restoring the saved draft when moving forward past the newest.
- [x] 6.3 Implement `Ctrl+R` reverse incremental search: query accumulation, most-recent match as the query grows, repeated `Ctrl+R` stepping to the next older match, and a no-match indication that leaves the query editable.
- [x] 6.4 Implement accepting a match into the buffer as editable text, and cancelling search by restoring the exact pre-search buffer.
- [x] 6.5 Render the search-status line while search mode is active.
- [x] 6.6 Unit-test every navigation and search scenario in the `prompt-history` spec, including empty history, draft restoration, match cycling, and editing a recalled prompt before submit.

## 7. Cross-platform verification and documentation

- [x] 7.1 Run `cargo test` and `cargo clippy` clean on macOS/Linux.
- [ ] 7.2 (macOS verified via pty: rendering, cursor columns, Ctrl+C/Esc cancel, terminal restore, reverse search. **Linux and Windows still need a human.**) Manually verify the editor on macOS Terminal, a Linux terminal, and Windows Terminal plus PowerShell — key handling, redraw, cursor alignment, and terminal restoration on both submit and cancel — since the editor path cannot be exercised headlessly.
- [x] 7.3 Decide the `prompt_history.log` byte cap (design.md Open Questions) and whether wide-character rendering needs `unicode-width`; record the outcome in the module.
- [x] 7.4 Update [src/help.rs](src/help.rs) with `--interactive` and `--no-interactive`, and describe the default editor behavior in the usage text.
- [x] 7.5 Update [README.md](README.md) with the editor, its key bindings, and the prompt-history file location and how to clear it.
- [x] 7.6 Add the v1.2.0 entry to [CHANGELOG.md](CHANGELOG.md), noting that a bare `sai` no longer produces clap's required-argument error.
