use anyhow::{anyhow, Context, Result};
use clap::Parser;
use dirs::config_dir;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use shell_words;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Maximum number of bytes to read from each --peek file.
/// This is a safety/efficiency limit; the sample is for schema inference, not full data.
const PEEK_MAX_BYTES: usize = 16 * 1024;

/// sai: AI-assisted, config-driven command executor.
///
/// Usage:
///   sai "natural language prompt"
///       -> uses global default prompt/tools from config.yaml
///
///   sai config_prompt.yml "natural language prompt"
///       -> uses prompt/tools from config_prompt.yml
///
/// Options:
///   -c / --confirm   Ask for confirmation before executing the generated command.
///   -u / --unsafe    Disable operator-level safety checks (always implies confirm).
///   -p / --peek      Provide one or more sample data files for the LLM to inspect.
#[derive(Parser, Debug)]
#[command(name = "sai")]
#[command(version)]
#[command(about = "AI-powered, YAML-configured command executor", long_about = None)]
struct Cli {
    /// Ask for confirmation before executing the generated command
    #[arg(short, long)]
    confirm: bool,

    /// Disable operator-level safety checks (pipes, redirects, etc.).
    /// This always forces an interactive confirmation before running.
    #[arg(short = 'u', long = "unsafe")]
    unsafe_mode: bool,

    /// Sample data files to send to the LLM (truncated, for schema inference).
    /// Each file is read up to PEEK_MAX_BYTES and clearly marked as sample data.
    #[arg(short = 'p', long = "peek")]
    peek: Vec<String>,

    /// Either a per-call prompt config YAML file, or the natural language prompt (simple mode)
    arg1: String,

    /// Natural language prompt (advanced mode, when arg1 is a config file)
    prompt: Option<String>,
}

/// Global config file structure: infra + optional default prompt.
#[derive(Debug, Default, Deserialize)]
struct GlobalConfig {
    #[serde(default)]
    ai: Option<AiConfig>,

    #[serde(default)]
    default_prompt: Option<PromptConfig>,
}

/// AI configuration that may come from file and/or environment.
#[derive(Debug, Default, Deserialize)]
struct AiConfig {
    #[serde(default)]
    provider: Option<String>, // "openai" or "azure"

    // OpenAI
    #[serde(default)]
    openai_api_key: Option<String>,
    #[serde(default)]
    openai_base_url: Option<String>,
    #[serde(default)]
    openai_model: Option<String>,

    // Azure OpenAI
    #[serde(default)]
    azure_api_key: Option<String>,
    #[serde(default)]
    azure_endpoint: Option<String>,
    #[serde(default)]
    azure_deployment: Option<String>,
    #[serde(default)]
    azure_api_version: Option<String>,
}

/// Prompt configuration (also used as per-call config).
#[derive(Debug, Default, Clone, Deserialize)]
struct PromptConfig {
    #[serde(default)]
    meta_prompt: Option<String>,
    #[serde(default)]
    tools: Vec<ToolConfig>,
}

/// Single tool description for the LLM.
#[derive(Debug, Clone, Deserialize)]
struct ToolConfig {
    name: String,
    config: String,
}

/// Provider resolved after merging env + file.
enum EffectiveAiConfig {
    OpenAI {
        api_key: String,
        base_url: String,
        model: String,
    },
    Azure {
        api_key: String,
        endpoint: String,
        deployment: String,
        api_version: String,
    },
}

/// Structures to talk to the Chat Completions APIs.
#[derive(Serialize)]
struct ChatRequest {
    model: Option<String>,
    messages: Vec<Message>,
    temperature: f32,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let global_config_path = find_global_config_path();
    let global_cfg = load_global_config(&global_config_path)?;

