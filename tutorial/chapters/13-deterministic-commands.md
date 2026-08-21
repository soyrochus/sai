# Chapter 13 — Turn AI Output into a Deterministic Artifact

A useful command should not require another model call every time it runs. Freezing moves AI from runtime to authoring time: generate once, review once, then execute a stable script.

## Product goal

Save a reviewed command as an executable Unix script containing its provenance, written atomically and preserving the execution semantics under which it was reviewed.

## Rust concepts

You will work with rendering and parsing, `Cow<str>`, filesystem permissions, same-directory atomic rename, conditional compilation, cleanup on failure, and executable integration tests.

## Build

Define the artifact:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrozenCommand {
    pub name: String,
    pub command: String,
    pub tokens: Vec<String>,
    pub intent: String,
    pub frozen_at: String,
    pub mode: SafetyMode,
    pub risk_markers: Vec<String>,
}
```

Render metadata as JSON string values inside comments. JSON escaping preserves newlines and quotes while keeping the script readable:

```rust
impl FrozenCommand {
    pub fn render(&self) -> String {
        let mut out = String::from("#!/usr/bin/env bash\n");
        for (key, value) in [
            ("intent", self.intent.clone()),
            ("frozen-at", self.frozen_at.clone()),
            ("safety", format!("{:?}", self.mode).to_lowercase()),
            ("risk-markers", self.risk_markers.join(" | ")),
            ("command", self.command.clone()),
        ] {
            out.push_str(&format!(
                "# sai:{key}={}\n",
                serde_json::to_string(&value).expect("String serialization cannot fail")
            ));
        }
        out.push_str("set -euo pipefail\n");

        if !self.risk_markers.is_empty() {
            out.push_str(
                "read -rp 'This command was marked risky. Continue? [y/N] ' answer\n\
                 [[ \"$answer\" == y || \"$answer\" == yes ]] || exit 1\n"
            );
        }

        if self.mode.uses_shell() {
            out.push_str(&self.command);
        } else {
            let body = self.tokens.iter().map(|token| {
                if token.contains(['*', '?', '[']) {
                    token.clone()
                } else {
                    shell_words::quote(token).into_owned()
                }
            }).collect::<Vec<_>>().join(" ");
            out.push_str(&body);
        }
        out.push('\n');
        out
    }
}
```

Why the glob exception? Your default executor expanded glob-bearing arguments itself. Quoting every token in the script would change that observed behavior. All non-glob tokens must be quoted so the script does not acquire shell splitting or operator behavior that default mode never had.

Validate the name as one filename: reject empty names, `.`, `..`, `/`, and `\\`. Check for collisions before writing. Require confirmation before creating any temporary file.

Write atomically on Unix:

```rust
#[cfg(unix)]
pub fn write_atomic(
    dir: &Path,
    command: &FrozenCommand,
    replace_confirmed: bool,
) -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;

    validate_name(&command.name)?;
    std::fs::create_dir_all(dir)?;
    let target = dir.join(&command.name);
    if target.exists() && !replace_confirmed {
        anyhow::bail!("refusing to replace {} without confirmation", target.display());
    }
    let temp = dir.join(format!(".{}.{}.tmp", command.name, std::process::id()));

    let result: Result<()> = (|| {
        std::fs::write(&temp, command.render())?;
        std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o700))?;
        std::fs::rename(&temp, &target)?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(error);
    }
    Ok(target)
}

