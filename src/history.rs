use crate::config;
use anyhow::{Context, Result};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub ts: String,
    pub cwd: String,
    pub argv: Vec<String>,
    pub exit_code: i32,
    pub generated_command: Option<String>,
    pub unsafe_mode: bool,
    pub confirm: bool,
    pub explain: bool,
    pub scope: Option<String>,
    pub peek_files: Vec<String>,
    pub notes: Option<String>,
}

pub const HISTORY_MAX_BYTES: u64 = 1_000_000;

pub fn history_log_path() -> PathBuf {
    config::config_root_dir().join("history.log")
}

pub fn write_entry(entry: HistoryEntry) -> Result<()> {
    let path = history_log_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create history directory {}", parent.display()))?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("Failed to open history log {}", path.display()))?;

    let line = serde_json::to_string(&entry)?;
    writeln!(file, "{}", line)?;
    file.flush()?;

    rotate_history_if_needed(&path)?;
    Ok(())
}

pub fn read_latest_entry() -> Result<Option<HistoryEntry>> {
    let path = history_log_path();

    if let Some(entry) = read_latest_from_file(&path)? {
        return Ok(Some(entry));
    }

    let backup = backup_path(&path);
    read_latest_from_file(&backup)
}

fn read_latest_from_file(path: &Path) -> Result<Option<HistoryEntry>> {
    if !path.exists() {
        return Ok(None);
    }

    let file = File::open(path)
        .with_context(|| format!("Failed to open history log {}", path.display()))?;
    let reader = BufReader::new(file);

    let mut last_good: Option<HistoryEntry> = None;
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(err) => {
                eprintln!("Skipping unreadable line in {}: {}", path.display(), err);
                continue;
            }
        };

        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<HistoryEntry>(&line) {
            Ok(entry) => last_good = Some(entry),
            Err(err) => {
                eprintln!(
                    "Skipping malformed history entry in {}: {}",
                    path.display(),
                    err
                );
            }
        }
    }

    Ok(last_good)
}

fn rotate_history_if_needed(path: &Path) -> Result<()> {
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return Ok(()),
    };

    if meta.len() <= HISTORY_MAX_BYTES {
        return Ok(());
    }

    let backup = backup_path(path);
    if backup.exists() {
        fs::remove_file(&backup).with_context(|| {
            format!(
                "Failed to remove existing history backup {}",
                backup.display()
            )
        })?;
    }

    fs::rename(path, &backup).with_context(|| {
        format!(
            "Failed to rotate history log {} -> {}",
            path.display(),
            backup.display()
        )
    })?;

    Ok(())
}

fn backup_path(path: &Path) -> PathBuf {
    let mut backup = path.to_path_buf();
    backup.set_extension("log.1");
    backup
}

pub fn now_iso_ts() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::set_config_dir_override_for_tests;
    use tempfile::TempDir;

    #[test]
    fn write_and_read_round_trip() {
        let temp = TempDir::new().unwrap();
        let _guard = set_config_dir_override_for_tests(temp.path().join("config"));

        let entry = HistoryEntry {
            ts: "2024-01-01T00:00:00Z".to_string(),
            cwd: "/tmp".to_string(),
            argv: vec!["sai".to_string()],
            exit_code: 0,
            generated_command: Some("echo hi".to_string()),
            unsafe_mode: false,
            confirm: true,
            explain: false,
            scope: Some(".".to_string()),
            peek_files: vec!["a.txt".to_string()],
            notes: Some("note".to_string()),
        };

        write_entry(entry.clone()).unwrap();
        let latest = read_latest_entry().unwrap().unwrap();
        assert_eq!(latest.generated_command, entry.generated_command);
        assert_eq!(latest.peek_files, entry.peek_files);
        assert!(latest.confirm);
    }

    #[test]
    fn rotates_when_size_exceeded() {
        let temp = TempDir::new().unwrap();
        let _guard = set_config_dir_override_for_tests(temp.path().join("config"));

        let base_entry = HistoryEntry {
            ts: "2024-01-01T00:00:00Z".to_string(),
            cwd: "/tmp".to_string(),
            argv: vec!["sai".to_string()],
            exit_code: 0,
            generated_command: Some("echo hi".to_string()),
            unsafe_mode: false,
            confirm: true,
            explain: false,
            scope: None,
            peek_files: Vec::new(),
            notes: Some("small".to_string()),
        };

        write_entry(base_entry.clone()).unwrap();

        let mut large_entry = base_entry.clone();
        large_entry.notes = Some("x".repeat((HISTORY_MAX_BYTES as usize) + 100));
        write_entry(large_entry).unwrap();

        let log_path = history_log_path();
        let backup = backup_path(&log_path);
        assert!(backup.exists());

        write_entry(base_entry.clone()).unwrap();
        let latest = read_latest_entry().unwrap().unwrap();
        assert_eq!(latest.notes, base_entry.notes);
    }
}
