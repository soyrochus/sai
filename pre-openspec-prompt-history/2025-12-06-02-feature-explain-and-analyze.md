# SAI Command Agent – Implement `--analyze` and `--explain`

## Role

You are a **Rust code generation agent** working on the SAI CLI project.

Your task is to add:

1. A **history log** (append-only NDJSON) usable by analysis tools.
2. A new CLI mode `--analyze` that explains the **most recent SAI run**.
3. A new flag `--explain` that explains **what a generated command will do** and forces confirmation before executing it.

You must write idiomatic Rust 2021, keep the design modular, and preserve existing behaviour unless explicitly changed. See the file TECHSPEC.md for the technical reference and README.md for the funcional description of the application. 

---

## Part 1 – History Log Design

### 1.1 Log format

Implement a **simple, human-readable history file** using **newline-delimited JSON (NDJSON)**. Each line is a complete JSON object representing one SAI run.

Define a struct similar to:

```rust
#[derive(Serialize, Deserialize)]
pub struct HistoryEntry {
    pub ts: String,              // ISO 8601 UTC timestamp
    pub cwd: String,             // current working directory
    pub argv: Vec<String>,       // full CLI argv as seen by SAI
    pub exit_code: i32,          // process exit code (or synthetic code for internal errors)
    pub generated_command: Option<String>, // final shell command, if any
    pub unsafe_mode: bool,       // whether --unsafe was used
    pub confirm: bool,           // whether confirmation was requested
    pub explain: bool,           // whether --explain was used
    pub scope: Option<String>,   // raw scope value, if any
    pub peek_files: Vec<String>, // list of peek paths, if any
    pub notes: Option<String>,   // optional free-form note (e.g. error summary)
}
````

Rules:

* **One entry per line**, UTF-8 text.
* No multi-line fields; if you need to summarise an error, keep it short.
* Use `chrono` or existing time utilities (or `time` crate already in the project) to emit an **ISO 8601 UTC** timestamp.
* Nulls are fine; keep the format stable and backwards compatible.

### 1.2 Log file location

Choose a path under the **same base directory** as SAI’s config (

* Linux: `~/.local/share/sai/history.log` or `~/.config/sai/history.log`
* macOS: `~/Library/Application Support/sai/history.log`
* Windows: `%APPDATA%\sai\history.log`

Expose a helper function, e.g.:

```rust
fn history_log_path() -> PathBuf
```

so the path is computed in exactly one place.

### 1.3 Writing history entries

* After each SAI invocation finishes (success or failure), write exactly one `HistoryEntry` to the log.

* Append mode only:

  ```rust
  OpenOptions::new()
      .create(true)
      .append(true)
      .open(history_log_path())?;
  ```

* Serialize the entry with `serde_json::to_string(&entry)?`, write it plus `\n`.

* Flush the writer; no need for `fsync` in this project.

* **Do not** delete or rewrite the file when adding new entries.

### 1.4 Bounded growth (rotation)

Implement a **simple size-based rotation**:

* Define a constant:

  ```rust
  pub const HISTORY_MAX_BYTES: u64 = 1_000_000; // e.g. 1 MB
  ```

* After appending an entry, check the file size.

* If size exceeds `HISTORY_MAX_BYTES`:

  * Atomically rename `history.log` to `history.log.1`,
  * Start a new `history.log` on next write.

* Do not chain multiple generations (no `.2`, `.3`, etc.); one backup is enough.

* Rotation logic should live in a small, well-named helper, e.g. `rotate_history_if_needed`.

### 1.5 Reading the latest entry

Provide a function, in a dedicated module (e.g. `history`):

```rust
pub fn read_latest_entry() -> Result<Option<HistoryEntry>>
```

Semantics:

* If the log does not exist or is empty → return `Ok(None)`.
* Otherwise, read the file, iterate over lines, parse valid `HistoryEntry` records, and return the **last successfully parsed entry**.
* If a line is malformed, skip it **with a debug log**, but do not fail the entire reading operation.

You do **not** need to optimize further; the history is bounded by rotation.

---

## Part 2 – `--analyze` Mode

### 2.1 CLI behaviour

Add a new flag:

* Long: `--analyze`
* No short form required.
* When `--analyze` is present:

  * **No normal SAI parameters are allowed**:

    * No natural language prompt,
    * No config prompt file,
    * No scope, no peek, no unsafe, etc.
  * The CLI should enforce this as a **mutually exclusive mode**:

    * Example: in the `clap` definition, make `analyze` conflict with `prompt`, `arg1`, `--peek`, `--scope`, etc.

If `--analyze` is given together with any incompatible option, show a clear error and exit with a non-zero code, e.g.:

```text
The --analyze option cannot be combined with normal SAI parameters.
Run `sai --analyze` alone to analyze the latest command.
```

### 2.2 Functional behaviour

When `--analyze` is used:

1. **Read latest history entry**

   * Call `history::read_latest_entry()`.
   * If there is no history, print a friendly message and exit with a code like `2`:

     ```text
     No history available to analyze yet.
     ```

2. **Build an analysis prompt**

   * Construct a system prompt describing the task, e.g.:

     > “You are a debugging assistant for the SAI CLI. You receive structured information about the last SAI invocation (command line, generated shell command, exit code, etc.). Explain in concise technical terms what likely happened and why, and suggest what the user might try next. If information is missing, state the limitations.”

   * Provide the `HistoryEntry` serialized as JSON (or a nicely formatted text) in the user content.

   Example of user content:

   ```text
   Here is the last SAI invocation as a JSON object:

   {
     "ts": "...",
     "cwd": "...",
     "argv": [...],
     "exit_code": ...,
     "generated_command": "...",
     "unsafe_mode": false,
     "confirm": true,
     "explain": false,
     "scope": "...",
     "peek_files": [...],
     "notes": "..."
   }

   Please explain what likely happened and why.
   ```

3. **Call the LLM**

   * Use the same LLM wiring as for normal command generation (OpenAI / Azure), but:

     * Keep `temperature = 0.0`.
     * Do **not** generate any shell commands here.
   * Print the LLM’s explanation to stdout.

4. **No command execution**

   * `--analyze` must **never execute any shell commands**.
   * It does not go through the normal “generate command → validate → run” pipeline.
   * It only reads history and calls the LLM in “explain” mode.

5. **History for analyze runs**

   * Optionally log an entry for `--analyze` itself with:

     * `generated_command = None`,
     * `notes = Some("analyze mode")`.
   * This is not strictly required; implement if it fits existing patterns.

---

## Part 3 – `--explain` Mode

### 3.1 CLI behaviour

Add a new flag:

* Long: `--explain`
* Short: `--e`

* `--explain` is a **modifier** for normal command generation:

  * It is **incompatible** with `--analyze`.
  * It may be combined with other options: `--scope`, `--peek`, `--unsafe`, etc.

Important rule:

> When `--explain` is present, it **implies `--confirm`**.

Implementation:

* In the parsed CLI struct, add `explain: bool`.
* Compute `let need_confirm = cli.confirm || cli.unsafe_mode || cli.explain;`.

### 3.2 Functional behaviour

With `--explain`:

1. **Generate the command as usual**

   * Run the normal SAI pipeline:

     * Build system prompt and messages.
     * Call LLM to obtain the command line.
     * Validate with `validate_and_split_command`, including safety checks (or skipping them in `--unsafe`).
   * Do **not** execute the command yet.

2. **Explain the generated command**

   Create a second LLM call (or reuse the same client) with:

   * System prompt: an “explainer” role, e.g.:

     > “You are a shell and tool usage explainer. Given a shell command, explain in concise technical language what it will do, describing each flag and argument, and the overall effect. Do not invent behaviour not implied by the command.”

   * User content:

     ```text
     Explain this command in detail, but concisely:

     <COMMAND LINE HERE>
     ```

   * Temperature: `0.0`.

   Print the explanation to stderr or stdout before asking for confirmation. Suggested layout:

   ```text
   Generated command:
     <command>

   Explanation:
     <multi-line explanation from LLM>

   ```

3. **Force confirmation**

   After printing the explanation:

   * Show the same confirmation question as the existing `--confirm` flow, e.g.:

     ```text
     Execute this command? [y/N]
     ```

   * If user answers “yes” → execute the command as usual.

   * If user answers anything else → do **not** execute; exit with code `0` or a dedicated “cancelled” code.

4. **Interaction with `--unsafe`**

   * If `--unsafe` is also set:

     * Operator checks are disabled (as already implemented).
     * Confirmation is still **required** because of the `explain` flag.
   * The explanation step should still work normally and may explicitly mention risky constructs if visible (pipes, redirects, etc.).

5. **History integration**

   * The `HistoryEntry` for this run should include:

     * `generated_command` = final shell command string,
     * `explain` = true,
     * `confirm` = true,
     * `unsafe_mode` as appropriate,
     * `exit_code` based on actual execution or 0 if user cancelled.

---

## Part 4 – Integration & Structure

### 4.1 Module boundaries

* Put history logic into a dedicated module, e.g. `history.rs`:

  * `pub struct HistoryEntry`
  * `pub fn write_entry(...)`
  * `pub fn read_latest_entry() -> Result<Option<HistoryEntry>>`
  * `fn rotate_history_if_needed(...)`

* Extend the main CLI module to add:

  * `analyze: bool`,
  * `explain: bool`.

* Integrate:

  * At program start, after parsing args:

    * If `analyze` → go to analyze flow and return.
  * At program end, after executing (or cancelling) the command:

    * Call `history::write_entry(...)`.

### 4.2 Error handling

* `--analyze` with no history:

  * print a clear message, exit non-zero.
* LLM failure in `--analyze`:

  * print an error and exit non-zero; do not execute anything.
* LLM failure in `--explain`:

  * print an error; still show the raw command,
  * ask for confirmation anyway (user may still choose to run),
  * do not crash.

### 4.3 Tests

Add unit or integration tests to cover:

* `history::write_entry` + `read_latest_entry` simple round trip.
* Rotation triggers when size threshold exceeded.
* `--analyze`:

  * returns friendly message when no history,
  * reads latest entry and does **not** attempt to generate commands.
* `--explain`:

  * forces confirmation even if `--confirm` was not explicitly passed,
  * does not execute command when user answers “no”.

---

## Deliverables

* Updated CLI parsing with `--analyze` and `--explain`.
* New `history` module implementing NDJSON logging and latest-entry retrieval.
* Implemented flows:

  * `sai --analyze`
  * Normal SAI run with optional `--explain`.
* Tests for history and new modes.
* No regressions in existing SAI behaviour.
