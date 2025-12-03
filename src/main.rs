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
///   sai --init
///       -> creates a starter config file with placeholder API credentials
///
///   sai "natural language prompt"
///       -> uses global default prompt/tools from config.yaml
///
///   sai config_prompt.yml "natural language prompt"
///       -> uses prompt/tools from config_prompt.yml
///
/// Options:
///   -c / --confirm        Ask for confirmation before executing the generated command.
///   -u / --unsafe         Disable operator-level safety checks (always implies confirm).
///   -p / --peek           Provide one or more sample data files for the LLM to inspect.
///   -s / --scope          Provide path/glob hints to narrow the command output.
///        --list-tools     Show configured tool names and exit.
///        --init           Create a default ~/.config/sai/config.yaml and exit.
///        --create-prompt  Generate a starter prompt config (COMMAND [PATH]).
///        --add-prompt     Merge a prompt config file into the global config.
#[derive(Parser, Debug)]
#[command(name = "sai")]
#[command(version)]
#[command(about = "AI-powered, YAML-configured command executor", long_about = None)]
struct Cli {
    /// Initialize the default config file with placeholder values
    #[arg(long)]
    init: bool,

    /// Create a per-call prompt config template for the specified command and optional path
    #[arg(long, value_names = ["COMMAND", "PATH"], num_args = 1..=2)]
    create_prompt: Option<Vec<String>>,

    /// Merge tools from a prompt config file into the global default prompt
    #[arg(long, value_name = "PATH")]
    add_prompt: Option<String>,

    /// List the configured tools (global config and optional prompt file) and exit
    #[arg(long = "list-tools")]
    list_tools: bool,

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

    /// Provide a path or glob hint to narrow the LLM response
    #[arg(short = 's', long = "scope", value_name = "PATTERN")]
    scope: Option<String>,

    /// Either a per-call prompt config YAML file, or the natural language prompt (simple mode)
    #[arg(required_unless_present_any = ["init", "create_prompt", "add_prompt", "list_tools"])]
    arg1: Option<String>,

    /// Natural language prompt (advanced mode, when arg1 is a config file)
    prompt: Option<String>,
}

/// Global config file structure: infra + optional default prompt.
#[derive(Debug, Default, Serialize, Deserialize)]
struct GlobalConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ai: Option<AiConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    default_prompt: Option<PromptConfig>,
}

/// AI configuration that may come from file and/or environment.
#[derive(Debug, Default, Serialize, Deserialize)]
struct AiConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provider: Option<String>, // "openai" or "azure"

    // OpenAI
    #[serde(default, skip_serializing_if = "Option::is_none")]
    openai_api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    openai_base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    openai_model: Option<String>,

    // Azure OpenAI
    #[serde(default, skip_serializing_if = "Option::is_none")]
    azure_api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    azure_endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    azure_deployment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    azure_api_version: Option<String>,
}

/// Prompt configuration (also used as per-call config).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct PromptConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    meta_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ToolConfig>,
}

/// Single tool description for the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    if cli.init {
        init_global_config(&global_config_path)?;
        return Ok(());
    }

    if let Some(values) = cli.create_prompt.as_ref() {
        create_prompt_template(values)?;
        return Ok(());
    }

    if let Some(path) = cli.add_prompt.as_ref() {
        add_prompt_to_global(&global_config_path, Path::new(path))?;
        return Ok(());
    }

    if cli.list_tools {
        list_tools(&global_config_path, cli.arg1.as_deref())?;
        return Ok(());
    }

    let arg1 = cli.arg1.clone().ok_or_else(|| {
        anyhow!("Expected a prompt or prompt config path when not running with --init")
    })?;

    let global_cfg = load_global_config(&global_config_path)?;

    // Decide mode: simple vs advanced
    let (prompt_cfg, prompt_source): (PromptConfig, Option<PathBuf>) = match cli.prompt.as_ref() {
        Some(_nl_prompt) => {
            // Advanced mode: arg1 is config path, prompt is nl_prompt
            let cfg_path = PathBuf::from(&arg1);
            let prompt_cfg = load_prompt_config(&cfg_path)?;
            (prompt_cfg, Some(cfg_path))
        }
        None => {
            // Simple mode: arg1 is NL prompt, use global default_prompt
            let prompt_cfg = global_cfg.default_prompt.clone().ok_or_else(|| {
                anyhow!("No default_prompt found in global config for simple mode")
            })?;
            (prompt_cfg, None)
        }
    };

    // Natural language prompt string
    let nl_prompt = cli.prompt.clone().unwrap_or_else(|| arg1.clone());

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
        cli.scope.as_deref(),
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
            cli.scope.as_deref(),
            &cmd_line,
        )? {
            eprintln!("Cancelled.");
            return Ok(());
        }
    }

    // Execute the command
    let status = run_command(&cmd_line, &tokens, cli.unsafe_mode)?;
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

