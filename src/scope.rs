use anyhow::{Context, Result};
use std::env;
use std::fs;

pub const SCOPE_DOT_MAX_BYTES: usize = 8 * 1024;
const TRUNCATION_NOTE: &str = "(truncated directory listing)";

pub fn build_scope_dot_listing() -> Result<String> {
    let cwd = env::current_dir().context("Failed to determine current directory")?;
    let mut entries = Vec::new();
    let dir_iter = fs::read_dir(&cwd)
        .with_context(|| format!("Failed to list directory {}", cwd.display()))?;

    for entry in dir_iter {
        let entry = entry?;
        let mut name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_dir() {
            name.push('/');
        }
        entries.push(name);
    }

    entries.sort();

    let max_content_len = SCOPE_DOT_MAX_BYTES.saturating_sub(TRUNCATION_NOTE.len() + 1);
    let mut listing = String::new();
    let mut truncated = false;
    for name in entries {
        let addition_len = name.len() + if listing.is_empty() { 0 } else { 1 };
        if listing.len() + addition_len > max_content_len {
            truncated = true;
            break;
        }

        if !listing.is_empty() {
            listing.push('\n');
        }
        listing.push_str(&name);
    }

    if truncated {
        if !listing.is_empty() {
            listing.push('\n');
        }
        listing.push_str(TRUNCATION_NOTE);
    }

    Ok(listing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::sync::Mutex;
    use tempfile::tempdir;

    // Global mutex to ensure only one test changes current directory at a time
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn with_temp_cwd<F: FnOnce() -> R, R>(dir: &tempfile::TempDir, f: F) -> R {
        let _guard = TEST_MUTEX.lock().unwrap();
        let original = env::current_dir().unwrap();
        env::set_current_dir(dir.path()).unwrap();
        let result = f();
        env::set_current_dir(original).unwrap();
        result
    }

    #[test]
    fn empty_directory_produces_empty_listing() {
        let dir = tempdir().unwrap();
        let listing = with_temp_cwd(&dir, || build_scope_dot_listing().unwrap());
        assert_eq!(listing, "");
    }

    #[test]
    fn directory_listing_marks_directories() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("file.txt");
        File::create(file_path).unwrap();
        let subdir = dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();
        let listing = with_temp_cwd(&dir, || build_scope_dot_listing().unwrap());
        assert!(listing.contains("file.txt"));
        assert!(listing.contains("subdir/"));
    }

    #[test]
    fn directory_listing_truncates() {
        let dir = tempdir().unwrap();
        for i in 0..500 {
            let name = format!("long_file_name_{}_{}", i, "x".repeat(20));
            let path = dir.path().join(&name);
            let mut file = File::create(&path).unwrap();
            writeln!(file, "data").unwrap();
        }

        let listing = with_temp_cwd(&dir, || build_scope_dot_listing().unwrap());
        assert!(listing.contains(TRUNCATION_NOTE));
        assert!(listing.len() <= SCOPE_DOT_MAX_BYTES);
    }
}
