# SAI Help System Agent – Implement a Hierarchical, Self-Explanatory Interface

## Role

You are a **Rust code generation agent** working on the `sai-cli` project (“sai”).


Your task is to design and implement a **complete, hierarchical help system** that makes SAI self-explanatory from the command line.

You write **Rust code** and tests; this prompt defines behaviour, structure, and expectations.

---

## High-Level Goal

Create a **first-class help system** such that a new user can discover and understand all major SAI concepts purely via:

- `sai --help`
- `sai help`
- `sai help <topic>`

without needing to open the README.

Help must be:

- **Hierarchical** (top-level + topics),
- **Consistent** (terminology, structure),
- **Aligned with README** (not contradictory),
- **Non-magical** (plain text, no AI in help).

---

## Existing Concepts to Cover

From the current README and design, the help system must cover at least:

1. **Overview / philosophy**
2. **Basic usage: simple vs advanced mode**
3. **Configuration**
   - Global config `~/.config/sai/config.yaml` (and OS variants),
   - AI provider configuration (OpenAI / Azure),
   - Environment variable overrides.
4. **Prompt configs / tools**
   - Default `default_prompt` from global config,
   - Per-call prompt YAML,
   - Tool definitions and safety rules.
5. **Scope**
   - `-s/--scope` as a hint,
   - Special case `-s .` (directory listing).
6. **Peek**
   - `--peek` sample data (truncation, schema inference).
7. **Unsafe / confirm / dry-run**
   - `-u/--unsafe` (operator checks disabled, always confirm),
   - `-c/--confirm`,
   - Implicit confirm when `--unsafe` or `--explain` is used.
8. **Explain**
   - `-e/--explain` (explain generated command, then confirm).
9. **Analyze**
   - `--analyze` (reads latest history entry, asks LLM to explain what happened).
10. **History**
    - NDJSON log, location, rotation, purpose.
11. **Prompt packages & ops**
    - `--init`,
    - `--add-prompt`,
    - `--list-tools`,
    - (and any other existing ops: `--create-prompt`, etc.).
12. **Safety model**
    - Tool whitelisting,
    - Operator-level blocking,
    - Explicit unsafe, confirmation.
13. **Standard packages under `prompts/`** :contentReference[oaicite:2]{index=2}
14. **Installation & path sanity**
15. **Principles / ethos (briefly)**

---

## Required User-Facing Behaviour

### 1. `sai --help` / `sai -h`

- Show **high-level usage** and top-level flags.
- Include a **summary of the most common commands** and a pointer to `sai help topics`.
- Make it concise but informative.

Example (shape, not exact text):

```text
sai-cli – Tell the shell what you want, not how to do it.

Usage:
  sai [FLAGS] [PROMPT_CONFIG] "<natural language prompt>"

Common flags:
  -s, --scope <SCOPE>     Provide a path or hint to restrict context
  -p, --peek <FILE>...    Send sample file(s) for schema inference
  -c, --confirm           Ask before executing the generated command
  -u, --unsafe            Allow pipes and redirects (always implies confirm)
  -e, --explain           Explain the generated command, then ask to confirm
      --analyze           Explain the last sai invocation, do not run anything
      --init              Create a starter config.yaml
      --add-prompt PATH   Merge tools from a prompt file into the global config
      --list-tools [PATH] List tools from global config and optional prompt file

Run:
  sai help topics    to list help topics
  sai help <topic>   for detailed help on <topic>
```

**Do not hard-code text in clap only**; centralize help strings where possible so they can be reused for `sai help`.

### 2. `sai help` with no arguments

* Show **top-level help** including:

  * What SAI is,
  * Core idea (“Tell the shell what you want, not how to do it”),
  * A couple of **short examples**,
  * A list of **help topics** (like a manpage “SEE ALSO”).

Example topics (exact set may be slightly adjusted):

* `overview`
* `quickstart`
* `config`
* `tools`
* `prompts`
* `scope`
* `peek`
* `safety`
* `unsafe`
* `confirm`
* `explain`
* `analyze`
* `history`
* `packages`
* `ops` (for init/add/list)
* `advanced`

You do **not** need a topic per flag if it’s redundant; group logically.

Example output sketch:

