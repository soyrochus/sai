use crate::config::EffectiveAiConfig;
use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

pub trait CommandGenerator {
    fn generate(
        &self,
        ai: &EffectiveAiConfig,
        system_prompt: &str,
        nl_prompt: &str,
        scope_hint: Option<&str>,
        peek_text: Option<&str>,
    ) -> Result<String>;
}

pub struct HttpCommandGenerator {
    client: Client,
}

impl HttpCommandGenerator {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl Default for HttpCommandGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandGenerator for HttpCommandGenerator {
    fn generate(
        &self,
        ai: &EffectiveAiConfig,
        system_prompt: &str,
        nl_prompt: &str,
        scope_hint: Option<&str>,
        peek_text: Option<&str>,
    ) -> Result<String> {
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
                    "Focus your command on files or paths matching this scope:\n{}",
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
                let resp: ChatResponse = self
                    .client
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
                    model: None,
                    messages,
                    temperature: 0.0,
                };
                let url = format!(
                    "{}/openai/deployments/{}/chat/completions?api-version={}",
                    endpoint.trim_end_matches('/'),
                    deployment,
                    api_version
                );
                let resp: ChatResponse = self
                    .client
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
}

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

fn extract_first_line(resp: &ChatResponse) -> Result<String> {
    let content = resp
        .choices
        .get(0)
        .ok_or_else(|| anyhow!("No choices in LLM response"))?
        .message
        .content
        .trim()
        .to_string();

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
