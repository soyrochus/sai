# AGENTS.md – SAI Code Generation Agent Specification

## Role

You are a **code generation agent** (similar to GitHub Copilot / Codex) working on the **SAI** project – a Rust CLI that turns natural language into safe shell commands, using a global config, per-call prompt configs, optional `--peek` data, and a strict safety model. :contentReference[oaicite:0]{index=0}

Your task in this iteration is to implement **two new features** in the existing codebase:

1. **Interactive duplicate-tool resolution for `--add-prompt`**  
2. **Special handling for scope `"."` that injects a directory listing into the LLM context**

These features must be **logically separated**, easily changeable, and consistent with the current architecture (modules, traits, safety model).

---

## Context (Short Summary of Current Behaviour)

- SAI has:
  - a **global config** (`config.yaml`) with `ai` + `default_prompt`.  
  - optional **per-call prompt configs** that define `meta_prompt` + `tools`.
- `sai --add-prompt <path>` merges tools from a prompt file into the global `default_prompt`.  
  **Currently, the merge aborts if any tool names already exist** (duplicate detection). :contentReference[oaicite:1]{index=1}
- When constructing the LLM request, SAI:
  - builds a **system message** from `meta_prompt`, tool list, and tool details,
  - adds a **user message** with the natural language request,
  - optionally adds a **scope message** when `-s/--scope` is supplied,
  - optionally adds a **peek message** when `--peek` is used. :contentReference[oaicite:2]{index=2}
- Safety is enforced via tool whitelisting, operator-level blocking, and confirmation gates. :contentReference[oaicite:3]{index=3}

You must **preserve all existing behaviour** unless explicitly changed below.

---

## Feature 1 – Interactive Duplicate-Tool Resolution for `--add-prompt`

### Goal

When the operator runs:

```bash
sai --add-prompt some-tools.yaml
```

and **one or more tool names already exist** in the global `default_prompt`, SAI should **no longer abort immediately**. Instead, it must interactively present the conflict and allow the user to choose for *each conflicting tool*:

* **[O] Overwrite** – overwrite the existing global tool definition with the definition from the imported file.
* **[S] Skip** – keep the existing global tool definition and ignore the imported one for this tool.
* **[C] Cancel** – abort the entire import, without applying *any* tools from this `--add-prompt` call.

### Requirements

1. **Detect duplicates as before**

   * Reuse or extend the existing duplicate detection logic in the module that implements `--add-prompt` (currently described under `ops` in the TECHSPEC). 
   * Do not change how tools are parsed from prompt YAMLs; operate on the in-memory representation (e.g., `PromptConfig.tools`).

2. **Present both definitions to the user**

   For each duplicate tool name:

   * Print a **clear header** indicating the tool name and that a conflict was found.
   * Print the **current global definition** (from the merged global config):

     * include at least: tool name, and the `config` text block.
     * label this section clearly, e.g. `Current global definition:`.
   * Print the **imported definition** (from the incoming prompt file):

     * same fields, clearly labeled, e.g. `Imported definition (from some-tools.yaml):`.
   * Make it visually obvious which is which (use headings or prefixes).

3. **Interactive choice per tool**

   * After printing both definitions for a given tool, prompt:

     ```text
     Conflict for tool '<tool-name>':

     [O] Overwrite global definition with imported definition
     [S] Skip imported definition (keep global)
     [C] Cancel entire import

     Choice [O/S/C]:
     ```

   * Accept single-letter input (`o`, `s`, `c`, case-insensitive).

   * Loop until a valid choice is entered (or EOF).

4. **Semantics of choices**

   * **Overwrite (O)**

     * Replace the **global** tool definition for this tool name with the imported definition.
     * Continue processing the next tool (including other conflicts).
   * **Skip (S)**

     * Keep the existing global definition **unchanged** for this tool name.
     * Ignore the imported definition for this tool.
     * Continue processing the next tool.
   * **Cancel (C)**

     * Immediately abort the entire `--add-prompt` operation.
     * **No tools from this import file** (including non-conflicting ones) should be persisted.
     * Global config must remain exactly as before this command started.

5. **Non-interactive / invalid input considerations**

   * If stdin is not a TTY (e.g. running in CI) and a duplicate is detected:

     * Keep the behaviour **safe and explicit**:

       * Either:

         * fail with a clear error explaining that interactive resolution is required, **or**
         * add a future flag (not now) to specify default behaviour.
       * For now, prefer failing with a clear error message.
   * If the user hits EOF or an unrecoverable I/O error while resolving, treat as **Cancel**.

6. **Isolation and testability**

   * Implement the decision logic in a **dedicated helper** (e.g. within `ops` or a new `duplicate_tools` helper module), so it can be unit-tested in isolation.
   * The helper should take:

     * existing global tools,
     * imported tools,
     * a way to read user input / write output (dependency-injected so tests can mock).
   * No changes to the LLM or execution pipeline are required for this feature.

