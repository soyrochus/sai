# Divergences from the Reference Application

This tutorial builds a small, working version of SAI. It does not build SAI. This
document says precisely where and how the two part ways, chapter by chapter, so a
learner who opens `src/` after finishing a chapter knows what to expect: which
differences are just scale, and which are different decisions.

None of this is a defect in the course. `proposal.md` states the non-goals up
front — no async, no GUI, no exhaustive shell parsing, no full Windows script
emission — and `README.md` calls `src/` "the reference destination, not code to
copy at the start." This file makes that boundary concrete instead of general.

## Scale, in one table

| | Tutorial (through Chapter 13) | `src/` (reference) |
|---|---:|---:|
| `cli` | one required `String` field | 414 lines, 14+ flags, two prompt-source modes |
| `app` | ~30–80 lines, grows through Ch. 2–8 | 2,478 lines |
| `editor` | ~150 lines of pure state | 1,884 lines |
| `config` | one flat struct + one enum | 493 lines, provider-tagged enum, thread-local test seam |
| `llm` | one function, one JSON shape | 518 lines, two traits, two providers, two API modes |
| whole application | low hundreds of lines | 8,914 lines across 16 modules |

Six production modules — roughly 1,700 lines — have no tutorial chapter at all.
They're listed at the end of this document.

## Chapter-by-chapter

### Chapters 1–3 — `cli.rs`, `app.rs` (early state)

The tutorial's `Cli` has one field: `request: String`, required. Production's
`Cli` (`src/cli.rs`) has no field named `request` at all. Instead it has `arg1:
Option<String>` and `prompt: Option<String>`, resolved through a `PromptSource`
enum (`resolve_prompt_source`) that decides between *simple mode* (one
positional natural-language argument), *advanced mode* (a prompt-config file
plus a separate prompt argument), and *interactive mode* (no argument at all —
falls into the Chapter 12 editor). The tutorial's "one input, one output"
starting contract is real and pedagogically correct, but it is not a simplified
version of the production `Cli` — production never had a single required
positional string to begin with.

`RunSummary` also diverges immediately in shape, not just later in fate (see
Chapter 8 below). The tutorial's version has 3 fields: `request`,
`generated_command`, `exit_code`. Production's `RunSummary` (`src/app.rs`) has
10: `exit_code`, `generated_command`, `unsafe_mode`, `unrestricted`, `confirm`,
`explain`, `scope`, `peek_files`, `notes`, `prompt` — one field for nearly every
CLI flag production supports and the tutorial doesn't build.

### Chapter 4 — `llm.rs`