    // Decide mode: simple vs advanced
    let (prompt_cfg, prompt_source): (PromptConfig, Option<PathBuf>) = match cli.prompt.as_ref() {
        Some(_nl_prompt) => {
            // Advanced mode: arg1 is config path, prompt is nl_prompt
            let cfg_path = PathBuf::from(&cli.arg1);
            let prompt_cfg = load_prompt_config(&cfg_path)?;
            (prompt_cfg, Some(cfg_path))
        }
        None => {
            // Simple mode: arg1 is NL prompt, use global default_prompt
            let prompt_cfg = global_cfg
                .default_prompt
                .clone()
                .ok_or_else(|| anyhow!("No default_prompt found in global config for simple mode"))?;
            (prompt_cfg, None)
        }
    };

    // Natural language prompt string
    let nl_prompt = cli
        .prompt
        .clone()
        .unwrap_or_else(|| cli.arg1.clone());

    // Build system prompt and allowed tool names from selected prompt config
    let (system_prompt, allowed_tools) = build_system_prompt(&prompt_cfg)?;

    // Build peek context if any peek files were provided
    let peek_context = build_peek_context(&cli.peek)?;

    // Resolve AI config (file + env) but only validate NOW, before first LLM call
    let effective_ai = resolve_ai_config(global_cfg.ai)?;

    // Call the LLM to get a command line
    let cmd_line = call_llm(
        &effective_ai,
        &system_prompt,
        &nl_prompt,
        peek_context.as_deref(),
    )
    .context("Failed to obtain command from LLM")?;

    // Show the raw command to stderr
    eprintln!(">> {}", cmd_line);

    // Basic safety check: first token must be allowed, and disallow dangerous operators unless unsafe_mode
    let tokens = validate_and_split_command(&cmd_line, &allowed_tools, cli.unsafe_mode)?;

    // Unsafe mode always implies confirmation
    let need_confirm = cli.confirm || cli.unsafe_mode;
    if need_confirm {
        if !confirm(
            &global_config_path,
            prompt_source.as_deref(),
            &nl_prompt,
            &cmd_line,
        )? {
            eprintln!("Cancelled.");
            return Ok(());
        }
    }

    // Execute the command
    let status = run_command(&tokens)?;
    std::process::exit(status);
}

/// Determine the global config path using OS-standard config directory.
fn find_global_config_path() -> PathBuf {
    let base = config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("sai").join("config.yaml")
}

/// Load the global config file; if missing, return an empty default.
fn load_global_config(path: &Path) -> Result<GlobalConfig> {
    if !path.exists() {
        // It's valid to run without a file; AI config can come from env.
        return Ok(GlobalConfig::default());
    }
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read global config file {}", path.display()))?;
    let cfg: GlobalConfig = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse global config YAML {}", path.display()))?;
    Ok(cfg)
}

/// Load a per-call prompt config YAML.
fn load_prompt_config(path: &Path) -> Result<PromptConfig> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read prompt config file {}", path.display()))?;
    let cfg: PromptConfig = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse prompt config YAML {}", path.display()))?;
    Ok(cfg)
}

/// Build the system prompt from the prompt config and return also the list of allowed tool names.
fn build_system_prompt(prompt_cfg: &PromptConfig) -> Result<(String, Vec<String>)> {
    if prompt_cfg.tools.is_empty() {
        return Err(anyhow!("Prompt config must define at least one tool under 'tools:'"));
    }

    let meta_prompt = prompt_cfg
        .meta_prompt
        .clone()
        .unwrap_or_else(|| "".to_string());

    let mut allowed_names = Vec::new();
    let mut tool_texts = Vec::new();

    for tool in &prompt_cfg.tools {
        if tool.name.trim().is_empty() || tool.config.trim().is_empty() {
            return Err(anyhow!(
                "Each tool must have non-empty 'name' and 'config' fields"
            ));
        }
        allowed_names.push(tool.name.clone());
        tool_texts.push(tool.config.clone());
    }

    let mut tools_listing = String::from("You may ONLY use the following tools:\n");
    for name in &allowed_names {
        tools_listing.push_str(&format!("- {}\n", name));
    }

    let mut system_parts = Vec::new();
    if !meta_prompt.trim().is_empty() {
        system_parts.push(meta_prompt.trim().to_string());
    }
    system_parts.push(tools_listing);
    system_parts.push(format!("\nTool details:\n\n{}", tool_texts.join("\n\n")));

    let full_prompt = system_parts.join("\n\n").trim().to_string();
    Ok((full_prompt, allowed_names))
}

