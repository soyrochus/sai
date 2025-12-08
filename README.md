# Sai-cli ('sai')

## Tell the shell what you want, not how to do it

<!-- markdownlint-disable MD033 -->
<p align="center">
  <img alt="MIT license badge" src="https://img.shields.io/badge/license-MIT-green.svg" />
  <img alt="Platform badge" src="https://img.shields.io/badge/platform-linux%20%7C%20macos%20%7C%20windows-blue.svg" />
  <img alt="Latest release badge" src="https://img.shields.io/github/v/release/soyrochus/sai?include_prereleases" />
  <img alt="Build status badge" src="https://github.com/soyrochus/sai/actions/workflows/build.yaml/badge.svg" />
  <img alt="Rust language badge" src="https://img.shields.io/badge/language-Rust-93450a?logo=rust&logoColor=white" />
  <img alt="OpenAI provider badge" src="https://img.shields.io/badge/AI%20Provider-OpenAI-412991?logo=openai" />
</p>
<!-- markdownlint-enable MD033 -->

**Sai-cli** ('sai') is a small, fast, Rust-based command-line tool that transforms **natural language** into **safe, real shell commands**, using an LLM — while enforcing strict guardrails to keep execution safe and predictable.

It is designed for Unix-like environments like Linux and MacOS but builds cleanly on Windows as well.

![sai-logo](images/sai-logo-smallest.png)

---

## What Sai-cli Does

Sai-cli takes two things:

1. A **prompt** describing what you want, in plain language  
2. A **configuration file** describing what tools sai-cli is allowed to use (e.g. `jq`, `grep`, `sed`, `cat`, …)

And it produces:

- A **validated, safe command** using only those tools  
- With **no shell operators**, **no pipes**, **no redirections**  
- Unless you explicitly allow it with `--unsafe`

Examples (runnable from the repo root with `prompts/standard-tools.yml`):

```bash
sai prompts/standard-tools.yml "Show where the trait CommandGenerator is defined in src"
>>  rg 'trait CommandGenerator' src

sai prompts/standard-tools.yml "List every Rust source file under src"
>> find src -type f -name '*.rs'

sai prompts/standard-tools.yml "Count lines in src/app.rs"
>> wc -l src/app.rs
```

You tell the shell **what you want**, and sai-cli figures out **how** using the tools you have whitelisted.

---

## Installation (prebuilt binaries)

