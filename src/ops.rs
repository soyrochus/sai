use crate::config::{load_global_config, load_prompt_config, PromptConfig, ToolConfig};
use anyhow::{anyhow, Context, Result};
use serde_yaml;
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

pub trait DuplicateResolverIo {
    fn is_interactive(&self) -> bool;
    fn write_str(&mut self, content: &str) -> Result<()>;
    fn read_line(&mut self, buf: &mut String) -> Result<usize>;
}

struct StdioDuplicateResolverIo;

impl DuplicateResolverIo for StdioDuplicateResolverIo {
    fn is_interactive(&self) -> bool {
        io::stdin().is_terminal()
    }

    fn write_str(&mut self, content: &str) -> Result<()> {
        let mut stdout = io::stdout();
        stdout
            .write_all(content.as_bytes())
            .context("Failed to write duplicate resolution prompt")?;
        stdout.flush().ok();
        Ok(())
    }

    fn read_line(&mut self, buf: &mut String) -> Result<usize> {
        io::stdin()
            .read_line(buf)
            .context("Failed to read duplicate resolution choice")
    }
}

#[derive(Debug)]
pub enum MergeResult {
    Applied(Vec<ToolConfig>),
    Cancelled,
}

pub fn resolve_duplicate_tools(
    existing: &[ToolConfig],
    incoming: &[ToolConfig],
    prompt_label: &str,
    io: &mut dyn DuplicateResolverIo,
) -> Result<MergeResult> {
    let mut merged = existing.to_vec();

    for tool in incoming {
        if let Some(pos) = merged.iter().position(|t| t.name == tool.name) {
            if !io.is_interactive() {
                return Err(anyhow!(
                    "Tool '{}' already exists in the global default prompt and interactive resolution is required. Re-run in a TTY to choose overwrite, skip, or cancel.",
                    tool.name
                ));
            }

            show_conflict(io, &merged[pos], tool, prompt_label)?;

            loop {
                io.write_str(
                    &format!(
                        "Conflict for tool '{}':\n\n[O] Overwrite global definition with imported definition\n[S] Skip imported definition (keep global)\n[C] Cancel entire import\n\nChoice [O/S/C]: ",
                        tool.name
                    ))?;

                let mut buf = String::new();
                let bytes = io.read_line(&mut buf)?;
                if bytes == 0 {
                    return Ok(MergeResult::Cancelled);
                }
                let trimmed = buf.trim().to_lowercase();
                match trimmed.as_str() {
                    "o" => {
                        merged[pos] = tool.clone();
                        break;
                    }
                    "s" => {
                        break;
                    }
                    "c" => {
                        return Ok(MergeResult::Cancelled);
                    }
                    _ => {
                        io.write_str("Please enter O, S, or C.\n\n")?;
                    }
                }
            }
        } else {
            merged.push(tool.clone());
        }
    }

    Ok(MergeResult::Applied(merged))
}

fn show_conflict(
    io: &mut dyn DuplicateResolverIo,
    existing: &ToolConfig,
    incoming: &ToolConfig,
    prompt_label: &str,
) -> Result<()> {
    io.write_str(&format!(
        "\n=== Tool conflict detected: '{}' ===\n",
        existing.name
    ))?;
    io.write_str("Current global definition:\n")?;
    io.write_str(&format!(
        "name: {}\nconfig:\n{}\n\n",
        existing.name, existing.config
    ))?;
    io.write_str(&format!(
        "Imported definition (from {}):\nname: {}\nconfig:\n{}\n\n",
        prompt_label, incoming.name, incoming.config
    ))?;
    Ok(())
}

pub fn create_prompt_template(values: &[String]) -> Result<()> {
    if values.is_empty() {
        return Err(anyhow!("--create-prompt requires at least a command name"));
    }

    let command = &values[0];
    let sanitized = sanitize_filename(command);
    let cwd = env::current_dir().context("Failed to determine current working directory")?;

    let mut path = if let Some(custom_path) = values.get(1) {
        PathBuf::from(custom_path)
    } else {
        PathBuf::from(format!("{}.yaml", sanitized))
    };

    if path.is_relative() {
        path = cwd.join(path);
    }

    if path.exists() {
        return Err(anyhow!(
            "Prompt config already exists at {}. Refusing to overwrite.",
            path.display()
        ));
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }

    let template = format!(
        "meta_prompt: |\n  Compose a single {cmd} command that satisfies the user request.\n  Do not add shell operators or use disallowed tools.\ntools:\n  - name: {cmd}\n    config: |\n      Accept a natural language request and emit one {cmd} invocation.\n      Include all required flags explicitly and avoid chaining other commands.\n",
        cmd = command
    );

    fs::write(&path, template).with_context(|| {
        format!(
            "Failed to write prompt config template to {}",
            path.display()
        )
    })?;

    println!(
        "Prompt config template for '{}' written to {}",
        command,
        path.display()
    );

    Ok(())
}

