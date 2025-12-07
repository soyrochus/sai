use crate::config::PromptConfig;
use anyhow::{anyhow, Result};

pub fn build_system_prompt(prompt_cfg: &PromptConfig) -> Result<(String, Vec<String>)> {
    if prompt_cfg.tools.is_empty() {
        return Err(anyhow!(
            "Prompt config must define at least one tool under 'tools:'"
        ));
    }

    let meta_prompt = prompt_cfg.meta_prompt.clone().unwrap_or_default();

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
