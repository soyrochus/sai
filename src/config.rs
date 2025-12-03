use anyhow::{anyhow, Context, Result};
use dirs::config_dir;
use serde::{Deserialize, Serialize};
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
    pub openai_model: Option<String>,

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
    pub config: String,
}

/// Provider resolved after merging env + file.
#[derive(Debug, Clone)]
pub enum EffectiveAiConfig {
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

pub fn find_global_config_path() -> PathBuf {
    let base = config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("sai").join("config.yaml")
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
    let openai_model = env_or(file_ai.openai_model, "SAI_OPENAI_MODEL");

    let azure_api_key = env_or(file_ai.azure_api_key, "SAI_AZURE_API_KEY");
    let azure_endpoint = env_or(file_ai.azure_endpoint, "SAI_AZURE_ENDPOINT");
    let azure_deployment = env_or(file_ai.azure_deployment, "SAI_AZURE_DEPLOYMENT");
    let azure_api_version = env_or(file_ai.azure_api_version, "SAI_AZURE_API_VERSION");

    let provider = if let Some(p) = provider {
        p.to_lowercase()
    } else {
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

fn env_or(file_value: Option<String>, env_key: &str) -> Option<String> {
    if let Ok(v) = env::var(env_key) {
        if !v.is_empty() {
            return Some(v);
        }
    }
    file_value
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn env_override_takes_precedence() {
        env::set_var("SAI_PROVIDER", "azure");
        let cfg = resolve_ai_config(None).unwrap_err();
        assert!(cfg
            .to_string()
            .contains("Azure selected but no AZURE API key configured"));
        env::remove_var("SAI_PROVIDER");
    }
}