pub fn add_prompt_to_global(global_path: &Path, prompt_path: &Path) -> Result<()> {
    if !prompt_path.exists() {
        return Err(anyhow!(
            "Prompt file {} does not exist",
            prompt_path.display()
        ));
    }

    let prompt_cfg = load_prompt_config(prompt_path)?;
    if prompt_cfg.tools.is_empty() {
        return Err(anyhow!("Prompt config must define at least one tool"));
    }

    let mut global_cfg = load_global_config(global_path)?;
    let default_prompt = global_cfg
        .default_prompt
        .get_or_insert_with(PromptConfig::default);

    let prompt_label = prompt_path.display().to_string();
    let mut resolver = StdioDuplicateResolverIo;
    let merge_result = resolve_duplicate_tools(
        &default_prompt.tools,
        &prompt_cfg.tools,
        &prompt_label,
        &mut resolver,
    )?;

    let merged_tools = match merge_result {
        MergeResult::Applied(tools) => tools,
        MergeResult::Cancelled => {
            println!("Import cancelled; no changes applied.");
            return Ok(());
        }
    };

    if default_prompt.meta_prompt.is_none() {
        default_prompt.meta_prompt = prompt_cfg.meta_prompt.clone();
    }

    default_prompt.tools = merged_tools;

    if let Some(parent) = global_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory {}", parent.display()))?;
    }

    let mut serialized =
        serde_yaml::to_string(&global_cfg).context("Failed to serialize merged global config")?;
    if !serialized.ends_with('\n') {
        serialized.push('\n');
    }

    fs::write(global_path, serialized)
        .with_context(|| format!("Failed to write merged config to {}", global_path.display()))?;

    println!(
        "Merged prompt {} into {}",
        prompt_path.display(),
        global_path.display()
    );

    Ok(())
}

pub fn list_tools(global_path: &Path, prompt_path: Option<&str>) -> Result<()> {
    let global_cfg = load_global_config(global_path)?;

    println!("Global config file: {}", global_path.display());
    match global_cfg.default_prompt {
        Some(ref prompt) if !prompt.tools.is_empty() => {
            println!("  Tools ({}):", prompt.tools.len());
            for tool in &prompt.tools {
                println!("    - {} {}", tool.name, availability_status(&tool.name));
            }
        }
        Some(_) => println!("  Tools: (none configured)"),
        None => println!("  Default prompt: not configured"),
    }

    if let Some(path_str) = prompt_path {
        let path = Path::new(path_str);
        let prompt_cfg = load_prompt_config(path)?;
        println!();
        println!("Prompt file: {}", path.display());
        if prompt_cfg.tools.is_empty() {
            println!("  Tools: (none configured)");
        } else {
            println!("  Tools ({}):", prompt_cfg.tools.len());
            for tool in &prompt_cfg.tools {
                println!("    - {} {}", tool.name, availability_status(&tool.name));
            }
        }
    }

    Ok(())
}

pub fn init_global_config(path: &Path) -> Result<()> {
    if path.exists() {
        return Err(anyhow!(
            "Config file already exists at {}. Refusing to overwrite.",
            path.display()
        ));
    }

    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)
            .with_context(|| format!("Failed to create config directory {}", dir.display()))?;
    }

    let template = r#"ai:
  provider: openai
  openai_api_key: changeme
  openai_model: gpt-4.1-mini
  # openai_base_url: https://api.openai.com/v1
  # azure_api_key: changeme
  # azure_endpoint: https://your-azure-openai-resource.openai.azure.com
  # azure_deployment: changeme
  # azure_api_version: 2024-02-15-preview

default_prompt:
  meta_prompt: |
    You are SAI, a careful command composer. Only emit a single allowed tool command.
    Never introduce shell operators such as pipes or redirects unless the operator has
    explicitly enabled unsafe mode.
    Add tools to this configuration by running "sai --add-prompt path/to/prompt.yaml".
  tools: []
"#;

    fs::write(path, template)
        .with_context(|| format!("Failed to write default config file to {}", path.display()))?;

    println!("Default configuration written to {}", path.display());
    println!("Update the placeholder API credentials and add tools (e.g. with 'sai --add-prompt ...') before running sai.");

    Ok(())
}

fn sanitize_filename(name: &str) -> String {
    let mut sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();

    if sanitized.is_empty() || sanitized.chars().all(|c| c == '_') {
        sanitized = "prompt".to_string();
    }

    sanitized
}

