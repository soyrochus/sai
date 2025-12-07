use crate::config::EffectiveAiConfig;
use crate::scope::build_scope_dot_listing;
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

pub trait ChatClient {
    fn respond(
        &self,
        ai: &EffectiveAiConfig,
        system_prompt: &str,
        user_prompt: &str,
        temperature: f32,
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
            let scope_content = if scope == "." {
                let listing = build_scope_dot_listing()?;
                format!(
                    "Scope: current directory.\nHere is a non-recursive listing of the working directory:\n{}",
                    listing
                )
            } else {
                format!(
                    "Focus your command on files or paths matching this scope:\n{}",
                    scope
                )
            };

            messages.push(Message {
                role: "user".to_string(),
                content: scope_content,
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

        let content = self.chat(ai, messages, 0.0)?;
        extract_first_line_from_text(&content)
    }
}

impl ChatClient for HttpCommandGenerator {
    fn respond(
        &self,
        ai: &EffectiveAiConfig,
        system_prompt: &str,
        user_prompt: &str,
        temperature: f32,
    ) -> Result<String> {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            Message {
                role: "user".to_string(),
                content: user_prompt.to_string(),
            },
        ];

        self.chat(ai, messages, temperature)
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

impl HttpCommandGenerator {
    fn chat(
        &self,
        ai: &EffectiveAiConfig,
        messages: Vec<Message>,
        temperature: f32,
    ) -> Result<String> {
        let resp = match ai {
            EffectiveAiConfig::OpenAI {
                api_key,
                base_url,
                model,
            } => {
                let req = ChatRequest {
                    model: Some(model.clone()),
                    messages,
                    temperature,
                };
                let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
                self.client
                    .post(&url)
                    .bearer_auth(api_key)
                    .json(&req)
                    .send()
                    .context("HTTP error calling OpenAI")?
                    .error_for_status()
                    .context("Non-success status from OpenAI")?
                    .json()
                    .context("Failed to parse OpenAI response JSON")?
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
                    temperature,
                };
                let url = format!(
                    "{}/openai/deployments/{}/chat/completions?api-version={}",
                    endpoint.trim_end_matches('/'),
                    deployment,
                    api_version
                );
                self.client
                    .post(&url)
                    .header("api-key", api_key)
                    .json(&req)
                    .send()
                    .context("HTTP error calling Azure OpenAI")?
                    .error_for_status()
                    .context("Non-success status from Azure OpenAI")?
                    .json()
                    .context("Failed to parse Azure OpenAI response JSON")?
            }
        };

        extract_content(&resp)
    }
}

fn extract_content(resp: &ChatResponse) -> Result<String> {
    let content = resp
        .choices
        .first()
        .ok_or_else(|| anyhow!("No choices in LLM response"))?
        .message
        .content
        .trim()
        .to_string();

    Ok(strip_code_fences(&content))
}

fn extract_first_line_from_text(text: &str) -> Result<String> {
    let first_line = text
        .lines()
        .next()
        .ok_or_else(|| anyhow!("Empty content from LLM"))?
        .trim()
        .to_string();

    if first_line.is_empty() {
        return Err(anyhow!("LLM returned an empty command line"));
    }

    Ok(first_line)
}

fn strip_code_fences(text: &str) -> String {
    if !text.trim_start().starts_with("```") {
        return text.trim().to_string();
    }

    let mut cleaned = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            continue;
        }
        cleaned.push_str(line);
        cleaned.push('\n');
    }
    cleaned.trim().to_string()
}
