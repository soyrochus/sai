use anyhow::{anyhow, Context, Result};
use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Global config file structure: infra + optional default prompt.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct GlobalConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai: Option<AiConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_prompt: Option<PromptConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safety: Option<SafetySettings>,
}

/// Machine-level limits on which safety modes may be used at all.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct SafetySettings {
    /// Whether `--unrestricted` may be used. Absent means allowed, so existing
    /// configurations keep working; only an explicit `false` forbids it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_unrestricted: Option<bool>,
}

impl GlobalConfig {
    /// Whether unrestricted mode is permitted by this configuration.
    pub fn allows_unrestricted(&self) -> bool {
        self.safety
            .as_ref()
            .and_then(|s| s.allow_unrestricted)
            .unwrap_or(true)
    }
}

/// AI configuration that may come from file and/or environment.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct AiConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>, // "openai" or "azure"

    // OpenAI
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai_api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai_base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai_api_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai_model_snapshot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai_reasoning_effort: Option<String>,

    // Azure OpenAI
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub azure_api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub azure_endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub azure_deployment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub azure_api_version: Option<String>,
}

/// Prompt configuration (also used as per-call config).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PromptConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolConfig>,
}

/// Single tool description for the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    pub name: String,

    /// Forces explain mode when this tool is used in a generated command.
    /// When true, the tool automatically triggers --explain behavior even if
    /// the flag wasn't specified, providing an additional safety layer for
    /// destructive or complex operations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force_explain: Option<bool>,

    /// The tool configuration or description.
    /// Example:
    ///    Tool: jq
    ///    Role: filter and transform JSON input.
    ///    Rules:
    ///        - Commands must start with "jq".
    ///        - Use a single filter expression.
    ///        - Input files must appear at the end of the command.
    ///        - Do not use shell pipes or redirections; jq must be the only command.
    ///        - Prefer compact filters that directly express the user's intent.
    ///        Output format:
    ///        - jq 'filter' file.json
    pub config: String,
}

