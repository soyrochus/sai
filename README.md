# SAI

## Tell the shell what you want, not how to do it

<!-- markdownlint-disable MD033 -->
<p align="center">
  <img alt="MIT license badge" src="https://img.shields.io/badge/license-MIT-green.svg" />
  <img alt="Platform badge" src="https://img.shields.io/badge/platform-linux%20%7C%20macos%20%7C%20windows-blue.svg" />
  <img alt="Latest release badge" src="https://img.shields.io/github/v/release/your-org/sai" />
  <img alt="Build status badge" src="https://github.com/soyrochus/sai/actions/workflows/build.yml/badge.svg" />
  <img alt="Rust language badge" src="https://img.shields.io/badge/language-Rust-93450a?logo=rust&logoColor=white" />
  <img alt="OpenAI provider badge" src="https://img.shields.io/badge/AI%20Provider-OpenAI-412991?logo=openai" />
</p>
<!-- markdownlint-enable MD033 -->

**SAI** is a small, fast, Rust-based command-line tool that transforms **natural language** into **safe, real shell commands**, using an LLM — while enforcing strict guardrails to keep execution safe and predictable.

It is designed for Unix-like environments but builds cleanly for macOS and Windows as well.

![sai-logo](images/sai-logo-smallest.png)

---

## What SAI Does

SAI takes two things:

1. A **prompt** describing what you want, in plain language  
2. A **configuration file** describing what tools SAI is allowed to use (e.g. `jq`, `grep`, `sed`, `cat`, …)

And it produces:

- A **validated, safe command** using only those tools  
- With **no shell operators**, **no pipes**, **no redirections**  
- Unless you explicitly allow it with `--unsafe`

Example:

```bash
sai "List all active users showing id and email in users.json"
````

SAI reads your default config, calls the LLM with the tool instructions and (optionally) a data sample, then produces something like:

```bash
>> jq '.users[] | select(.active) | {id, email}' users.json
```

You tell the shell **what you want**, and SAI figures out **how**.

---

## Installation (prebuilt binaries)

Go to:

### **➡ [https://github.com/soyrochus/sai/releases](https://github.com/your-org/sai/releases)**

Download the binary for your platform:

| OS      | File                                                    |
| ------- | ------------------------------------------------------- |
| Linux   | `sai-x86_64-unknown-linux-gnu`                          |
| macOS   | `sai-aarch64-apple-darwin` or `sai-x86_64-apple-darwin` |
| Windows | `sai.exe`                                               |

Make it executable and put it in your PATH:

```bash
chmod +x sai
sudo mv sai /usr/local/bin/
```

That’s it.

---

## Configuration

SAI loads its global config from the OS-standard location:

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

This writes a starter config with placeholder API credentials and no tools. Add tooling later with
`sai --add-prompt ...` or your own YAML edits.

### Example `config.yaml`

```yaml
ai:
  provider: openai
  openai_api_key: "$OPENAI_API_KEY"
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

The command fails gracefully if any tool names already exist in the global config.

### **List configured tools**

See which tools SAI will allow before running anything:

```bash
sai --list-tools
```

If you supply a prompt file, both sources are reported:

```bash
sai --list-tools prompts/standard-tools.yml
```

### **Starter prompt catalog**

The repo ships with ready-to-adapt prompt configs under `prompts/`:

- [`prompts/standard-tools.yml`](prompts/standard-tools.yml)
- [`prompts/data-focussed-tool.yml`](prompts/data-focussed-tool.yml)
- [`prompts/safe-destructive-tools.yml`](prompts/safe-destructive-tools.yml)

---

## Philosophy

SAI has three principles:

1. **The shell remains in control.**
   SAI generates commands — it does not become a shell itself.

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