#[cfg(not(unix))]
pub fn write_atomic(
    _dir: &Path,
    _command: &FrozenCommand,
    _replace_confirmed: bool,
) -> Result<PathBuf> {
    anyhow::bail!("frozen commands are not yet supported on this platform")
}
```

The full reference implementation, including header parsing and listing, is [`src/commands.rs`](../../src/commands.rs).

## AI collaboration script

Start with semantics, not rendering code:

> Specify how a frozen Bash script must preserve default-mode direct-execution behavior and shell-mode behavior. Cover spaces, quotes, operators, globs, risk confirmation, metadata, permissions, overwrite refusal, and failure cleanup. Identify which behavior needs an actual execution test.

Then request implementation in small pieces:

> Implement header render/parse round trips first. Next implement body quoting with tests. Finally add an atomic Unix writer with all refusals before the first write.

Ask the AI to review the order of effects. A plausible implementation that creates a temp file before confirmation has already violated the product contract.

## Compiler conversation

Conditional compilation makes platform support explicit. Only Unix compiles the permissions import and executable-bit logic. Other platforms compile a real function that returns a clear error, rather than failing later with a missing method.

`shell_words::quote` returns `Cow<'_, str>`: it can borrow an already-safe token or allocate an escaped one. `into_owned()` normalizes both cases because the rendered vector must own its strings.

Atomic rename requires the temporary file to be in the target directory. Across filesystems, rename may not be atomic and may fail entirely. The target directory also gives the temp file the same filesystem and usual permission context.

There is a small cleanup race if multiple processes freeze the same name using only a process ID. A production implementation can add random uniqueness and explicit replacement policy.

## Tests

The important test executes the artifact instead of only admiring its text:

```rust
#[cfg(all(test, unix))]
#[test]
fn frozen_default_command_matches_direct_execution() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("one.rs"), "").unwrap();
    std::fs::write(dir.path().join("two.rs"), "").unwrap();

    let frozen = FrozenCommand {
        name: "show-rust".into(),
        command: "printf '%s\\n' '*.rs'".into(),
        tokens: vec!["printf".into(), "%s\\n".into(), "*.rs".into()],
        intent: "show Rust files".into(),
        frozen_at: "2026-08-20T12:00:00Z".into(),
        mode: SafetyMode::Default,
        risk_markers: vec![],
    };
    let path = write_atomic(dir.path(), &frozen, false).unwrap();

    let output = std::process::Command::new(&path)
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("one.rs"));
    assert!(stdout.contains("two.rs"));
}
```

Also require:

- header render/parse round trip, including multiline intent;
- ordinary arguments with spaces, quotes, `$`, and `;` remain one inert argument;
- shell-mode body is emitted verbatim;
- risk guard appears only when markers exist;
- declining save confirmation creates no directory or temp file;
- rename failure cleans up the temp file;
- the resulting Unix mode contains the owner executable bit.

## Review checklist

- Confirmation and every refusal occur before filesystem mutation.
- Metadata round-trips through escaped headers.
- Default and shell modes have distinct rendering rules.
- Glob behavior is verified by executing the script.
- Writing is temp-file, permissions, then same-directory rename.
- Unsupported platforms return an explicit error.
- The frozen script does not call SAI or a model.

## Checkpoint

```bash
git add src
git commit -m "tutorial: freeze reviewed commands"
git tag tutorial-13-deterministic-commands
```

Evidence: rendering tests pass, the emitted file is executable, and an end-to-end test proves its output matches the application’s executor for a glob-bearing command.

## Stretch exercise

Implement listing by scanning and parsing script headers, with no registry file. Skip unrelated or malformed files and mark commands whose recorded tools are no longer on `PATH`.

## Reflection

- How does freezing change the model from a runtime dependency into an authoring dependency?
- Why is exact-text comparison insufficient to prove equivalent execution semantics?
- Which guarantees end when a user edits the generated script?

## Further learning

- [The Rust Book — Using `Box<T>` to Point to Data on the Heap](https://doc.rust-lang.org/book/ch15-01-box.html) — the smart-pointer family `Cow` belongs to, though `Cow` itself isn't covered in the Book, Comprehensive Rust, or Rust by Example.
- [Rust by Example — `cfg`](https://doc.rust-lang.org/rust-by-example/attribute/cfg.html) — the attribute behind every `#[cfg(unix)]` in this chapter.

Next: [Specification-driven development with AI](14-spec-driven-development.md).