The tutorial's `generate_command(client, endpoint, api_key, model, request) ->
Result<String>` sends exactly the user's raw text as `input`, with a
two-sentence hardcoded system instruction.

Production's `CommandGenerator` trait is:

```rust
fn generate(
    &self,
    ai: &EffectiveAiConfig,
    system_prompt: &str,
    nl_prompt: &str,
    scope_hint: Option<&str>,
    peek_text: Option<&str>,
) -> Result<String>;
```

Four differences that matter, not just added parameters:

- `ai` is a provider-tagged `EffectiveAiConfig` enum (OpenAI vs. Azure, see
  Chapter 5), not a flat endpoint/key/model triple.
- `system_prompt` is built separately by `src/prompt.rs` from the tool
  allowlist and its descriptions — the tutorial's system instruction is
  inlined as a string literal and never grows.
- `scope_hint` and `peek_text` come from `src/scope.rs` and `src/peek.rs`,
  neither of which any chapter builds (see "Never built," below).
- Production also defines a second trait, `ChatClient`, used only by
  `--explain`. The tutorial never implements the explain feature, so it never
  needs this trait. `HttpCommandGenerator` in production implements both
  traits on one type; `run_with_dependencies` requires `G: CommandGenerator +
  ChatClient` — a detail with no tutorial counterpart.

### Chapter 5 — `config.rs`

The tutorial's `EffectiveAiConfig` is a flat struct: `endpoint`, `model`,
`api_key`, `allowed_tools`. Production's is a two-variant *enum*:

```rust
pub enum EffectiveAiConfig {
    OpenAI { api_key, base_url, api_mode: OpenAiApiMode, model, model_snapshot, reasoning_effort },
    Azure  { api_key, endpoint, deployment, api_version },
}
```

This is a design difference, not a size difference: the tutorial's resolver
picks fields; production's resolver picks a *shape*, and `OpenAiApiMode`
(`Responses` vs. `ChatCompletions`) is itself a further branch inside the
OpenAI variant. The stretch exercise in Chapter 5 ("support a provider enum")
gestures at this but stops well short of it.

Production also resolves the config directory itself
(`config_root_dir`/`find_global_config_path`) rather than accepting an
`Option<&Path>` argument, and uses a `thread_local!` override
(`CONFIG_ROOT_OVERRIDE`) as its test seam instead of the tutorial's explicit
parameter. The tutorial's approach is more testable by construction; production
trades that for not needing a path threaded through every caller.

### Chapter 6 — `validation.rs`

**This file does not exist in `src/`.** The tutorial has learners create a
standalone `src/validation.rs` with `validate()` and `reject_shell_operators()`.
In production, the equivalent logic — `validate_and_split_command` and
`detect_forbidden_operator` — lives in `src/safety.rs`, in the same file as
the Chapter 10 risk scanner. A learner who goes looking for `validation.rs` in
the real repository won't find it by that name; the module boundary the
tutorial teaches (validation as its own concern) doesn't survive into
production, where validation and risk analysis share a file.

### Chapter 7 — `executor.rs`

Two real divergences here, not one:

1. **Glob expansion is core in production, optional in the tutorial.** Chapter
   7's stretch exercise says "add explicit glob expansion with the `glob`
   crate... only after validation." Production's `expand_glob_if_needed`
   (using the same `glob` crate) is not optional — it's the first thing
   `ShellCommandExecutor` does to every non-shell argument.
2. **The trait signature drops the typed command entirely.** The tutorial's
   `CommandExecutor::execute(&self, command: &ValidatedCommand) -> Result<i32>`
   takes the validated domain type built in Chapter 6. Production's is
   `execute(&self, cmd_line: &str, tokens: &[String], unsafe_mode: bool) ->
   Result<i32>` — two raw strings and a bool, no `ValidatedCommand`-shaped type
   at the boundary at all. **The tutorial's design is stricter than
   production's here**, not a simplification of it — Chapter 6's "make invalid
   states harder to construct" type doesn't have a production counterpart to
   grow into.

### Chapter 8 — traits, `app.rs`

Beyond the generator/executor signature gaps already covered, production
layers dependency injection one level deeper than the tutorial does. Where the
tutorial's `run<G, E>(request, allowed, generator, executor) -> Result<i32>` is
one function, production has three: `run()` (assembles real dependencies),
`run_with_dependencies<G, E>` (injects generator/executor, reads stdin
directly), and `run_with_reader<G, E, R>` (also injects the `BufRead` used for
confirmation prompts). There's a fourth layer,
`run_with_reader_and_confirmation_output`, that injects where confirmation text
is written. The tutorial never needs to inject stdin or stdout because it never
builds a confirmation flow that reads from the terminal — that arrives
conceptually in Chapter 9 but isn't wired to real I/O the way production's is.

### Chapter 9 — `safety_mode.rs`

**This is the closest match in the whole course.** Both are three-variant
enums (`Default`, `Unsafe`, `Unrestricted`) with capability-query methods
rather than booleans, and production's module doc even uses the same "ladder,
not independent switches" framing the tutorial teaches. The difference is
depth: production has 9 methods (`as_str`, `parse`, `from_cli`,
`allows_operators`, `lifts_tool_restriction`, `forces_inspection`,
`uses_shell`, `is_unrestricted`, `is_unsafe_for_history`) against the
tutorial's 3 (`uses_shell`, `enforces_allowlist`,
`requires_typed_confirmation`). The extra methods exist because production's
enum also has to answer to history recording (`is_unsafe_for_history`) and CLI
parsing (`from_cli`, `parse`) — concerns the tutorial's smaller surface doesn't
carry yet.

### Chapter 10 — risk analysis (`safety.rs` in production, not `risk.rs`)

The tutorial builds a **more granular, more precise** scanner than production
ships. Tutorial `RiskKind` has 6 variants (`Pipeline`, `Redirect`,
`CommandSubstitution`, `Separator`, `DestructiveFlag`, `BroadWildcard`) and
`RiskMarker<'a>` carries a `byte_offset: usize` plus a `token: &'a str`
borrowed directly from the scanned text — introduced specifically so the
chapter has a real, motivated reason to teach named lifetimes on a struct, one
of the gaps identified relative to the Book. Production's `RiskKind` has **3**
variants (`Operator`, `Destructive`, `WildcardBreadth` — pipes, redirects,
chaining, and substitution all collapse into one `Operator` case) and
`RiskMarker` carries only `{ kind, detail: String }`, an owned copy with **no
position information at all**. The tutorial's design lets a caller point at
exactly where in the source a risk was found, without duplicating the matched
text as a separate literal; production's only lets a caller say what kind of
risk it was, in prose. This is a case where finishing the chapter produces a
strictly more capable type than the one
in `src/` — don't expect production code to explain or improve on the
tutorial's offsets, because there's nothing to compare them against.

### Chapter 11 — `history.rs`

Tutorial `HistoryEntry` has 6 fields (`timestamp`, `cwd`, `exit_code`,
`generated_command`, `prompt`, `unrestricted`). Production's has 12: add
`ts` (renamed from `timestamp`), `argv`, `unsafe_mode`, `confirm`, `explain`,
`scope`, `peek_files`, `notes` — again, nearly one field per CLI flag the
tutorial doesn't build.

More structurally: the tutorial's `append(path: &Path, entry)` and
`latest(path: &Path)` take the log path as an explicit argument specifically
so tests can point at a temp directory. Production's `write_entry(entry)` and
`read_latest_entry()` take **no path argument** — they resolve
`history_log_path()` internally via `config::config_root_dir()`, the same
global/thread-local mechanism noted under Chapter 5. Production also falls
back to a backup file (`backup_path`) when the primary log's last record is
unreadable; the tutorial chapter mentions this exists in production but does
not build it, even as a stretch exercise.

### Chapter 12 — `editor.rs`

The tutorial's `EditorState` has 5 fields and one public method beyond
construction (`handle`). Production's exposes `buffer`, `cursor`,
`search_query`, `show_help`, `hint`, `is_searching`, `search_match`,
`search_failed` — meaning **incremental reverse search and a help overlay are
load-bearing features in production**, not optional ones. Chapter 12's stretch
exercise proposes reverse search ("Ctrl+R") as an *optional* extension; in
`src/editor.rs` it's already there, unconditionally, and accounts for a large
share of why that file is 1,884 lines against the tutorial's ~150.

Production also splits `EditorAction` (returned by the low-level `apply(&mut
self, key: KeyEvent)`) from a separate `EditorOutcome` type returned by the
top-level `compose()` function — a second layer of state the tutorial's
`EditorAction` alone doesn't need because it never builds the outer
driver loop with the same generality.

### Chapter 13 — `commands.rs`

Close in shape, with one policy difference worth knowing before you read the
real file: the tutorial's `write_atomic(dir, command, replace_confirmed: bool)`
bundles the overwrite refusal *inside* the write function. Production's
`write(command, config)` takes no such flag — it always writes and always
renames over an existing file. The refusal check
(`if target.exists() { ... }`) happens one layer up, in `src/app.rs`, before
`commands::write` is ever called. Production separates the policy decision
(should we overwrite?) from the mechanism (atomic write); the tutorial
teaches the mechanism with the policy folded in, which is easier to follow in
isolation but is not how the reference application actually draws the line.

Production's `list()` and `format_listing()` (for `--list-commands`) exist and
correspond to Chapter 13's stretch exercise, but that exercise is explicitly
optional — a learner who skips it will not have built anything resembling
those two functions.

### Chapter 14 — specification-driven development

This chapter is the closest to a 1:1 correspondence in the whole course,
because it teaches a *process*, not a code artifact. The archived change it
points learners at —
[`openspec/changes/archive/2026-08-20-add-deterministic-commands`](../openspec/changes/archive/2026-08-20-add-deterministic-commands/) —
is the real proposal, spec, design, and task list that produced the actual
`src/commands.rs` frozen-command feature. Reading it after Chapter 13 is the
one place in the course where "the reference destination" and "what you just
did" are describing the same historical event.

## Never built: production modules with no tutorial chapter

Roughly 1,700 lines across six files that no chapter, stretch exercise, or
capstone example touches:

| Module | Lines | What it does |
|---|---:|---|
| `help.rs` | 445 | The structured `sai help <topic>` system: a `HelpTopic` enum, per-topic rendered text, and the CLI's usage/about strings. The tutorial's own chapters are the substitute. |
| `ops.rs` | 569 | `--init`, `--create-prompt`, `--add-prompt` (with a `DuplicateResolverIo` trait and `MergeResult` for interactively resolving tool-list conflicts), `--list-tools`, and `program_on_path`. The entire per-call prompt-config-file system lives here. |
| `prompt.rs` | 142 | `build_system_prompt()` — the real construction of what gets sent to the model as system instructions, built from the tool allowlist and descriptions. Chapter 4's system instruction is a hardcoded string that never grows toward this. |
| `prompt_history.rs` | 357 | A *second*, separate history stream from `history.rs`: raw submitted prompt text (not outcomes), used to populate the editor's up/down recall in Chapter 12. The tutorial passes a literal `Vec<String>` into `EditorState::new` and never builds what actually fills it. |
| `scope.rs` | 104 | `--scope`: builds a short directory-listing hint appended to the prompt so the model has some notion of the working directory's contents. |
| `peek.rs` | 59 | `--peek <file>`: reads up to `PEEK_MAX_BYTES` from named files and folds them into the prompt as context. |

## How to use this file

Read the relevant section after finishing a chapter, before you go looking at
`src/` for "the real version." Where this document says the difference is
scale (more fields, more flags, more callers), treat the reference file as
what your chapter's code grows into. Where it says the difference is a design
decision — the `validation.rs`/`safety.rs` split, the typed `ValidatedCommand`
executor boundary, the risk scanner's byte offsets, the write/confirm split in
Chapter 13 — treat production as *one* valid choice among the ones the chapter
itself taught you to compare, not as a correction of the chapter.
