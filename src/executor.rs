use anyhow::{Context, Result};
use glob::glob;
use std::process::Command;

/// Expands glob patterns in a command argument.
/// If the argument contains glob metacharacters (*, ?, [) and matches files,
/// returns the expanded paths. Otherwise returns the original argument.
fn expand_glob_if_needed(arg: &str) -> Vec<String> {
    // Check if this looks like a glob pattern
    if !arg.contains('*') && !arg.contains('?') && !arg.contains('[') {
        return vec![arg.to_string()];
    }

    // Try to expand the glob
    match glob(arg) {
        Ok(paths) => {
            let expanded: Vec<String> = paths
                .filter_map(|entry| entry.ok())
                .map(|path| path.to_string_lossy().to_string())
                .collect();

            // If we got matches, use them; otherwise fall back to literal
            if expanded.is_empty() {
                vec![arg.to_string()]
            } else {
                expanded
            }
        }
        Err(_) => {
            // If glob parsing fails, use the literal string
            vec![arg.to_string()]
        }
    }
}

pub trait CommandExecutor {
    fn execute(&self, cmd_line: &str, tokens: &[String], unsafe_mode: bool) -> Result<i32>;
}

pub struct ShellCommandExecutor;

impl CommandExecutor for ShellCommandExecutor {
    fn execute(&self, cmd_line: &str, tokens: &[String], unsafe_mode: bool) -> Result<i32> {
        let status = if unsafe_mode {
            #[cfg(windows)]
            let mut cmd = {
                let mut command = Command::new("cmd");
                command.arg("/C").arg(cmd_line);
                command
            };

            #[cfg(not(windows))]
            let mut cmd = {
                let mut command = Command::new("sh");
                command.arg("-c").arg(cmd_line);
                command
            };

            cmd.status()
                .with_context(|| format!("Failed to execute command '{}'", cmd_line))?
        } else {
            // Safe mode: expand globs in arguments before executing
            let mut cmd = Command::new(&tokens[0]);
            if tokens.len() > 1 {
                let mut expanded_args = Vec::new();
                for arg in &tokens[1..] {
                    expanded_args.extend(expand_glob_if_needed(arg));
                }
                cmd.args(&expanded_args);
            }
            cmd.status()
                .with_context(|| format!("Failed to execute command '{}'", tokens[0]))?
        };

        Ok(status.code().unwrap_or(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::TempDir;

    struct NoopExecutor;

    impl CommandExecutor for NoopExecutor {
        fn execute(&self, _: &str, _: &[String], _: bool) -> Result<i32> {
            Ok(0)
        }
    }

    #[test]
    fn noop_executor_returns_zero() {
        let exec = NoopExecutor;
        assert_eq!(exec.execute("", &[], false).unwrap(), 0);
    }

    #[test]
    fn expand_glob_no_metacharacters() {
        let result = expand_glob_if_needed("simple.txt");
        assert_eq!(result, vec!["simple.txt"]);
    }

    #[test]
    fn expand_glob_with_matches() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create test files
        File::create(base.join("test1.txt")).unwrap();
        File::create(base.join("test2.txt")).unwrap();

        let pattern = format!("{}/*.txt", base.display());
        let result = expand_glob_if_needed(&pattern);

        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|s| s.ends_with("test1.txt")));
        assert!(result.iter().any(|s| s.ends_with("test2.txt")));
    }

    #[test]
    fn expand_glob_no_matches() {
        let result = expand_glob_if_needed("/nonexistent/path/*.txt");
        // Should fall back to literal when no matches
        assert_eq!(result, vec!["/nonexistent/path/*.txt"]);
    }

    #[test]
    fn expand_glob_invalid_pattern() {
        // Unclosed bracket - invalid glob pattern
        let result = expand_glob_if_needed("file[.txt");
        // Should fall back to literal on parse error
        assert_eq!(result, vec!["file[.txt"]);
    }
}
