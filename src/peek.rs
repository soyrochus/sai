use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Maximum number of bytes to read from each --peek file.
pub const PEEK_MAX_BYTES: usize = 16 * 1024;

pub fn build_peek_context(peek_files: &[String]) -> Result<Option<String>> {
    if peek_files.is_empty() {
        return Ok(None);
    }

    let mut out = String::new();
    for (idx, path_str) in peek_files.iter().enumerate() {
        let path = Path::new(path_str);
        let data = fs::read(path)
            .with_context(|| format!("Failed to read peek file {}", path.display()))?;

        let truncated = if data.len() > PEEK_MAX_BYTES {
            &data[..PEEK_MAX_BYTES]
        } else {
            &data[..]
        };

        let text = String::from_utf8_lossy(truncated);

        out.push_str(&format!("=== Sample {}: {} ===\n", idx + 1, path.display()));
        if data.len() > PEEK_MAX_BYTES {
            out.push_str(&format!("(truncated after {} bytes)\n", PEEK_MAX_BYTES));
        }
        out.push_str("```text\n");
        out.push_str(&text);
        out.push_str("\n```\n\n");
    }

    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn peek_context_includes_samples() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("sample.txt");
        let mut file = File::create(&path).unwrap();
        writeln!(file, "hello world").unwrap();

        let peek = build_peek_context(&[path.to_string_lossy().to_string()])
            .unwrap()
            .unwrap();
        assert!(peek.contains("Sample 1"));
        assert!(peek.contains("hello world"));
    }
}
