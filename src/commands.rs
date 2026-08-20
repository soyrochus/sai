use crate::config::{GlobalConfig, commands_dir};
use crate::ops::program_on_path;
use crate::safety_mode::SafetyMode;
use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenCommand {
    pub name: String,
    pub command: String,
    pub tokens: Vec<String>,
    pub intent: String,
    pub frozen_at: String,
    pub safety_mode: SafetyMode,
    pub tools: Vec<String>,
    pub prompt_config: String,
    pub risk_markers: Vec<String>,
}

impl FrozenCommand {
    fn fields(&self) -> [(&'static str, String); 7] {
        [
            ("intent", self.intent.clone()),
            ("frozen-at", self.frozen_at.clone()),
            ("safety", self.safety_mode.as_str().into()),
            ("tools", self.tools.join(",")),
            ("prompt-config", self.prompt_config.clone()),
            ("risk-markers", self.risk_markers.join(" | ")),
            ("command", self.command.clone()),
        ]
    }

    pub fn render(&self) -> String {
        let mut out = String::from("#!/usr/bin/env bash\n");
        for (key, value) in self.fields() {
            out.push_str(&format!(
                "# sai:{key}={}\n",
                serde_json::to_string(&value).unwrap()
            ));
        }
        out.push_str("set -euo pipefail\n");
        if !self.risk_markers.is_empty() {
            out.push_str("read -rp 'This frozen command was marked risky. Continue? [y/N] ' sai_answer\n[[ \"$sai_answer\" == y || \"$sai_answer\" == yes ]] || exit 1\n");
        }
        if self.safety_mode.uses_shell() {
            out.push_str(&self.command);
        } else {
            out.push_str(
                &self
                    .tokens
                    .iter()
                    .map(|token| {
                        // Mirrors executor.rs default-mode glob expansion.
                        if token.contains(['*', '?', '[']) {
                            token.clone()
                        } else {
                            shell_words::quote(token).into_owned()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }
        out.push('\n');
        out
    }

    pub fn parse(name: String, text: &str) -> Result<Self> {
        if !text.starts_with("#!/usr/bin/env bash\n# sai:") {
            return Err(anyhow!("not a SAI command"));
        }
        let mut get = |wanted: &str| -> Result<String> {
            let prefix = format!("# sai:{wanted}=");
            let raw = text
                .lines()
                .find_map(|line| line.strip_prefix(&prefix))
                .ok_or_else(|| anyhow!("missing {wanted} header"))?;
            serde_json::from_str(raw).with_context(|| format!("invalid {wanted} header"))
        };
        let safety = get("safety")?;
        let command = get("command")?;
        Ok(Self {
            name,
            tokens: shell_words::split(&command).unwrap_or_default(),
            command,
            intent: get("intent")?,
            frozen_at: get("frozen-at")?,
            safety_mode: SafetyMode::parse(&safety)
                .ok_or_else(|| anyhow!("invalid safety header"))?,
            tools: split_csv(&get("tools")?),
            prompt_config: get("prompt-config")?,
            risk_markers: get("risk-markers")?
                .split(" | ")
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
        })
    }
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

pub fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return Err(anyhow!("Frozen command name must be a single file name"));
    }
    Ok(())
}

#[cfg(unix)]
pub fn write(command: &FrozenCommand, config: &GlobalConfig) -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    validate_name(&command.name)?;
    let dir = commands_dir(config);
    fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    let target = dir.join(&command.name);
    let temp = dir.join(format!(".{}.{}.tmp", command.name, std::process::id()));
    fs::write(&temp, command.render())
        .with_context(|| format!("Failed to write {}", temp.display()))?;
    fs::set_permissions(&temp, fs::Permissions::from_mode(0o700))?;
    if let Err(error) = fs::rename(&temp, &target) {
        let _ = fs::remove_file(&temp);
        return Err(error.into());
    }
    Ok(target)
}

#[cfg(not(unix))]
pub fn write(_command: &FrozenCommand, _config: &GlobalConfig) -> Result<PathBuf> {
    Err(anyhow!(
        "Frozen commands are not yet supported on this platform"
    ))
}

pub fn list(config: &GlobalConfig) -> Result<Vec<FrozenCommand>> {
    let dir = commands_dir(config);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let Ok(text) = fs::read_to_string(entry.path()) else {
            continue;
        };
        if let Ok(command) = FrozenCommand::parse(entry.file_name().to_string_lossy().into(), &text)
        {
            result.push(command);
        }
    }
    result.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(result)
}

pub fn format_listing(commands: &[FrozenCommand]) -> String {
    if commands.is_empty() {
        return "no frozen commands".into();
    }
    commands
        .iter()
        .map(|c| {
            let mut flags = Vec::new();
            if c.safety_mode.is_unrestricted() {
                flags.push("unrestricted".to_string());
            }
            let missing: Vec<_> = c
                .tools
                .iter()
                .filter(|t| program_on_path(t).is_none())
                .cloned()
                .collect();
            if !missing.is_empty() {
                flags.push(format!("missing: {}", missing.join(", ")));
            }
            format!(
                "{} - {}{}",
                c.name,
                c.intent,
                if flags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", flags.join("; "))
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn card(mode: SafetyMode) -> FrozenCommand {
        FrozenCommand {
            name: "demo".into(),
            command: "printf '%s\\n' 'a b' src/*".into(),
            tokens: vec![
                "printf".into(),
                "%s\\n".into(),
                "a b".into(),
                "src/*".into(),
            ],
            intent: "multi\nline".into(),
            frozen_at: "2026-01-01T00:00:00Z".into(),
            safety_mode: mode,
            tools: vec!["printf".into()],
            prompt_config: "global".into(),
            risk_markers: vec![],
        }
    }
    #[test]
    fn headers_round_trip() {
        let c = card(SafetyMode::Unsafe);
        assert_eq!(
            FrozenCommand::parse(c.name.clone(), &c.render()).unwrap(),
            c
        );
    }
    #[test]
    fn default_quotes_except_globs() {
        let text = card(SafetyMode::Default).render();
        assert!(text.contains("'a b' src/*"));
        assert!(!text.contains("\"$@\""));
    }
    #[test]
    fn shell_modes_are_verbatim() {
        let c = card(SafetyMode::Unsafe);
        assert!(c.render().ends_with("printf '%s\\n' 'a b' src/*\n"));
    }
    #[test]
    fn risk_guard_is_conditional() {
        let mut c = card(SafetyMode::Default);
        assert!(!c.render().contains("read -rp"));
        c.risk_markers.push("destructive".into());
        assert!(c.render().contains("read -rp"));
    }
}
