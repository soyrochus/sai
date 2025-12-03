use anyhow::{Context, Result};
use std::process::Command;

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
            let mut cmd = Command::new(&tokens[0]);
            if tokens.len() > 1 {
                cmd.args(&tokens[1..]);
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
}