/// Read peek files (if any) and build a single text block to send to the LLM.
/// Each file is truncated to PEEK_MAX_BYTES and clearly delimited.
/// If no files are provided, returns Ok(None).
fn build_peek_context(peek_files: &[String]) -> Result<Option<String>> {
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

/// Merge AI config from global file and environment, and validate for the chosen provider.
fn resolve_ai_config(global_ai: Option<AiConfig>) -> Result<EffectiveAiConfig> {
    let file_ai = global_ai.unwrap_or_default();

    // Helper: env var overrides file
    let provider = env_or(file_ai.provider, "SAI_PROVIDER");

    let openai_api_key = env_or(file_ai.openai_api_key, "SAI_OPENAI_API_KEY");
    let openai_base_url = env_or(file_ai.openai_base_url, "SAI_OPENAI_BASE_URL");
    let openai_model = env_or(file_ai.openai_model, "SAI_OPENAI_MODEL");

    let azure_api_key = env_or(file_ai.azure_api_key, "SAI_AZURE_API_KEY");
    let azure_endpoint = env_or(file_ai.azure_endpoint, "SAI_AZURE_ENDPOINT");
    let azure_deployment = env_or(file_ai.azure_deployment, "SAI_AZURE_DEPLOYMENT");
    let azure_api_version = env_or(file_ai.azure_api_version, "SAI_AZURE_API_VERSION");

    // Decide provider:
    let provider = if let Some(p) = provider {
        p.to_lowercase()
    } else {
        // No explicit provider: try to infer from presence of keys
        if openai_api_key.is_some() {
            "openai".to_string()
        } else if azure_api_key.is_some() {
            "azure".to_string()
        } else {
            return Err(anyhow!(
                "No AI configuration found: set OpenAI or Azure info in config or environment"
            ));
        }
    };

    match provider.as_str() {
        "openai" => {
            let api_key = openai_api_key.ok_or_else(|| {
                anyhow!("OpenAI selected but no OPENAI API key configured (SAI_OPENAI_API_KEY)")
            })?;
            let base_url =
                openai_base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            let model = openai_model
                .ok_or_else(|| anyhow!("OpenAI selected but no model configured (SAI_OPENAI_MODEL)"))?;
            Ok(EffectiveAiConfig::OpenAI {
                api_key,
                base_url,
                model,
            })
        }
        "azure" => {
            let api_key = azure_api_key.ok_or_else(|| {
                anyhow!("Azure selected but no AZURE API key configured (SAI_AZURE_API_KEY)")
            })?;
            let endpoint = azure_endpoint.ok_or_else(|| {
                anyhow!("Azure selected but no endpoint configured (SAI_AZURE_ENDPOINT)")
            })?;
            let deployment = azure_deployment.ok_or_else(|| {
                anyhow!("Azure selected but no deployment configured (SAI_AZURE_DEPLOYMENT)")
            })?;
            let api_version = azure_api_version.ok_or_else(|| {
                anyhow!(
                    "Azure selected but no API version configured (SAI_AZURE_API_VERSION)"
                )
            })?;
            Ok(EffectiveAiConfig::Azure {
                api_key,
                endpoint,
                deployment,
                api_version,
            })
        }
        other => Err(anyhow!(
            "Unsupported provider '{}'. Use 'openai' or 'azure'.",
            other
        )),
    }
}

/// Read env var; if set, override the config value.
fn env_or(file_value: Option<String>, env_key: &str) -> Option<String> {
    if let Ok(v) = env::var(env_key) {
        if !v.is_empty() {
            return Some(v);
        }
    }
    file_value
}

/// Call the LLM (OpenAI or Azure) and return a single-line command string.
/// If peek_text is Some(...), it is passed as a separate message explicitly
/// tagged as sample data for schema inference.
fn call_llm(
    ai: &EffectiveAiConfig,
    system_prompt: &str,
    nl_prompt: &str,
    peek_text: Option<&str>,
) -> Result<String> {
    let client = Client::new();

    let mut messages = vec![
        Message {
            role: "system".to_string(),
            content: system_prompt.to_string(),
        },
        Message {
            role: "user".to_string(),
            content: nl_prompt.to_string(),
        },
    ];

    if let Some(peek) = peek_text {
        messages.push(Message {
            role: "user".to_string(),
            content: format!(
                "Here is a sample of the data the tools will operate on. \
                 It may be truncated and is provided only to infer structure and field names, \
                 not to be hard-coded:\n\n{}",
                peek
            ),
        });
    }

    match ai {
        EffectiveAiConfig::OpenAI {
            api_key,
            base_url,
            model,
        } => {
            let req = ChatRequest {
                model: Some(model.clone()),
                messages,
                temperature: 0.0,
            };
            let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
            let resp: ChatResponse = client
                .post(&url)
                .bearer_auth(api_key)
                .json(&req)
                .send()
                .context("HTTP error calling OpenAI")?
                .error_for_status()
                .context("Non-success status from OpenAI")?
                .json()
                .context("Failed to parse OpenAI response JSON")?;

            extract_first_line(&resp)
        }
        EffectiveAiConfig::Azure {
            api_key,
            endpoint,
            deployment,
            api_version,
        } => {
            let req = ChatRequest {
                model: None, // Azure uses deployment instead
                messages,
                temperature: 0.0,
            };
            let url = format!(
                "{}/openai/deployments/{}/chat/completions?api-version={}",
                endpoint.trim_end_matches('/'),
                deployment,
                api_version
            );
            let resp: ChatResponse = client
                .post(&url)
                .header("api-key", api_key)
                .json(&req)
                .send()
                .context("HTTP error calling Azure OpenAI")?
                .error_for_status()
                .context("Non-success status from Azure OpenAI")?
                .json()
                .context("Failed to parse Azure OpenAI response JSON")?;

            extract_first_line(&resp)
        }
    }
}

/// Extract first line of content from chat response, stripping markdown fences if needed.
fn extract_first_line(resp: &ChatResponse) -> Result<String> {
    let content = resp
        .choices
        .get(0)
        .ok_or_else(|| anyhow!("No choices in LLM response"))?
        .message
        .content
        .trim()
        .to_string();

    // Strip fenced code blocks if present
    let mut text = content.clone();
    if text.starts_with("```") {
        let mut cleaned = String::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("```") {
                continue;
            }
            cleaned.push_str(line);
            cleaned.push('\n');
        }
        text = cleaned.trim().to_string();
    }

    let first_line = text
        .lines()
        .next()
        .ok_or_else(|| anyhow!("Empty content from LLM"))?
        .trim()
        .to_string();

    if first_line.is_empty() {
        Err(anyhow!("LLM returned an empty command line"))
    } else {
        Ok(first_line)
    }
}