/// Create a template prompt config for a specific command, storing it at the requested path.
fn create_prompt_template(values: &[String]) -> Result<()> {
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

/// Merge a prompt config's tools into the global default prompt without overwriting duplicates.
fn add_prompt_to_global(global_path: &Path, prompt_path: &Path) -> Result<()> {
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

    for tool in &prompt_cfg.tools {
        if default_prompt
            .tools
            .iter()
            .any(|existing| existing.name == tool.name)
        {
            return Err(anyhow!(
                "Tool '{}' already exists in the global default prompt",
                tool.name
            ));
        }
    }

    if default_prompt.meta_prompt.is_none() {
        default_prompt.meta_prompt = prompt_cfg.meta_prompt.clone();
    }

    default_prompt.tools.extend(prompt_cfg.tools.clone());

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

/// List tools configured in the global default prompt and, optionally, an explicit prompt file.
fn list_tools(global_path: &Path, prompt_path: Option<&str>) -> Result<()> {
    let global_cfg = load_global_config(global_path)?;

    println!("Global config file: {}", global_path.display());
    match global_cfg.default_prompt {
        Some(ref prompt) if !prompt.tools.is_empty() => {
            println!("  Tools ({}):", prompt.tools.len());
            for tool in &prompt.tools {
                println!("    - {}", tool.name);
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
                println!("    - {}", tool.name);
            }
        }
    }

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

/// Create a starter config file with placeholder AI settings and no default tools.
fn init_global_config(path: &Path) -> Result<()> {
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

/// Build the system prompt from the prompt config and return also the list of allowed tool names.
fn build_system_prompt(prompt_cfg: &PromptConfig) -> Result<(String, Vec<String>)> {
    if prompt_cfg.tools.is_empty() {
        return Err(anyhow!(
            "Prompt config must define at least one tool under 'tools:'"
        ));
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
            let model = openai_model.ok_or_else(|| {
                anyhow!("OpenAI selected but no model configured (SAI_OPENAI_MODEL)")
            })?;
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
                anyhow!("Azure selected but no API version configured (SAI_AZURE_API_VERSION)")
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
/// The optional scope hint is sent as its own message so the model can focus on
/// relevant paths, and peek_text (when provided) is passed as a separate sample
/// message for schema inference.
fn call_llm(
    ai: &EffectiveAiConfig,
    system_prompt: &str,
    nl_prompt: &str,
    scope_hint: Option<&str>,
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

    if let Some(scope) = scope_hint {
        messages.push(Message {
            role: "user".to_string(),
            content: format!(
                "Focus your command on files or paths matching this scope:
{}",
                scope
            ),
        });
    }

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
    scope_hint: Option<&str>,
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
    if let Some(scope) = scope_hint {
        eprintln!("Scope hint:");
        eprintln!("  {}", scope);
        eprintln!();
    }
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

/// Run the command as a process. In safe mode we spawn the tool directly without shell
/// interpolation. In unsafe mode we hand the full command line to the platform shell so
/// pipes, redirects, and other operators function as expected.
fn run_command(cmd_line: &str, tokens: &[String], unsafe_mode: bool) -> Result<i32> {
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
