use crate::config::{EffectiveAiConfig, OpenAiApiMode};
use crate::scope::build_scope_dot_listing;
use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use reqwest::blocking::Response;
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
        let mut messages = vec![TextMessage {
            role: "user".to_string(),
            content: nl_prompt.to_string(),
        }];

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

            messages.push(TextMessage {
                role: "user".to_string(),
                content: scope_content,
            });
        }

        if let Some(peek) = peek_text {
            messages.push(TextMessage {
                role: "user".to_string(),
                content: format!(
                    "Here is a sample of the data the tools will operate on. \
                     It may be truncated and is provided only to infer structure and field names, \
                     not to be hard-coded:\n\n{}",
                    peek
                ),
            });
        }

        let content = self.chat(ai, system_prompt, messages, 0.0)?;
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
        let messages = vec![TextMessage {
            role: "user".to_string(),
            content: user_prompt.to_string(),
        }];

        self.chat(ai, system_prompt, messages, temperature)
    }
}

#[derive(Serialize)]
struct ChatRequest {
    model: Option<String>,
    messages: Vec<TextMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
}

#[derive(Serialize, Clone)]
struct TextMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ResponsesApiRequest {
    model: String,
    input: Vec<TextMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<ResponsesReasoningConfig>,
}

#[derive(Serialize)]
struct ResponsesReasoningConfig {
    effort: String,
}

struct OpenAiRequestConfig<'a> {
    api_key: &'a str,
    base_url: &'a str,
    model: &'a str,
    model_snapshot: Option<&'a str>,
    system_prompt: &'a str,
    reasoning_effort: Option<&'a str>,
}

struct AzureRequestConfig<'a> {
    api_key: &'a str,
    endpoint: &'a str,
    deployment: &'a str,
    api_version: &'a str,
    system_prompt: &'a str,
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

#[derive(Deserialize)]
struct ResponsesApiResponse {
    #[serde(default)]
    output_text: Option<String>,
    #[serde(default)]
    output: Vec<ResponsesOutputItem>,
}

#[derive(Deserialize)]
struct ResponsesOutputItem {
    #[serde(rename = "type")]
    item_type: String,
    #[serde(default)]
    content: Vec<ResponsesOutputContent>,
}

#[derive(Deserialize)]
struct ResponsesOutputContent {
    #[serde(rename = "type")]
    content_type: String,
    #[serde(default)]
    text: Option<String>,
}

impl HttpCommandGenerator {
    fn chat(
        &self,
        ai: &EffectiveAiConfig,
        system_prompt: &str,
        messages: Vec<TextMessage>,
        temperature: f32,
    ) -> Result<String> {
        match ai {
            EffectiveAiConfig::OpenAI {
                api_key,
                base_url,
                api_mode,
                model,
                model_snapshot,
                reasoning_effort,
            } => match api_mode {
                OpenAiApiMode::Responses => self.call_openai_responses(
                    OpenAiRequestConfig {
                        api_key,
                        base_url,
                        model,
                        model_snapshot: model_snapshot.as_deref(),
                        system_prompt,
                        reasoning_effort: reasoning_effort.as_deref(),
                    },
                    &messages,
                ),
                OpenAiApiMode::ChatCompletions => self.call_openai_chat_completions(
                    OpenAiRequestConfig {
                        api_key,
                        base_url,
                        model,
                        model_snapshot: model_snapshot.as_deref(),
                        system_prompt,
                        reasoning_effort: reasoning_effort.as_deref(),
                    },
                    messages,
                    temperature,
                ),
            },
            EffectiveAiConfig::Azure {
                api_key,
                endpoint,
                deployment,
                api_version,
            } => self.call_azure_chat_completions(
                AzureRequestConfig {
                    api_key,
                    endpoint,
                    deployment,
                    api_version,
                    system_prompt,
                },
                messages,
                temperature,
            ),
        }
    }