/// Validate the command line against allowed tools and, unless unsafe_mode is set,
/// reject dangerous shell operators. Returns the tokenized command.
fn validate_and_split_command(
    cmd_line: &str,
    allowed_tools: &[String],
    unsafe_mode: bool,
) -> Result<Vec<String>> {
    let tokens =
        shell_words::split(cmd_line).context("Failed to split command line from LLM output")?;

    if tokens.is_empty() {
        return Err(anyhow!("LLM returned an empty command after parsing"));
    }

    let first = &tokens[0];
    if !allowed_tools.iter().any(|t| t == first) {
        return Err(anyhow!(
            "Disallowed command '{}'. Allowed tools: {}",
            first,
            allowed_tools.join(", ")
        ));
    }

    // If not in unsafe mode, enforce operator-level safety.
    if !unsafe_mode {
        if let Some(op) = detect_forbidden_operator(cmd_line) {
            return Err(anyhow!(
                "Disallowed shell operator or construct '{}' in generated command. \
                 Re-run with --unsafe if you really want to execute it.",
                op
            ));
        }
    }

    Ok(tokens)
}

/// Scan the raw command line for forbidden shell operators when safety checks are enabled.
/// We treat any unescaped operator outside single quotes as unsafe. Command substitution is
/// disallowed even inside double quotes because the shell would still execute it.
fn detect_forbidden_operator(cmd_line: &str) -> Option<String> {
    let mut chars = cmd_line.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;

    while let Some(c) = chars.next() {
        if escaped {
            escaped = false;
            continue;
        }

        match c {
            '\\' if !in_single => {
                escaped = true;
                continue;
            }
            '\'' if !in_double => {
                in_single = !in_single;
                continue;
            }
            '"' if !in_single => {
                in_double = !in_double;
                continue;
            }
            _ => {}
        }

        if in_single {
            continue;
        }

        match c {
            '$' => {
                if let Some(&next) = chars.peek() {
                    if next == '(' {
                        return Some("$(...)".to_string());
                    }
                    if next == '{' {
                        return Some("${...}".to_string());
                    }
                }
            }
            '`' => {
                return Some("`...`".to_string());
            }
            _ => {}
        }

        if in_double {
            continue;
        }

        match c {
            '|' => {
                if let Some(&next) = chars.peek() {
                    if next == '|' {
                        return Some("||".to_string());
                    }
                    if next == '&' {
                        return Some("|&".to_string());
                    }
                }
                return Some("|".to_string());
            }
            '&' => {
                if let Some(&next) = chars.peek() {
                    if next == '&' {
                        return Some("&&".to_string());
                    }
                }
                return Some("&".to_string());
            }
            ';' => {
                return Some(";".to_string());
            }
            '>' => {
                if let Some(&next) = chars.peek() {
                    if next == '>' {
                        return Some(">>".to_string());
                    }
                    if next == '(' {
                        return Some(">(".to_string());
                    }
                }
                return Some(">".to_string());
            }
            '<' => {
                if let Some(&next) = chars.peek() {
                    if next == '<' {
                        return Some("<<".to_string());
                    }
                    if next == '(' {
                        return Some("<(".to_string());
                    }
                }
                return Some("<".to_string());
            }
            _ => {}
        }
    }

    None
}

