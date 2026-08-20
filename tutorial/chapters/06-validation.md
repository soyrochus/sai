# Chapter 6 — Treat Model Output as Untrusted Input

The model can produce a plausible command, but plausible is not the same as permitted. This chapter creates a deterministic validation boundary before execution.

## Product goal

Parse the candidate into arguments, require the first program to be allowlisted, and reject shell control syntax in the default mode.

## Rust concepts

This boundary uses a validated domain type, `HashSet` lookup through borrowing, fallible parsing, iterator search, and exact string matching to make invalid states harder to construct.

## Build

Add:

```toml
[dependencies]
shell-words = "1"
```

Create `src/validation.rs`:

```rust
use anyhow::{Result, bail};
use std::collections::HashSet;

#[derive(Debug, PartialEq, Eq)]
pub struct ValidatedCommand {
    pub source: String,
    pub tokens: Vec<String>,
}

pub fn validate(source: &str, allowed: &HashSet<String>) -> Result<ValidatedCommand> {
    if source.trim().is_empty() {
        bail!("model returned an empty command");
    }

    reject_shell_operators(source)?;
    let tokens = shell_words::split(source)
        .map_err(|error| anyhow::anyhow!("invalid command quoting: {error}"))?;
    let Some(program) = tokens.first() else {
        bail!("command has no program");
    };

    if !allowed.contains(program) {
        bail!("tool {program:?} is not allowed");
    }

    Ok(ValidatedCommand {
        source: source.to_owned(),
        tokens,
    })
}

fn reject_shell_operators(source: &str) -> Result<()> {
    let forbidden = ["&&", "||", ";", "|", "`", "$(", ">", "<", "\n", "\r"];
    if let Some(operator) = forbidden.into_iter().find(|op| source.contains(op)) {
        bail!("shell operator {operator:?} is not allowed in default mode");
    }
    Ok(())
}
```

Build the exact program-name set from Chapter 5's effective configuration:

```rust
let allowed = effective_config.allowed_tools.into_iter().collect();
let command = validation::validate(&generated, &allowed)?;
```

This scanner is intentionally conservative, but it is not yet quote-aware. It will reject `rg 'a|b'`, even though the pipe is literal text inside a quoted argument. That false positive is acceptable for this checkpoint and becomes an explicit design problem in Chapter 10.

The key invariant is:

> Default mode never asks a shell to interpret model output.

## AI collaboration script

Ask:

> Write a pure Rust validator for an AI-generated command. It must reject empty text, malformed quoting, shell operators, and tools outside an exact allowlist. Return a type that can only be constructed after validation. Generate adversarial table tests before implementation.

Then challenge the draft:

> Find bypasses involving whitespace, quoted operators, newlines, command substitution, absolute paths, and a program name that merely starts with an allowed name.

Do not accept “looks safe” as a conclusion. Turn every suspected bypass into a test, then decide whether the intended policy should accept or reject it.

## Compiler conversation

`ValidatedCommand` is a small example of making invalid states harder to represent. Its fields are public here for convenience; making them private and exposing accessor methods would prevent other modules from constructing unchecked values.

The allowlist uses `HashSet<String>`, while `program` is `&String`. `HashSet::contains` accepts a borrowed lookup, so validation does not allocate another string.

The `find` call returns `Option<&str>`. Pattern matching with `if let Some(operator)` handles the failure case without indexing or panicking.

You may be tempted to accept a partial match:

```rust
allowed.iter().any(|tool| program.starts_with(tool))
```

Do not. If `git` is allowed, that would also accept `git-malware`. Exact matching is both simpler and safer.

## Tests

Table tests make the policy visible:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn allowed() -> HashSet<String> {
        ["find", "rg"].into_iter().map(str::to_owned).collect()
    }

    #[test]
    fn accepts_allowed_program_and_preserves_quoted_argument() {
        let command = validate("find . -name '*.rs'", &allowed()).unwrap();
        assert_eq!(command.tokens, ["find", ".", "-name", "*.rs"]);
    }

    #[test]
    fn rejects_unknown_or_prefixed_tools() {
        for source in ["rm file", "find-extra ."] {
            assert!(validate(source, &allowed()).is_err(), "{source}");
        }
    }

    #[test]
    fn rejects_shell_control_syntax() {
        for source in [
            "find . | head",
            "find . && rm file",
            "find .; rm file",
            "find $(pwd)",
            "find .\nrm file",
        ] {
            assert!(validate(source, &allowed()).is_err(), "{source}");
        }
    }

    #[test]
    fn rejects_unclosed_quotes() {
        assert!(validate("find 'unfinished", &allowed()).is_err());
    }
}
```

Also add a regression test whenever a human reviewer or AI assistant finds a new bypass. Security behavior should become more stable with every review.

## Review checklist

- Model output is validated before any execution API sees it.
- Tool matching is exact.
- Empty input and malformed quoting are errors.
- Shell syntax is rejected in default mode.
- Policy examples are represented as table tests.
- Known false positives are documented instead of hidden.

## Checkpoint

```bash
git add Cargo.toml Cargo.lock src
git commit -m "tutorial: validate generated commands"
git tag tutorial-06-validation
```

Evidence: all adversarial tests pass, including unknown tools, command substitution, pipelines, separators, and malformed quotes.

## Stretch exercise

Make `ValidatedCommand` fields private. Add `program()` and `args()` accessors, and ensure the only public constructor runs validation. Try to create an invalid value from `main.rs` and let the compiler demonstrate the boundary.

## Reflection

- What can types guarantee here, and what still requires runtime checks?
- Which false positives are acceptable for a safety-first default?
- Why is a deterministic validator preferable to asking the model whether its own output is safe?
