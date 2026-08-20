//! Persistent history of submitted natural language prompts.
//!
//! This is deliberately separate from [`crate::history`], which records one
//! entry per invocation together with its outcome and is read only by
//! `--analyze`. Prompt history is written once per submitted prompt and read in
//! full when the interactive editor opens, so it keeps its own file and its own
//! size budget.

use crate::config;
use crate::history::now_iso_ts;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct PromptEntry {
    pub ts: String,
    pub prompt: String,
}

/// Size budget for the prompt history file.
///
/// Prompts are short — a typical entry serialises to well under 100 bytes — so
/// 256 KB holds several thousand of them. That is far more than reverse search
/// is useful over, while keeping the whole file cheap to read at editor startup.
pub const PROMPT_HISTORY_MAX_BYTES: u64 = 256_000;

pub fn prompt_history_path() -> PathBuf {
    config::config_root_dir().join("prompt_history.log")
}

fn backup_path(path: &Path) -> PathBuf {
    let mut backup = path.to_path_buf();
    backup.set_extension("log.1");
    backup
}

/// Append `prompt` to the history, unless it repeats the most recent entry.
///
/// Failures are reported but never propagated: losing a history entry must not
/// stop the user from getting their command.
pub fn record(prompt: &str) {
    if let Err(err) = try_record(prompt) {
        eprintln!("Warning: failed to record prompt history: {:#}", err);
    }
}

fn try_record(prompt: &str) -> Result<()> {
    if prompt.trim().is_empty() {
        return Ok(());
    }

    let path = prompt_history_path();

    // Consecutive duplicates add nothing to recall, so collapse them.
    if last_prompt(&path)?.as_deref() == Some(prompt) {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create prompt history directory {}",
                parent.display()
            )
        })?;
    }

    let mut options = OpenOptions::new();
    options.create(true).append(true);
    restrict_to_owner(&mut options);

    let mut file = options
        .open(&path)
        .with_context(|| format!("Failed to open prompt history {}", path.display()))?;

    let entry = PromptEntry {
        ts: now_iso_ts(),
        prompt: prompt.to_string(),
    };
    writeln!(file, "{}", serde_json::to_string(&entry)?)?;
    file.flush()?;

    rotate_if_needed(&path)?;
    Ok(())
}

/// Create the history file readable and writable by its owner only, where the
/// platform has a notion of file modes. Prompts can name internal hosts and
/// paths, so they should not be world-readable.
#[cfg(unix)]
fn restrict_to_owner(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
}

#[cfg(not(unix))]
fn restrict_to_owner(_options: &mut OpenOptions) {}

/// Every recorded prompt, newest first, with unparseable lines skipped.
pub fn load() -> Vec<String> {
    match try_load() {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("Warning: failed to read prompt history: {:#}", err);
            Vec::new()
        }
    }
}

fn try_load() -> Result<Vec<String>> {
    let path = prompt_history_path();

    // Read the rotated file first so its (older) entries end up further back.
    let mut prompts = read_prompts(&backup_path(&path))?;
    prompts.extend(read_prompts(&path)?);
    prompts.reverse();
    Ok(prompts)
}

/// Prompts from one file, oldest first. A missing file is an empty history.
fn read_prompts(path: &Path) -> Result<Vec<String>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = File::open(path)
        .with_context(|| format!("Failed to open prompt history {}", path.display()))?;
    let reader = BufReader::new(file);

    let mut prompts = Vec::new();
    for line in reader.lines() {
        let Ok(line) = line else {
            continue;
        };
        if line.trim().is_empty() {
            continue;
        }
        // One corrupt line must not cost the user the rest of their history.
        if let Ok(entry) = serde_json::from_str::<PromptEntry>(&line) {
            prompts.push(entry.prompt);
        }
    }

    Ok(prompts)
}

fn last_prompt(path: &Path) -> Result<Option<String>> {
    Ok(read_prompts(path)?.pop())
}