    fn call_openai_responses(
        &self,
        config: OpenAiRequestConfig<'_>,
        messages: &[TextMessage],
    ) -> Result<String> {
        let mut input = Vec::with_capacity(messages.len() + 1);
        input.push(TextMessage {
            role: "developer".to_string(),
            content: config.system_prompt.to_string(),
        });
        input.extend_from_slice(messages);

        let req = ResponsesApiRequest {
            model: request_model_name(config.model, config.model_snapshot).to_string(),
            input,
            reasoning: config
                .reasoning_effort
                .map(|effort| ResponsesReasoningConfig {
                    effort: effort.to_string(),
                }),
        };
        let url = format!("{}/responses", config.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .post(&url)
            .bearer_auth(config.api_key)
            .json(&req)
            .send()
            .context("HTTP error calling OpenAI Responses API")?;
        let resp: ResponsesApiResponse =
            parse_json_response(resp, "OpenAI Responses API", "OpenAI Responses API JSON")?;

        extract_responses_content(&resp)
    }

    fn call_openai_chat_completions(
        &self,
        config: OpenAiRequestConfig<'_>,
        messages: Vec<TextMessage>,
        temperature: f32,
    ) -> Result<String> {
        let mut all_messages = Vec::with_capacity(messages.len() + 1);
        all_messages.push(TextMessage {
            role: "developer".to_string(),
            content: config.system_prompt.to_string(),
        });
        all_messages.extend(messages);

        let req = ChatRequest {
            model: Some(request_model_name(config.model, config.model_snapshot).to_string()),
            messages: all_messages,
            temperature: Some(temperature),
            reasoning_effort: config.reasoning_effort.map(str::to_string),
        };
        let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .post(&url)
            .bearer_auth(config.api_key)
            .json(&req)
            .send()
            .context("HTTP error calling OpenAI Chat Completions")?;
        let resp: ChatResponse = parse_json_response(
            resp,
            "OpenAI Chat Completions",
            "OpenAI Chat Completions JSON",
        )?;

        extract_chat_content(&resp)
    }

    fn call_azure_chat_completions(
        &self,
        config: AzureRequestConfig<'_>,
        messages: Vec<TextMessage>,
        temperature: f32,
    ) -> Result<String> {
        let mut all_messages = Vec::with_capacity(messages.len() + 1);
        all_messages.push(TextMessage {
            role: "system".to_string(),
            content: config.system_prompt.to_string(),
        });
        all_messages.extend(messages);

        let req = ChatRequest {
            model: None,
            messages: all_messages,
            temperature: Some(temperature),
            reasoning_effort: None,
        };
        let url = format!(
            "{}/openai/deployments/{}/chat/completions?api-version={}",
            config.endpoint.trim_end_matches('/'),
            config.deployment,
            config.api_version
        );
        let resp = self
            .client
            .post(&url)
            .header("api-key", config.api_key)
            .json(&req)
            .send()
            .context("HTTP error calling Azure OpenAI")?;
        let resp: ChatResponse =
            parse_json_response(resp, "Azure OpenAI", "Azure OpenAI response JSON")?;

        extract_chat_content(&resp)
    }
}

fn parse_json_response<T>(resp: Response, service_name: &str, body_name: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let status = resp.status();
    let body = resp
        .text()
        .with_context(|| format!("Failed to read {} response body", service_name))?;

    if !status.is_success() {
        let compact_body = body.trim();
        if compact_body.is_empty() {
            return Err(anyhow!(
                "Non-success status from {}: HTTP {}",
                service_name,
                status
            ));
        }

        return Err(anyhow!(
            "Non-success status from {}: HTTP {}. Response body: {}",
            service_name,
            status,
            compact_body
        ));
    }

    serde_json::from_str(&body).with_context(|| format!("Failed to parse {}", body_name))
}

fn request_model_name<'a>(model: &'a str, model_snapshot: Option<&'a str>) -> &'a str {
    model_snapshot.unwrap_or(model)
}

fn extract_chat_content(resp: &ChatResponse) -> Result<String> {
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

fn extract_responses_content(resp: &ResponsesApiResponse) -> Result<String> {
    if let Some(output_text) = resp.output_text.as_deref() {
        let text = output_text.trim();
        if !text.is_empty() {
            return Ok(strip_code_fences(text));
        }
    }

    let text = resp
        .output
        .iter()
        .filter(|item| item.item_type == "message")
        .flat_map(|item| item.content.iter())
        .filter(|content| content.content_type == "output_text")
        .filter_map(|content| content.text.as_deref())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    if text.is_empty() {
        return Err(anyhow!(
            "No output_text content found in OpenAI Responses API response"
        ));
    }

    Ok(strip_code_fences(&text))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn responses_content_extracts_top_level_output_text() {
        let resp = ResponsesApiResponse {
            output_text: Some("rg 'needle' src".to_string()),
            output: Vec::new(),
        };

        let text = extract_responses_content(&resp).unwrap();
        assert_eq!(text, "rg 'needle' src");
    }

    #[test]
    fn responses_content_extracts_message_output_text() {
        let resp = ResponsesApiResponse {
            output_text: None,
            output: vec![ResponsesOutputItem {
                item_type: "message".to_string(),
                content: vec![ResponsesOutputContent {
                    content_type: "output_text".to_string(),
                    text: Some("```text\nfind src -name '*.rs'\n```".to_string()),
                }],
            }],
        };

        let text = extract_responses_content(&resp).unwrap();
        assert_eq!(text, "find src -name '*.rs'");
    }

    #[test]
    fn request_model_prefers_snapshot() {
        assert_eq!(
            request_model_name("gpt-5.4-mini", Some("gpt-5.4-mini-2026-03-17")),
            "gpt-5.4-mini-2026-03-17"
        );
        assert_eq!(request_model_name("gpt-5.4-mini", None), "gpt-5.4-mini");
    }
}
