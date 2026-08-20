# Chapter 11 — Keep History Compatible

Once commands affect a real machine, users need a record of what was requested, generated, and executed. An append-only format keeps the first implementation understandable.

## Product goal

Write one JSON object per invocation, read the latest valid entry, tolerate corrupt lines, and continue reading records created before new fields existed.

## Rust concepts

You will combine append-only file I/O, buffered reading, Serde defaults, optional fields, path borrowing, corruption recovery, and compatibility fixtures.

## Build

Add `chrono` and ensure Serde JSON is present:

```toml
chrono = { version = "0.4", features = ["serde"] }
serde_json = "1"
```

Create `src/history.rs`:

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub timestamp: String,
    pub cwd: String,
    pub exit_code: i32,
    pub generated_command: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub unrestricted: bool,
}

pub fn append(path: &Path, entry: &HistoryEntry) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open history {}", path.display()))?;
    serde_json::to_writer(&mut file, entry)?;
    writeln!(file)?;
    file.flush()?;
    Ok(())
}

pub fn latest(path: &Path) -> Result<Option<HistoryEntry>> {
    if !path.exists() {
        return Ok(None);
    }
    let file = std::fs::File::open(path)?;
    let mut last_good = None;
    for line in BufReader::new(file).lines() {
        let Ok(line) = line else { continue };
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str(&line) {
            last_good = Some(entry);
        }
    }
    Ok(last_good)
}
```

Construct the entry in orchestration after every completed attempt. Decide explicitly whether rejected commands are recorded and which sensitive prompt content should be omitted or redacted.

Newline-delimited JSON is useful because one damaged record does not make the rest of the file structurally unreadable. It is not a database: concurrent writers, durability, indexing, and unbounded growth need additional design.

The reference implementation adds rotation and backup fallback in [`src/history.rs`](../../src/history.rs).

## AI collaboration script

Ask:

> Design an append-only NDJSON history for this CLI. Pass the path into functions so tests use temporary directories. New optional fields must deserialize from old records with defaults. The reader should skip malformed lines and return the latest valid entry. Include compatibility, corruption, and multiline-prompt tests.

Then ask for a privacy review:

> Classify each proposed history field as operationally necessary, useful but sensitive, secret, or unnecessary. Recommend retention and redaction behavior. Do not assume local disk means private.

AI can help enumerate concerns, but product owners must decide what the application retains.

## Compiler conversation

`append` borrows both the path and entry because it only needs them for serialization. `serde_json::to_writer` accepts `&mut file`; the mutable borrow ends before `writeln!` and `flush` use the file again.

`#[serde(default)]` says that a missing field should receive its type’s default. For `Option<String>` that is `None`; for `bool` it is `false`. Add this attribute when a newly introduced field has a safe interpretation for old data.

Do not put `#[serde(default)]` on a field whose absence cannot be interpreted safely. In that case, version the record or write a custom migration.

The `let Ok(line) = line else` branch discards unreadable input. A production program should report a warning with the line number while still continuing.

## Tests

Use `tempfile`, never the user’s real history:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn entry() -> HistoryEntry {
        HistoryEntry {
            timestamp: "2026-08-20T12:00:00Z".into(),
            cwd: "/tmp".into(),
            exit_code: 0,
            generated_command: Some("printf '%s\\n' hello".into()),
            prompt: Some("say\nhello".into()),
            unrestricted: false,
        }
    }

    #[test]
    fn round_trip_preserves_multiline_prompt() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.log");
        append(&path, &entry()).unwrap();
        assert_eq!(latest(&path).unwrap(), Some(entry()));
    }

    #[test]
    fn old_record_receives_new_field_defaults() {
        let old = r#"{"timestamp":"t","cwd":".","exit_code":0,"generated_command":null}"#;
        let parsed: HistoryEntry = serde_json::from_str(old).unwrap();
        assert_eq!(parsed.prompt, None);
        assert!(!parsed.unrestricted);
    }

    #[test]
    fn corrupt_trailing_record_does_not_hide_last_good_record() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.log");
        append(&path, &entry()).unwrap();
        std::fs::write(
            &path,
            format!("{}\nnot-json\n", serde_json::to_string(&entry()).unwrap()),
        ).unwrap();
        assert_eq!(latest(&path).unwrap(), Some(entry()));
    }
}
```

Add a small rotation threshold in a test-only helper rather than writing a megabyte merely to cross the production limit.

## Review checklist

- History functions accept an explicit path.
- New compatible fields use intentional Serde defaults.
- Corrupt lines do not hide earlier valid records.
- Tests never touch real user data.
- Retained prompts and commands have a documented privacy policy.
- Growth and concurrent writers are acknowledged constraints.

## Checkpoint

```bash
git add Cargo.toml Cargo.lock src
git commit -m "tutorial: add compatible invocation history"
git tag tutorial-11-history
```

Evidence: round-trip, old-record, malformed-line, and rotation tests pass from a temporary directory.

## Stretch exercise

Add `schema_version: u32` and deserialize through an untagged compatibility enum. Compare explicit version migration with scattered `#[serde(default)]` attributes.

## Reflection

- Which history fields help debugging without retaining unnecessary private data?
- What does Serde guarantee, and what compatibility policy must you still design?
- Why is append-only storage easier to recover than one repeatedly rewritten JSON array?