/// Ask the user for confirmation, showing config paths, NL input and command.
fn confirm(
    global_cfg_path: &Path,
    prompt_cfg_path: Option<&Path>,
    nl_prompt: &str,
    cmd_line: &str,
) -> Result<bool> {
    eprintln!("Global config file: {}", global_cfg_path.display());
    if let Some(p) = prompt_cfg_path {
        eprintln!("Prompt config file: {}", p.display());
    } else {
        eprintln!("Prompt config: default_prompt from global config");
    }
    eprintln!();
    eprintln!("Natural language prompt:");
    eprintln!("  {}", nl_prompt);
    eprintln!();
    eprintln!("LLM output (command):");
    eprintln!("  {}", cmd_line);
    eprintln!();

    eprint!("Execute this command? [y/N] ");
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    let ans = buf.trim().to_lowercase();
    Ok(ans == "y" || ans == "yes")
}

/// Run the command as a process (no shell, just command + args).
fn run_command(tokens: &[String]) -> Result<i32> {
    let mut cmd = Command::new(&tokens[0]);
    if tokens.len() > 1 {
        cmd.args(&tokens[1..]);
    }
    let status = cmd
        .status()
        .with_context(|| format!("Failed to execute command '{}'", tokens[0]))?;
    Ok(status.code().unwrap_or(1))
}