fn rotate_if_needed(path: &Path) -> Result<()> {
    let Ok(meta) = fs::metadata(path) else {
        return Ok(());
    };

    if meta.len() <= PROMPT_HISTORY_MAX_BYTES {
        return Ok(());
    }

    let backup = backup_path(path);
    if backup.exists() {
        fs::remove_file(&backup).with_context(|| {
            format!(
                "Failed to remove existing prompt history backup {}",
                backup.display()
            )
        })?;
    }

    fs::rename(path, &backup).with_context(|| {
        format!(
            "Failed to rotate prompt history {} -> {}",
            path.display(),
            backup.display()
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::set_config_dir_override_for_tests;
    use tempfile::TempDir;

    #[test]
    fn absent_store_is_an_empty_history() {
        let temp = TempDir::new().unwrap();
        let _guard = set_config_dir_override_for_tests(temp.path().join("config"));
        assert!(load().is_empty());
    }

    #[test]
    fn prompts_persist_newest_first() {
        let temp = TempDir::new().unwrap();
        let _guard = set_config_dir_override_for_tests(temp.path().join("config"));

        record("find large files");
        record("count records");
        record("list json files");

        assert_eq!(
            load(),
            vec![
                "list json files".to_string(),
                "count records".to_string(),
                "find large files".to_string(),
            ]
        );
    }

    #[test]
    fn consecutive_duplicates_are_collapsed() {
        let temp = TempDir::new().unwrap();
        let _guard = set_config_dir_override_for_tests(temp.path().join("config"));

        record("list json files");
        record("list json files");

        assert_eq!(load(), vec!["list json files".to_string()]);
    }

    #[test]
    fn multiline_prompt_round_trips_as_one_entry_without_losing_breaks() {
        let temp = TempDir::new().unwrap();
        let _guard = set_config_dir_override_for_tests(temp.path().join("config"));
        let prompt = "first line\nsecond line\nthird line";

        record(prompt);

        assert_eq!(load(), vec![prompt.to_string()]);
        let stored = fs::read_to_string(prompt_history_path()).unwrap();
        assert_eq!(
            stored.lines().count(),
            1,
            "one prompt must occupy one NDJSON record"
        );
        assert!(stored.contains("first line\\nsecond line\\nthird line"));
    }

    #[test]
    fn differently_placed_line_breaks_are_distinct_prompts() {
        let temp = TempDir::new().unwrap();
        let _guard = set_config_dir_override_for_tests(temp.path().join("config"));

        record("ab\nc");
        record("a\nbc");

        assert_eq!(load(), vec!["a\nbc".to_string(), "ab\nc".to_string()]);
    }

    #[test]
    fn non_consecutive_repeats_are_recorded() {
        let temp = TempDir::new().unwrap();
        let _guard = set_config_dir_override_for_tests(temp.path().join("config"));

        record("list json files");
        record("count records");
        record("list json files");

        assert_eq!(
            load(),
            vec![
                "list json files".to_string(),
                "count records".to_string(),
                "list json files".to_string(),
            ]
        );
    }

    #[test]
    fn corrupt_lines_are_skipped() {
        let temp = TempDir::new().unwrap();
        let _guard = set_config_dir_override_for_tests(temp.path().join("config"));

        record("find large files");

        let path = prompt_history_path();
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        writeln!(file, "this is not json").unwrap();
        writeln!(file, "{{\"ts\":\"x\"}}").unwrap();
        drop(file);

        record("count records");

        assert_eq!(
            load(),
            vec!["count records".to_string(), "find large files".to_string()]
        );
    }

    #[test]
    fn rotation_preserves_the_newest_entries() {
        let temp = TempDir::new().unwrap();
        let _guard = set_config_dir_override_for_tests(temp.path().join("config"));

        record("oldest prompt");
        // One oversized entry pushes the file past its budget.
        record(&"x".repeat((PROMPT_HISTORY_MAX_BYTES as usize) + 100));
        record("newest prompt");

        let path = prompt_history_path();
        assert!(backup_path(&path).exists(), "the file should have rotated");
        assert!(
            fs::metadata(&path).unwrap().len() <= PROMPT_HISTORY_MAX_BYTES,
            "the live file should be back under budget"
        );

        let loaded = load();
        assert_eq!(loaded.first().unwrap(), "newest prompt");
        assert!(loaded.iter().any(|p| p == "oldest prompt"));
    }

    #[test]
    fn blank_prompts_are_not_recorded() {
        let temp = TempDir::new().unwrap();
        let _guard = set_config_dir_override_for_tests(temp.path().join("config"));

        record("   ");
        record("");

        assert!(load().is_empty());
    }

    #[test]
    fn write_failure_does_not_panic() {
        let temp = TempDir::new().unwrap();
        // Point the config root at a path that cannot hold a directory.
        let blocker = temp.path().join("blocker");
        fs::write(&blocker, b"not a directory").unwrap();
        let _guard = set_config_dir_override_for_tests(blocker.join("config"));

        record("find large files");
        assert!(load().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn store_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().unwrap();
        let _guard = set_config_dir_override_for_tests(temp.path().join("config"));

        record("find large files");

        let mode = fs::metadata(prompt_history_path())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