7. **Config write semantics**

   * Only **after** all conflicts have been processed and choices applied should the global config file be written back.
   * If user chooses Cancel, global config must not be modified.

---

## Feature 2 – Special Handling for Scope `"."` (Current Directory Listing)

### Goal

When the operator supplies a **scope equal to `"."`** (dot) via `-s/--scope`, SAI should:

* generate and include a **listing of the current directory** in the LLM context, so the model knows “where it is” and what files exist there;
* do this in a bounded, configurable way, controlled by a **clearly named constant** in the source.

This should **augment** the existing “scope hint” behaviour, not replace it. 

### Requirements

1. **Trigger condition**

   * This special behaviour is only activated when the effective scope value is exactly `"."`:

     * Command line: `sai -s . "…"`
   * Other scopes (e.g. `logs/**/*.json`, `src`, `"only YAML files"`) continue to behave as currently documented (i.e. passed as a free-form scope hint string to the LLM).

2. **Directory listing construction**

   * When scope is `"."`, compute a **snapshot listing** of the current working directory.
   * At minimum, include:

     * file and directory names (one per line),
     * optionally a marker for directories (e.g. trailing `/`).
   * Do **not** recurse; this is a simple flat listing of `.` for now.

3. **Configurable limit via a constant**

   * Introduce a **public or clearly visible constant** in an appropriate module (e.g. `prompt` or a new `scope` helper), such as:

     ```rust
     pub const SCOPE_DOT_MAX_BYTES: usize = 8 * 1024;
     ```

     or

     ```rust
     pub const SCOPE_DOT_MAX_ENTRIES: usize = 256;
     ```

   * Use this constant to:

     * either limit the number of entries,
     * or limit the total size of the serialized listing (bytes/chars),
     * or both (agent may choose the most practical design, but it must be centralized via a named constant).

   * If the listing exceeds the limit:

     * truncate cleanly,
     * and add a short note like `(truncated directory listing)` at the end.

   * The goal is that operators can change this behaviour by editing a **single constant**, not multiple magic numbers.

4. **How it is injected into the LLM messages**

   * Reuse the existing concept of a **scope message** in the LLM prompt construction pipeline. 
   * When scope is `"."`:

     * generate a **more detailed scope message** that includes:

       * a brief explanation, e.g.:

         ```text
         Scope: current directory.
         Here is a non-recursive listing of the working directory:
         <listing>
         ```

     * Do not add an extra message type; it should still be treated as the “scope hint” message, just with richer content when value is `"."`.
   * When scope is something else:

     * keep current behaviour: send the scope string as-is (no directory listing).

5. **Separation & testability**

   * Encapsulate directory listing generation and truncation into a **small, single-responsibility helper function**, e.g.:

     ```rust
     fn build_scope_dot_listing() -> Result<String> { ... }
     ```

   * Unit-test this helper with:

     * empty directory,
     * several entries,
     * more entries than the limit (ensure truncation and annotation).

   * The LLM prompt builder should simply:

     * check if scope equals `"."`,
     * call the helper,
     * embed the returned string into the scope message.

6. **Safety / privacy considerations**

   * Limit listing to **names only**; do not include file sizes, timestamps, or content.
   * Do not automatically include nested paths; recursion should be a separate feature if ever needed.
   * Do not perform any I/O when no scope was provided.

---

## General Implementation Guidance

* **Language & Style:** Rust 2021, follow existing patterns in modules described in TECHSPEC (e.g., `app`, `ops`, `prompt`, `llm`, `safety`). 
* **Non-regression:** Do not change existing behaviours for:

  * `--peek` semantics,
  * safety checks,
  * `--unsafe`,
  * basic execution model,
  * default config paths.
* **Error messages:** Keep them concise and explicit; when failing due to interactive needs (e.g., duplicate tools in non-TTY), describe what the user should do.
* **Tests:**

  * Add unit tests around:

    * duplicate resolution logic,
    * scope `"."` directory listing helper.
  * If there is an integration test harness via `run_with_dependencies`, add at least one scenario for each feature.

---

## Acceptance Criteria

A merge is acceptable when:

1. `sai --add-prompt`:

   * detects duplicates,
   * interactively offers Overwrite / Skip / Cancel per tool,
   * preserves or updates global config exactly as per the user’s choices,
   * behaves safely in non-interactive contexts.

2. `sai -s . "…" `:

   * injects a directory listing into the LLM scope message,
   * respects the limits defined by a single well-named constant,
   * does not alter behaviour for other scope values.

3. All existing tests pass, and new tests cover the added behaviours.