fn availability_status(tool: &str) -> &'static str {
    if Path::new(tool).is_absolute() {
        return if Path::new(tool).exists() {
            "[x]"
        } else {
            "[ ]"
        };
    }

    env::var_os("PATH")
        .and_then(|paths| {
            env::split_paths(&paths).find_map(|dir| {
                let candidate = dir.join(tool);
                if candidate.is_file() {
                    Some("[x]")
                } else {
                    None
                }
            })
        })
        .unwrap_or("[ ]")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use tempfile::tempdir;

    #[test]
    fn sanitize_handles_weird_chars() {
        assert_eq!(sanitize_filename("ls|wc"), "ls_wc");
    }

    #[test]
    fn availability_reports_missing_for_fake_tool() {
        assert_eq!(availability_status("definitely-not-a-tool"), "[ ]");
    }

    #[test]
    fn create_prompt_template_writes_file() {
        let dir = tempdir().unwrap();
        let template_path = dir.path().join("cmd.yaml");
        create_prompt_template(&vec![
            "cmd".to_string(),
            template_path.to_string_lossy().to_string(),
        ])
        .unwrap();
        assert!(template_path.exists());
    }

    #[test]
    fn resolve_duplicate_overwrite_replaces_definition() {
        let existing = vec![ToolConfig {
            name: "echo".to_string(),
            config: "old".to_string(),
        }];
        let incoming = vec![ToolConfig {
            name: "echo".to_string(),
            config: "new".to_string(),
        }];

        let mut io = MockIo::new(vec!["o\n"], true);
        let result = resolve_duplicate_tools(&existing, &incoming, "import.yaml", &mut io).unwrap();
        match result {
            MergeResult::Applied(tools) => {
                assert_eq!(tools.len(), 1);
                assert_eq!(tools[0].config, "new");
            }
            MergeResult::Cancelled => panic!("unexpected cancel"),
        }
        assert!(io.output.contains("Current global definition"));
        assert!(io.output.contains("Imported definition"));
    }

    #[test]
    fn resolve_duplicate_skip_keeps_existing() {
        let existing = vec![ToolConfig {
            name: "echo".to_string(),
            config: "old".to_string(),
        }];
        let incoming = vec![ToolConfig {
            name: "echo".to_string(),
            config: "new".to_string(),
        }];

        let mut io = MockIo::new(vec!["s\n"], true);
        let result = resolve_duplicate_tools(&existing, &incoming, "import.yaml", &mut io).unwrap();
        match result {
            MergeResult::Applied(tools) => {
                assert_eq!(tools[0].config, "old");
            }
            MergeResult::Cancelled => panic!("unexpected cancel"),
        }
    }

    #[test]
    fn resolve_duplicate_cancel_aborts() {
        let existing = vec![ToolConfig {
            name: "echo".to_string(),
            config: "old".to_string(),
        }];
        let incoming = vec![ToolConfig {
            name: "echo".to_string(),
            config: "new".to_string(),
        }];

        let mut io = MockIo::new(vec!["c\n"], true);
        let result = resolve_duplicate_tools(&existing, &incoming, "import.yaml", &mut io).unwrap();
        match result {
            MergeResult::Applied(_) => panic!("expected cancel"),
            MergeResult::Cancelled => {}
        }
    }

    #[test]
    fn non_interactive_duplicate_errors() {
        let existing = vec![ToolConfig {
            name: "echo".to_string(),
            config: "old".to_string(),
        }];
        let incoming = vec![ToolConfig {
            name: "echo".to_string(),
            config: "new".to_string(),
        }];

        let mut io = MockIo::new(vec![], false);
        let err =
            resolve_duplicate_tools(&existing, &incoming, "import.yaml", &mut io).unwrap_err();
        assert!(err
            .to_string()
            .contains("interactive resolution is required"));
    }

    struct MockIo {
        inputs: VecDeque<String>,
        output: String,
        interactive: bool,
    }

    impl MockIo {
        fn new(inputs: Vec<&str>, interactive: bool) -> Self {
            Self {
                inputs: inputs.into_iter().map(|s| s.to_string()).collect(),
                output: String::new(),
                interactive,
            }
        }
    }

    impl DuplicateResolverIo for MockIo {
        fn is_interactive(&self) -> bool {
            self.interactive
        }

        fn write_str(&mut self, content: &str) -> Result<()> {
            self.output.push_str(content);
            Ok(())
        }

        fn read_line(&mut self, buf: &mut String) -> Result<usize> {
            if let Some(next) = self.inputs.pop_front() {
                buf.push_str(&next);
                Ok(next.len())
            } else {
                Ok(0)
            }
        }
    }
}