Go to: [https://github.com/soyrochus/sai/releases](https://github.com/soyrochus/sai/releases)

Download the binary for your platform:

| OS      | File                                                    |
| ------- | ------------------------------------------------------- |
| Linux   | `sai-x86_64-unknown-linux-gnu`                          |
| macOS   | `sai-aarch64-apple-darwin` or `sai-x86_64-apple-darwin` |
| Windows | `sai.exe`                                               |

Make it executable and put it in your PATH:

Example: Linux

```bash
chmod +x sai
sudo mv sai /usr/local/bin/
```

That’s it.

---

## Installation (cargo install)

If you already have Rust tooling set up, install directly from crates.io:

```bash
cargo install sai-cli
```

This builds the crate `sai-cli` and drops the `sai` binary into `~/.cargo/bin` (make sure that path is on your `PATH`). Afterwards you can run:

```bash
sai --help
```

to verify the install.

---

## Configuration

Sai-cli loads its global config from the OS-standard location:

| OS      | Path                                            |
| ------- | ----------------------------------------------- |
| Linux   | `~/.config/sai/config.yaml`                     |
| macOS   | `~/Library/Application Support/sai/config.yaml` |
| Windows | `%APPDATA%/sai/config.yaml`                     |

This file contains:

1. **AI provider configuration** (OpenAI or Azure OpenAI)
2. **Default prompt/tools** for “simple mode”

You can bootstrap sensible defaults by running:

```bash
sai --init
```

This writes a starter config with placeholder API credentials and a curated set of standard Unix
tools (grep, find, awk, sed, sort, wc, etc.) pre-configured. You can immediately start using sai-cli
after updating your API key, or add more tools later with `sai --add-prompt ...` or your own YAML edits.

### Example `config.yaml`

```yaml
ai:
  provider: openai
  openai_api_key: "replace_with_your_key"
  openai_model: "gpt-5.1-mini"

default_prompt:
  meta_prompt: |
    You generate safe shell commands from natural language.
    Output exactly ONE line with the command to execute.
    Do not include markdown, explanations, or extra text.

  tools:
    - name: jq
      config: |
        Tool: jq
        Role: JSON processor.

        Rules:
        - Commands must start with "jq".
        - Do not use pipes, redirections or shell features.
        - Use jq filters to transform the JSON.
```

Environment variables always override AI configuration.

---

## Built-in Help System

Sai-cli includes a comprehensive hierarchical help system accessible directly from the command line. You can discover all features and concepts without needing to reference external documentation.

### **Getting help**

```bash
# Show top-level overview and common usage
sai help

# List all available help topics
sai help topics

# Get detailed help on a specific topic
sai help config
sai help scope
sai help explain
```

### **Available topics**

The help system covers:

- **overview** - High-level introduction to sai-cli
- **quickstart** - Minimal setup and first commands
- **config** - Global config, AI providers, defaults
- **tools** - Tool definitions and prompt configs
- **scope** - How to focus sai-cli on the right files
- **peek** - Sample data for schema inference (--peek)
- **safety** - Safety model, operator blocking, confirmation
- **unsafe** - What --unsafe relaxes and when to use it
- **explain** - Explain generated commands before running them
- **analyze** - Analyze the last sai invocation
- **history** - Where history is stored and how it is used
- **packages** - Built-in prompt configs under prompts/
- **ops** - Helper commands (--init, --add-prompt, --list-tools)
- **advanced** - Simple vs advanced mode, combining flags

Each topic provides detailed explanations, examples, and usage patterns. The help system is designed to be self-contained and progressively discoverable - start with `sai help` and explore from there.

---

## Usage

### **Simple mode**

Uses default prompt in the global config:

```bash
sai "Show all active users from users.json"
```

### **Advanced mode**

Explicit config file:

```bash
sai mytools.yaml "Find lines containing ERROR"
```

### **Peek mode** (supply sample data)

```bash
sai -p users.json "List active users"
```

This lets the LLM infer the **structure** of the data (truncated to 16 KB per file).

### **Scope hint**

Provide a path or glob so the LLM focuses on the right files:

```bash
sai -s "logs/**/*.json" "Summarize fatal errors"
```

You can use any descriptive text (e.g., "only PDF reports"), and the hint is passed as a separate message alongside the natural language prompt.

Special case: `-s .` injects a non-recursive listing of the current working directory into the LLM context (bounded by an internal size limit). This helps the model understand what files exist without you typing the names.

### **Unsafe mode**

Allows pipes, redirects, etc.
(Always forces confirmation.)

```bash
sai -u "Combine these two results and then sort"
```

### **With confirmation**

```bash
sai -c "Show me all user ids"
```

Confirmation shows:

- global config path
- prompt config path
- natural language prompt
- scope hint (if provided)
- generated command
- Y/N choice

### **Explain mode**

Get a detailed explanation of what the generated command will do before executing:

```bash
sai -e "Find all Python files modified today"
```

This mode:

- Generates the command as usual
- Asks the LLM to explain what the command does in plain language
- Shows the explanation before confirmation
- **Always requires confirmation** (implies `-c`)
- Can be combined with other flags like `--scope`, `--peek`, `--unsafe`

Example output:

```text
Generated command:
  find . -name '*.py' -mtime 0

Explanation:
  This command searches for Python files (*.py) in the current directory
  and subdirectories that were modified within the last 24 hours.
  - find . : Start search from current directory
  - -name '*.py' : Match files ending in .py
  - -mtime 0 : Modified less than 24 hours ago

Execute this command? [y/N]
```

### **Analyze mode**

Analyze the most recent sai invocation to understand what happened:

```bash
sai --analyze
```

This mode:

- Reads the last entry from sai-cli's history log
- Asks the LLM to explain what likely happened and why
- Suggests what to try next
- **Never executes any commands**
- Cannot be combined with other sai-cli parameters

Useful for:

- Understanding why a command failed
- Getting suggestions after an error
- Learning what a previous command did

Example:

```bash
$ sai "count lines in all rust files"
# ... command fails ...

$ sai --analyze
Analyzing last sai-cli invocation...

The command attempted to run 'wc -l *.rs' but failed because the shell
glob pattern wasn't expanded. The generated command needed either:
1. An explicit scope like -s . to help the LLM understand available files
2. Or a more specific prompt mentioning the directory structure

Suggested next steps:
- Try: sai -s . "count lines in all rust files in src/"
- Or: sai "count lines in src/*.rs"
```

### Create a prompt template

Generate a per-command prompt config with placeholders:

```bash
sai --create-prompt jq
```

The file defaults to `jq.yaml` in the current directory. You can specify a custom path:

```bash
sai --create-prompt jq prompts/jq-safe.yaml
```

### **Merge prompt tools into global config**

Add tools from a prompt file to your global default config:

```bash
sai --add-prompt prompts/jq-safe.yaml
```

If any tool names already exist, sai-cli shows both definitions and lets you choose per conflict:

- **O**verwrite the global definition with the imported one
- **S**kip the imported definition and keep the global one
- **C**ancel the whole import (no changes applied)

In non-interactive contexts (no TTY), duplicates cause a clear error so you can resolve interactively later.

### **List configured tools**

See which tools sai-cli will allow before running anything:

```bash
sai --list-tools
```

If you supply a prompt file, both sources are reported, and each entry notes
whether the tool is currently on your `PATH` (`[x]` present, `[ ]` missing):

```bash
sai --list-tools prompts/standard-tools.yml
```

### **Starter prompt catalog**

The repo ships with ready-to-adapt prompt configs under `prompts/`:

- [`prompts/standard-tools.yml`](prompts/standard-tools.yml) – Common Unix tools for file inspection and text processing
- [`prompts/data-focussed-tool.yml`](prompts/data-focussed-tool.yml) – Data transformation tools (jq, yq, mlr, csvkit, sed, awk)
- [`prompts/safe-destructive-tools.yml`](prompts/safe-destructive-tools.yml) – Tools that can modify files (use with caution)
- [`prompts/git-safe.yml`](prompts/git-safe.yml) – Read-only git operations (status, log, diff, show, blame, grep, etc.)
- [`prompts/git-full.yml`](prompts/git-full.yml) – Full git workflow including commits, pushes, merges, rebases (always use with --confirm)

---

## History and Analysis

Sai-cli automatically maintains a history log of all invocations in NDJSON format (newline-delimited JSON). Each command execution is recorded with metadata including:

- Timestamp and working directory
- Full command-line arguments
- Generated shell command
- Exit code and execution flags
- Optional notes about errors or special conditions

### **History log location**

| OS      | Path                                              |
| ------- | ------------------------------------------------- |
| Linux   | `~/.config/sai/history.log`                       |
| macOS   | `~/Library/Application Support/sai/history.log`   |
| Windows | `%APPDATA%\sai\history.log`                       |

The log automatically rotates when it exceeds 1 MB, keeping one backup generation.

### **Analyzing command history**

Use `--analyze` to review and understand your most recent sai-cli invocation:

```bash
sai --analyze
```

This is particularly useful after errors or unexpected results, as the LLM can explain what likely went wrong and suggest corrections.

---

## Architecture Overview

- `src/main.rs`: minimal bootstrap that calls into the real application logic.
- `src/app.rs`: orchestrates CLI parsing, configuration loading, LLM invocation, confirmation, and command execution. Exposes `run_with_dependencies` for dependency injection during tests.
- Supporting modules isolate responsibilities: `cli` (clap parser), `config` (YAML + env resolution), `prompt` (system prompt builder), `peek` (sample ingestion), `llm` (CommandGenerator trait + HTTP backend), `safety` (operator checks), `executor` (CommandExecutor trait + shell bridge), `history` (NDJSON logging and analysis), `scope` (directory context), and `ops` (init/create/add/list helpers).
- The trait boundaries (`CommandGenerator`, `CommandExecutor`) allow swapping in mocks or alternative implementations (e.g., offline generators or dry-run executors) without touching the application core.

## Development

- Format with `cargo fmt`.
- Run the unit suite with `cargo test`; it exercises filesystem helpers via `tempfile` and stays offline.
- Inspect or extend the technical deep dive in `TECHSPEC.md` for module-level rationale and expected behaviours.

## Philosophy

Sai-cli has three principles:

1. **The shell remains in control.**
   Sai-cli generates commands — it does not become a shell itself.

2. **Safety first.**
   Default mode blocks pipes, redirections, substitutions, and shell chaining.

3. **Context matters.**
   Tools behave better when they see sample data (`--peek`).

---

## Principles of Participation

Everyone is invited and welcome to contribute: open issues, propose pull requests, share ideas, or help improve documentation. Participation is open to all, regardless of background or viewpoint.

This project follows the [FOSS Pluralism Manifesto](./FOSS_PLURALISM_MANIFESTO.md), which affirms respect for people, freedom to critique ideas, and space for diverse perspectives.


## License and Copyright

Copyright (c) 2025, Iwan van der Kleijn

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