/// Provider resolved after merging env + file.
#[derive(Debug, Clone)]
pub enum EffectiveAiConfig {
    OpenAI {
        api_key: String,
        base_url: String,
        api_mode: OpenAiApiMode,
        model: String,
        model_snapshot: Option<String>,
        reasoning_effort: Option<String>,
    },
    Azure {
        api_key: String,
        endpoint: String,
        deployment: String,
        api_version: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenAiApiMode {
    Responses,
    ChatCompletions,
}

impl OpenAiApiMode {
    fn parse(raw: Option<String>) -> Result<Self> {
        match raw {
            None => Ok(Self::Responses),
            Some(value) => match value.trim().to_ascii_lowercase().as_str() {
                "responses" => Ok(Self::Responses),
                "chat_completions" => Ok(Self::ChatCompletions),
                other => Err(anyhow!(
                    "Unsupported OpenAI API mode '{}'. Use 'responses' or 'chat_completions'.",
                    other
                )),
            },
        }
    }
}

thread_local! {
    static CONFIG_ROOT_OVERRIDE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

pub fn config_root_dir() -> PathBuf {
    if let Some(dir) = CONFIG_ROOT_OVERRIDE.with(|cell| cell.borrow().clone()) {
        return dir;
    }

    config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("sai")
}

pub fn find_global_config_path() -> PathBuf {
    config_root_dir().join("config.yaml")
}

#[cfg(test)]
pub struct ConfigDirOverrideGuard {
    prev: Option<PathBuf>,
}

#[cfg(test)]
pub fn set_config_dir_override_for_tests<P: Into<PathBuf>>(dir: P) -> ConfigDirOverrideGuard {
    let dir = dir.into();
    let prev = CONFIG_ROOT_OVERRIDE.with(|cell| {
        let mut guard = cell.borrow_mut();
        guard.replace(dir)
    });
    ConfigDirOverrideGuard { prev }
}

#[cfg(test)]
impl Drop for ConfigDirOverrideGuard {
    fn drop(&mut self) {
        let prev = self.prev.take();
        CONFIG_ROOT_OVERRIDE.with(|cell| {
            *cell.borrow_mut() = prev;
        });
    }
}

pub fn load_global_config(path: &Path) -> Result<GlobalConfig> {
    if !path.exists() {
        return Ok(GlobalConfig::default());
    }
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read global config file {}", path.display()))?;
    let cfg: GlobalConfig = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse global config YAML {}", path.display()))?;
    Ok(cfg)
}

pub fn load_prompt_config(path: &Path) -> Result<PromptConfig> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read prompt config file {}", path.display()))?;
    let cfg: PromptConfig = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse prompt config YAML {}", path.display()))?;
    Ok(cfg)
}

pub fn resolve_ai_config(global_ai: Option<AiConfig>) -> Result<EffectiveAiConfig> {
    let file_ai = global_ai.unwrap_or_default();

    let provider = env_or(file_ai.provider, "SAI_PROVIDER");

    let openai_api_key = env_or(file_ai.openai_api_key, "SAI_OPENAI_API_KEY");
    let openai_base_url = env_or(file_ai.openai_base_url, "SAI_OPENAI_BASE_URL");
    let openai_api_mode = env_or(file_ai.openai_api_mode, "SAI_OPENAI_API_MODE");
    let openai_model = env_or(file_ai.openai_model, "SAI_OPENAI_MODEL");
    let openai_model_snapshot = env_or(file_ai.openai_model_snapshot, "SAI_OPENAI_MODEL_SNAPSHOT");
    let openai_reasoning_effort = env_or(
        file_ai.openai_reasoning_effort,
        "SAI_OPENAI_REASONING_EFFORT",
    )
    .map(|value| value.trim().to_ascii_lowercase())
    .filter(|value| !value.is_empty());

    let azure_api_key = env_or(file_ai.azure_api_key, "SAI_AZURE_API_KEY");
    let azure_endpoint = env_or(file_ai.azure_endpoint, "SAI_AZURE_ENDPOINT");
    let azure_deployment = env_or(file_ai.azure_deployment, "SAI_AZURE_DEPLOYMENT");
    let azure_api_version = env_or(file_ai.azure_api_version, "SAI_AZURE_API_VERSION");

    let provider = if let Some(p) = provider {
        p.to_lowercase()
    } else if openai_api_key.is_some() {
        "openai".to_string()
    } else if azure_api_key.is_some() {
        "azure".to_string()
    } else {
        return Err(anyhow!(
            "No AI configuration found: set OpenAI or Azure info in config or environment"
        ));
    };

    match provider.as_str() {
        "openai" => {
            let api_key = openai_api_key.ok_or_else(|| {
                anyhow!("OpenAI selected but no OPENAI API key configured (SAI_OPENAI_API_KEY)")
            })?;
            let base_url =
                openai_base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            let api_mode = OpenAiApiMode::parse(openai_api_mode)?;
            let model = openai_model.ok_or_else(|| {
                anyhow!("OpenAI selected but no model configured (SAI_OPENAI_MODEL)")
            })?;
            Ok(EffectiveAiConfig::OpenAI {
                api_key,
                base_url,
                api_mode,
                model,
                model_snapshot: openai_model_snapshot,
                reasoning_effort: openai_reasoning_effort,
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

fn env_or(file_value: Option<String>, env_key: &str) -> Option<String> {
    if let Ok(v) = env::var(env_key)
        && !v.is_empty()
    {
        return Some(v);
    }
    file_value
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    // Protects environment-variable mutations so parallel tests don't race.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn clear_ai_env() {
        unsafe {
            env::remove_var("SAI_PROVIDER");
            env::remove_var("SAI_OPENAI_API_KEY");
            env::remove_var("SAI_OPENAI_BASE_URL");
            env::remove_var("SAI_OPENAI_API_MODE");
            env::remove_var("SAI_OPENAI_MODEL");
            env::remove_var("SAI_OPENAI_MODEL_SNAPSHOT");
            env::remove_var("SAI_OPENAI_REASONING_EFFORT");
            env::remove_var("SAI_AZURE_API_KEY");
            env::remove_var("SAI_AZURE_ENDPOINT");
            env::remove_var("SAI_AZURE_DEPLOYMENT");
            env::remove_var("SAI_AZURE_API_VERSION");
        }
    }

    #[test]
    fn env_override_takes_precedence() {
        let _guard = ENV_MUTEX.lock().unwrap();
        clear_ai_env();
        unsafe {
            env::set_var("SAI_PROVIDER", "azure");
        }
        let cfg = resolve_ai_config(None).unwrap_err();
        assert!(cfg
            .to_string()
            .contains("Azure selected but no AZURE API key configured"));
        unsafe {
            env::remove_var("SAI_PROVIDER");
        }
    }

    #[test]
    fn tool_config_deserializes_force_explain() {
        let yaml = r#"
name: rm
config: "dangerous"
force_explain: true
"#;
        let tool: ToolConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(tool.force_explain, Some(true));
    }

    #[test]
    fn tool_config_defaults_force_explain_to_none() {
        let yaml = r#"
name: ls
config: "safe"
"#;
        let tool: ToolConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(tool.force_explain, None);
    }

    #[test]
    fn tool_config_skips_serializing_none() {
        let tool = ToolConfig {
            name: "echo".to_string(),
            config: "test".to_string(),
            force_explain: None,
        };
        let yaml = serde_yaml::to_string(&tool).unwrap();
        assert!(!yaml.contains("force_explain"));
    }

    #[test]
    fn tool_config_serializes_force_explain_when_present() {
        let tool = ToolConfig {
            name: "rm".to_string(),
            config: "dangerous".to_string(),
            force_explain: Some(true),
        };
        let yaml = serde_yaml::to_string(&tool).unwrap();
        assert!(yaml.contains("force_explain: true"));
    }

    #[test]
    fn openai_api_mode_defaults_to_responses() {
        let _guard = ENV_MUTEX.lock().unwrap();
        clear_ai_env();
        let cfg = resolve_ai_config(Some(AiConfig {
            provider: Some("openai".to_string()),
            openai_api_key: Some("test-key".to_string()),
            openai_model: Some("gpt-5.4-mini".to_string()),
            ..AiConfig::default()
        }))
        .unwrap();

        match cfg {
            EffectiveAiConfig::OpenAI { api_mode, .. } => {
                assert_eq!(api_mode, OpenAiApiMode::Responses);
            }
            _ => panic!("expected openai config"),
        }
    }

    #[test]
    fn openai_api_mode_accepts_chat_completions() {
        let _guard = ENV_MUTEX.lock().unwrap();
        clear_ai_env();
        let cfg = resolve_ai_config(Some(AiConfig {
            provider: Some("openai".to_string()),
            openai_api_key: Some("test-key".to_string()),
            openai_api_mode: Some("chat_completions".to_string()),
            openai_model: Some("gpt-5.4-mini".to_string()),
            ..AiConfig::default()
        }))
        .unwrap();

        match cfg {
            EffectiveAiConfig::OpenAI { api_mode, .. } => {
                assert_eq!(api_mode, OpenAiApiMode::ChatCompletions);
            }
            _ => panic!("expected openai config"),
        }
    }

    #[test]
    fn openai_snapshot_and_reasoning_effort_are_resolved() {
        let _guard = ENV_MUTEX.lock().unwrap();
        clear_ai_env();
        let cfg = resolve_ai_config(Some(AiConfig {
            provider: Some("openai".to_string()),
            openai_api_key: Some("test-key".to_string()),
            openai_model: Some("gpt-5.4-mini".to_string()),
            openai_model_snapshot: Some("gpt-5.4-mini-2026-03-17".to_string()),
            openai_reasoning_effort: Some("Medium".to_string()),
            ..AiConfig::default()
        }))
        .unwrap();

        match cfg {
            EffectiveAiConfig::OpenAI {
                model,
                model_snapshot,
                reasoning_effort,
                ..
            } => {
                assert_eq!(model, "gpt-5.4-mini");
                assert_eq!(model_snapshot.as_deref(), Some("gpt-5.4-mini-2026-03-17"));
                assert_eq!(reasoning_effort.as_deref(), Some("medium"));
            }
            _ => panic!("expected openai config"),
        }
    }

    #[test]
    fn invalid_openai_api_mode_errors() {
        let _guard = ENV_MUTEX.lock().unwrap();
        clear_ai_env();
        let err = resolve_ai_config(Some(AiConfig {
            provider: Some("openai".to_string()),
            openai_api_key: Some("test-key".to_string()),
            openai_api_mode: Some("legacy".to_string()),
            openai_model: Some("gpt-5.4-mini".to_string()),
            ..AiConfig::default()
        }))
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("Unsupported OpenAI API mode 'legacy'"));
    }
}