```text
SAI – Tell the shell what you want, not how to do it.

SAI turns natural language into safe shell commands using a configurable set
of tools and an AI backend. You whitelist tools, SAI generates commands.

Common usage:
  sai "List all Rust files under src"
  sai prompts/standard-tools.yml "Find lines containing ERROR in logs"

Help topics:
  overview    High-level introduction to SAI
  quickstart  Minimal setup and first commands
  config      Global config, AI providers, defaults
  tools       Tool definitions and prompt configs
  scope       How to focus SAI on the right files (-s/--scope)
  peek        Sample data for schema inference (--peek)
  safety      Safety model, operator blocking, confirmation
  explain     Explain mode for generated commands
  analyze     Analyzing the last SAI run
  history     Where history is stored and how it's used
  packages    Shipped prompt packages under prompts/
  ops         Helper commands like --init, --add-prompt, --list-tools

Run:
  sai help <topic>
```

### 3. `sai help <topic>`

Implement hierarchical, topic-based help:

* If `topic` matches a known topic (case-insensitive): print the long-form help text for that topic and exit 0.
* If `topic` is unknown: report an error and show `sai help topics`.

Examples:

```bash
sai help overview
sai help quickstart
sai help config
sai help scope
sai help peek
sai help explain
sai help analyze
sai help packages
```

`help topics` must be a valid topic itself, listing all topics.

---

## Content Requirements per Topic

The content should be **plain-text, stable, and hand-crafted** (no AI at runtime). It must align with the README. 

Below is guidance for each key topic. You should implement these as structured strings (or a small DSL) in Rust.

### 3.1 `overview`

Explain:

* What SAI is (1–2 paragraphs).
* The core idea: natural language → safe command.
* The three main principles (shell in control, safety first, context matters). 
* That SAI never becomes a shell; it just generates commands.

### 3.2 `quickstart`

Explain:

* Requirements: AI key (OpenAI/Azure), Rust or binary download.
* Minimal steps:

  * Run `sai --init`.
  * Edit `config.yaml` with your API key and model.
  * Add tools (either manually or with `--add-prompt`).
  * Run a first command or two.
* Show exactly **one or two** copy-pasteable commands.

### 3.3 `config`

Cover:

* Location of config files per platform (same as README). 
* Structure of `config.yaml`:

  * `ai` section (provider, API key, model, etc.),
  * `default_prompt` section.
* How environment variables override `ai` fields.
* Mention that the default prompt config is used whenever no per-call YAML is given.

### 3.4 `tools` / `prompts`

Explain:

* What a “tool” is in SAI (logical name + `config` text used in the LLM system prompt).
* How tools are defined in `default_prompt.tools` and per-call YAML. 
* How to:

  * create a new prompt file (`--create-prompt` if present),
  * merge tools (`--add-prompt`),
  * list tools (`--list-tools`).
* Safety: using tools to whitelist capabilities.

### 3.5 `scope`

Explain:

* `-s/--scope` as a free-form hint string or path/glob.
* How it narrows the model’s attention (“only consider these files / logs”).
* Special behaviour of `-s .`:

  * SAI injects a **non-recursive directory listing** of the current directory into the LLM context,
  * bounded by an internal limit (mention conceptually, no need for constant value).
* Emphasize: `scope` affects prompt, not actual shell restrictions.

### 3.6 `peek`

Explain:

* `-p/--peek <FILE>...` sends **truncated samples** to the LLM to help it infer data structure.
* Each file limited to N bytes (mention conceptually).
* This is meant for schema inference, not full data processing.
* Privacy considerations: only use `--peek` on data you’re comfortable sending.

### 3.7 `safety`

Explain:

* Tool whitelisting and operator-level blocking:

  * default: no pipes, redirects, `&&`, `||`, etc.,
  * `--unsafe` disables operator blocking but always implies confirmation.
* `--confirm` and `--unsafe` interplay.
* That SAI executes commands directly (no shell if you use `Command::new` etc.; keep details high-level).
* That `--explain` + `--analyze` are non-destructive.

### 3.8 `unsafe`

If you create a separate topic:

* Explain what `--unsafe` does:

  * allows pipes / redirects / substitution,
  * keeps whitelist of tools but relaxes operator safety,
  * always asks for confirmation.
* Recommend using it sparingly.

### 3.9 `explain`

Explain:

* What `-e/--explain` does:

  * generates command as usual,
  * calls LLM to explain the command,
  * prints explanation,
  * forces confirmation before execution.
* Show a short example of output (like in README). 

### 3.10 `analyze`

Explain:

* What `--analyze` does:

  * reads latest history entry,
  * calls LLM to explain what likely happened,
  * NO commands are executed.
* When to use it:

  * after an error or surprising behaviour.
* Mention that it cannot be combined with other parameters.

### 3.11 `history`

Explain:

* That SAI writes an NDJSON history log:

  * location per OS, as in README,
  * fields: timestamp, cwd, argv, generated command, exit code, etc. (conceptual).
