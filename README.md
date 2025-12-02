<p align="center">
  <img src="https://img.shields.io/badge/license-MIT-green.svg" />
  <img src="https://img.shields.io/badge/platform-linux%20%7C%20macos%20%7C%20windows-blue.svg" />
  <img src="https://img.shields.io/github/v/release/your-org/sai" />
  <img src="https://github.com/your-org/sai/actions/workflows/build.yml/badge.svg" />
</p>

# **SAI**
### **Tell the shell what you want, not how to do it.**

**SAI** is a small, fast, Rust-based command-line tool that transforms **natural language** into **safe, real shell commands**, using an LLM — while enforcing strict guardrails to keep execution safe and predictable.

It is designed for Unix-like environments but builds cleanly for macOS and Windows as well.

---

## ✨ **What SAI Does**

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

## 🛠️ Installation (prebuilt binaries)

Go to:

### **➡ [https://github.com/your-org/sai/releases](https://github.com/your-org/sai/releases)**

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

## 📁 Configuration

SAI loads its global config from the OS-standard location:

| OS      | Path                                            |
| ------- | ----------------------------------------------- |
| Linux   | `~/.config/sai/config.yaml`                     |
| macOS   | `~/Library/Application Support/sai/config.yaml` |
| Windows | `%APPDATA%/sai/config.yaml`                     |

This file contains:

1. **AI provider configuration** (OpenAI or Azure OpenAI)
2. **Default prompt/tools** for “simple mode”

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

## 🧭 Usage

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

* global config path
* prompt config path
* natural language prompt
* generated command
* Y/N choice

---

## 💡 Philosophy

SAI has three principles:

1. **The shell remains in control.**
   SAI generates commands — it does not become a shell itself.

2. **Safety first.**
   Default mode blocks pipes, redirections, substitutions, and shell chaining.

3. **Context matters.**
   Tools behave better when they see sample data (`--peek`).

---

## 📝 License

MIT License.
See `LICENSE` for details.

---

## 🤝 Contributing

PRs welcome.
Please keep implementations small, auditable, and safe by default.

---