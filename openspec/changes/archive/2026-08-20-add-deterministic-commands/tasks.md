## 1. Record the intent in the invocation history

- [x] 1.1 Add `prompt: Option<String>` to `HistoryEntry` in [src/history.rs](src/history.rs) with `#[serde(default)]`, mirroring how `unrestricted` was added, including a comment explaining why it is defaulted.
- [x] 1.2 Populate it from the resolved natural-language prompt in `run_with_reader` ([src/app.rs](src/app.rs)), on the same path for argument-supplied and editor-composed prompts alike.
- [x] 1.3 Test that an entry written without the field still parses and reports no intent, and that a round trip preserves a multi-line prompt.
- [x] 1.4 Confirm `--analyze` now sees the prompt for an editor-composed invocation, where it previously saw only `argv`.

## 2. Commands directory and configuration

- [x] 2.1 Add an optional `commands.dir` key to the global config in [src/config.rs](src/config.rs), defaulting to a `bin` directory under `config_root_dir()`.
- [x] 2.2 Add a resolver returning the effective commands directory, honouring the test override already used by every storage test.
- [x] 2.3 Add `--commands-path` to [src/cli.rs](src/cli.rs), printing the directory for use in a `PATH` assignment.
- [x] 2.4 Print the `export PATH="$(sai --commands-path):$PATH"` line from `--init` in [src/ops.rs](src/ops.rs).
- [x] 2.5 Assert in a test that no code path reads or writes a shell startup file.
- [x] 2.6 Create the directory on first freeze, not at startup, so an installation that never freezes anything is untouched.

## 3. Script emission

- [x] 3.1 Create `src/commands.rs` with a `FrozenCommand` type holding name, command text, intent, freeze time, safety mode, tools, prompt config, and risk markers.
- [x] 3.2 Implement header rendering as `# sai:<field>` comment lines, and a parser that reads them back — the two must round-trip, and a test should assert that directly.
- [x] 3.3 Implement the body quoting rule: verbatim `cmd_line` for shell-executed modes; per-token `shell_words::quote` for default mode, **except** tokens containing `*`, `?` or `[`, which are emitted bare.
- [x] 3.4 Test the glob exception explicitly against a `src/*` argument, with a comment naming the executor behavior at [src/executor.rs:8](src/executor.rs#L8) that it mirrors, so a future simplification cannot quietly break it.
- [x] 3.5 Test that a default-mode argument containing a space, a quote, or a non-glob shell metacharacter is quoted and cannot be split or reinterpreted.
- [x] 3.6 Emit the `#!/usr/bin/env bash` shebang and `set -euo pipefail` preamble, and do not forward `"$@"`.
- [x] 3.7 Emit the `read -rp` risk guard when markers are present, and omit it entirely when they are not.
- [x] 3.8 Write atomically: temporary file in the target directory, set the executable bit, then rename into place.
- [x] 3.9 Put emission behind `#[cfg(unix)]`, with the non-Unix path returning the "not yet supported on this platform" refusal.

## 4. The freeze path

- [x] 4.1 Add `--save <name>` and `--save <name> "<prompt>"` to [src/cli.rs](src/cli.rs), with unit tests for the argument shapes and for the fact that a lone positional elsewhere is still prompt text.
- [x] 4.2 Implement freezing a command generated in the same invocation, reusing the resolved command, tokens, safety mode and markers already in scope.
- [x] 4.3 Implement freezing the most recent command from `history.log`, building the card from the recorded entry — command, safety mode, scope, intent — and failing clearly when there is nothing to freeze.
- [x] 4.4 Present the existing `PreflightCard` and require confirmation before writing; do not reuse the unrestricted typed-`yes` rule, since freezing writes a file rather than executing anything.
- [x] 4.5 Mark the intent unavailable in the header when the source history entry carries no prompt, rather than omitting the field.
- [x] 4.6 Test that declining the confirmation writes nothing.

## 5. Freeze-time refusals

- [x] 5.1 Refuse a name that already resolves on `PATH`, naming the conflicting program, reusing the `PATH` walk in [src/ops.rs:352](src/ops.rs#L352) — extract it to a shared helper rather than duplicating it.
- [x] 5.2 Require explicit confirmation before replacing an existing frozen command, and leave the existing file byte-identical when declined.
- [x] 5.3 Refuse to freeze a command recorded as generated under unrestricted mode when `safety.allow_unrestricted: false`, naming the configuration file responsible.
- [x] 5.4 Order every refusal before the temporary file is created, and test that a refused freeze leaves the directory unchanged.
- [x] 5.5 Test each refusal separately, including that freezing a default-mode command is unaffected by `allow_unrestricted: false`.

## 6. Listing

- [x] 6.1 Implement `--list-commands` as a directory scan that parses each script's header, consulting no index.
- [x] 6.2 Mark commands frozen under unrestricted mode.
- [x] 6.3 Flag a command whose recorded tools are no longer on `PATH`, using the shared helper from 5.1.
- [x] 6.4 Skip files that are not SAI-emitted scripts, or whose headers do not parse, without breaking the rest of the listing.
- [x] 6.5 Report an empty or absent directory as "no frozen commands" rather than failing.
- [x] 6.6 Test that a hand-edited intent is reflected in the listing, proving the file is the source of truth.

## 7. End-to-end verification

- [x] 7.1 Test the full round trip against a temporary config directory: generate, freeze, read the emitted file, and assert its exact text for both a default-mode and an unsafe-mode command.
- [x] 7.2 Actually execute an emitted script in a test and assert its output matches what SAI's own executor produced for the same command — the only check that proves the quoting rule preserves semantics rather than merely looking plausible.
- [x] 7.3 Include a glob-bearing default-mode command in that comparison.
- [x] 7.4 Confirm ordinary generation, safety validation, confirmation and execution are unchanged by the presence of frozen commands, and that `sai run the tests` is still a prompt.
- [x] 7.5 Run `cargo test` and `cargo clippy` clean.
- [ ] 7.6 Manually verify on macOS: freeze a command, add the directory to `PATH`, run it by name, confirm the risk guard prompts for a destructive command, and confirm `--list-commands` flags a tool removed from `PATH`.

## 8. Documentation

- [x] 8.1 Add a `commands` help topic in [src/help.rs](src/help.rs) and `templates/help/commands.txt`, and list it in the topics index.
- [x] 8.2 Update [README.md](README.md) with the freeze workflow, the `PATH` step, a sample emitted script, and the fact that a frozen command runs without SAI.
- [x] 8.3 Document in README that `safety.allow_unrestricted: false` gates freezing rather than execution, since that is a narrower guarantee than it reads.
- [x] 8.4 Add a section to [docs/TECHSPEC.md](docs/TECHSPEC.md) covering the artifact format, the quoting rule and why it has a glob exception, and the freeze-time-only enforcement model.
- [x] 8.5 Note the new `prompt` field in the TECHSPEC `HistoryEntry` listing.
- [x] 8.6 Add a CHANGELOG entry, calling out the narrowed `allow_unrestricted` guarantee and that Windows support is deferred.