* Rotation policy (size-based, one backup).
* That `--analyze` uses this log.

### 3.12 `packages`

Explain:

* The existence of shipped YAML prompt configs in `prompts/` 

  * `standard-tools.yml`,
  * `data-focussed-tools.yml`,
  * `safe-destructive-tools.yml`,
  * etc.
* How to use them:

  * `sai prompts/standard-tools.yml "..."`.
* How to extend them.

### 3.13 `ops`

Explain helper commands, such as:

* `--init`
* `--add-prompt PATH`
* `--list-tools [PATH]`
* `--create-prompt` (if present)

Include examples.

---

## Implementation Requirements (Rust)

### 4.1 CLI integration

* Add a `help` *subcommand-like* mode:

  * `sai help`
  * `sai help <topic>`
* Integrate with the existing `clap` CLI parser:

  * Use a **subcommand** or a **"pseudo subcommand"** (e.g. `arg1="help"` branch) — pick what best fits the existing CLI design, but keep it robust.

### 4.2 Internal representation of help topics

Define an internal type, e.g.:

```rust
enum HelpTopic {
    Overview,
    Quickstart,
    Config,
    Tools,
    Scope,
    Peek,
    Safety,
    Unsafe,
    Explain,
    Analyze,
    History,
    Packages,
    Ops,
    Topics,   // for 'sai help topics'
}
```

Plus mapping from string (user input) to variant:

```rust
impl HelpTopic {
    fn from_str(s: &str) -> Option<Self> { ... }
}
```

Focus on:

* Case-insensitive matching,
* Accepting synonyms if useful (`sai help getting-started` → `Quickstart`).

### 4.3 Rendering help text

For each `HelpTopic`, implement a method that returns a **static &str** or `Cow<'static, str>`:

```rust
impl HelpTopic {
    fn render(&self) -> &'static str {
        match self {
            HelpTopic::Overview => OVERVIEW_HELP,
            HelpTopic::Quickstart => QUICKSTART_HELP,
            // ...
        }
    }
}
```

Define the help texts as constants:

```rust
const OVERVIEW_HELP: &str = r#"
SAI – Tell the shell what you want, not how to do it.

[...]
"#;
```

Make sure the texts:

* Are line-wrapped reasonably (80-ish columns),
* Use plain ASCII (avoid fancy quotes),
* Are independent of current config or environment variables.

### 4.4 `help topics` special case

* If user runs `sai help topics`, print a list of topics and one-line descriptions.
* This can be derived from a static list, e.g. an array of `(topic_name, one_line)`.

Example:

```text
Available help topics:

  overview     High-level introduction to SAI
  quickstart   Minimal setup and first commands
  config       Global config, AI providers, defaults
  tools        Tool definitions and prompt configs
  scope        How to focus SAI on the right files
  peek         Sample data for schema inference
  safety       Safety model, operator blocking, confirmation
  explain      Explain generated commands before running them
  analyze      Analyze the last SAI invocation
  history      Where history is stored and why
  packages     Built-in prompt configs under prompts/
  ops          Helper commands (--init, --add-prompt, --list-tools)
```

### 4.5 Unknown topic / errors

If user runs `sai help something-unknown`:

* Print a clear message:

  ```text
  Unknown help topic 'something-unknown'.

  Run 'sai help topics' to see all available topics.
  ```

* Exit with non-zero status.

### 4.6 No LLM dependency

**Important:** The help system must be **pure**, no LLM calls. It must work offline and instantaneously.

---

## Testing

Add tests to cover:

1. `HelpTopic::from_str` mapping:

   * Known topics,
   * Case-insensitivity,
   * `topics` alias.
2. `sai help` prints a header and lists topics.
3. `sai help topics` prints at least one line per topic.
4. Unknown topic returns error and suggests `sai help topics`.

If you have an integration test harness, consider adding a simple smoke test that runs the binary with `help` topics and checks that:

* It exits with code 0,
* Produces non-empty output.

---

## Acceptance Criteria

* `sai --help` provides a clear, concise top-level description and links to `sai help topics`.
* `sai help` and `sai help topics` work and list all available topics.
* `sai help <topic>` renders detailed help for that topic with consistent structure and terminology aligned with README. 
* No network / LLM calls are used for help.
* Help system is fully usable offline.
* Unknown topics produce a clear, friendly error and guidance.
* Tests exist for topic mapping and basic help commands.

Focus your code changes on:

* CLI parsing integration,
* Help topic enum and text rendering,
* Small tests verifying mapping and output.

Do **not** refactor unrelated parts of the SAI codebase unless strictly necessary to integrate help.

