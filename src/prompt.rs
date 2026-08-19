use crate::config::{PromptConfig, ToolConfig};
use crate::safety_mode::SafetyMode;
use anyhow::{anyhow, Result};

pub fn build_system_prompt(
    prompt_cfg: &PromptConfig,
    mode: SafetyMode,
) -> Result<(String, Vec<String>)> {
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

    // Under an unrestricted run the tool list stops being a ceiling. The tool
    // descriptions still carry the user's domain knowledge (how they want jq
    // invoked, say), so they are kept as guidance rather than discarded —
    // dropping them would make unrestricted output worse, not freer.
    let tools_listing = if mode.lifts_tool_restriction() {
        let mut listing = String::from(
            "You are not restricted to a fixed set of tools. Choose whatever \
             standard command-line tools best accomplish the request, and combine \
             them with pipes, redirection or chaining when that is the clearest \
             solution.\n\nThe following tools are described below as guidance, not \
             as a limit:\n",
        );
        for name in &allowed_names {
            listing.push_str(&format!("- {}\n", name));
        }
        listing
    } else {
        let mut listing = String::from("You may ONLY use the following tools:\n");
        for name in &allowed_names {
            listing.push_str(&format!("- {}\n", name));
        }
        listing
    };

    let mut system_parts = Vec::new();
    if !meta_prompt.trim().is_empty() {
        system_parts.push(meta_prompt.trim().to_string());
    }
    system_parts.push(tools_listing);
    system_parts.push(format!("\nTool details:\n\n{}", tool_texts.join("\n\n")));

    let full_prompt = system_parts.join("\n\n").trim().to_string();
    Ok((full_prompt, allowed_names))
}

/// Checks if the generated command uses a tool that requires forced explain mode.
/// Returns true if the first token of the command matches a tool with force_explain set to true.
pub fn should_force_explain(tools: &[ToolConfig], command: &str) -> bool {
    let first_token = command.split_whitespace().next().unwrap_or("");

    tools
        .iter()
        .any(|t| t.name == first_token && t.force_explain == Some(true))
}

#[cfg(test)]
mod system_prompt_tests {
    use super::*;
    use crate::config::ToolConfig;

    fn cfg() -> PromptConfig {
        PromptConfig {
            meta_prompt: Some("You translate requests into commands.".to_string()),
            tools: vec![
                ToolConfig {
                    name: "jq".to_string(),
                    force_explain: None,
                    config: "Tool: jq\nRole: filter JSON.".to_string(),
                },
                ToolConfig {
                    name: "find".to_string(),
                    force_explain: None,
                    config: "Tool: find\nRole: locate files.".to_string(),
                },
            ],
        }
    }

    #[test]
    fn default_mode_confines_the_model_to_configured_tools() {
        let (prompt, tools) = build_system_prompt(&cfg(), SafetyMode::Default).unwrap();
        assert!(prompt.contains("You may ONLY use the following tools:"));
        assert_eq!(tools, vec!["jq".to_string(), "find".to_string()]);
    }

    #[test]
    fn unsafe_mode_still_confines_the_model() {
        // --unsafe relaxes operators, never the tool set.
        let (prompt, _) = build_system_prompt(&cfg(), SafetyMode::Unsafe).unwrap();
        assert!(prompt.contains("You may ONLY use the following tools:"));
    }

    #[test]
    fn unrestricted_mode_does_not_confine_the_model() {
        let (prompt, _) = build_system_prompt(&cfg(), SafetyMode::Unrestricted).unwrap();
        assert!(
            !prompt.contains("ONLY"),
            "an unrestricted prompt must not impose an exclusive tool set, or the flag is inert"
        );
        assert!(prompt.contains("not restricted to a fixed set of tools"));
        assert!(prompt.contains("guidance, not"));
    }

    #[test]
    fn tool_descriptions_survive_in_both_modes() {
        // The descriptions carry domain knowledge worth keeping either way.
        for mode in [SafetyMode::Default, SafetyMode::Unrestricted] {
            let (prompt, _) = build_system_prompt(&cfg(), mode).unwrap();
            assert!(prompt.contains("Role: filter JSON."), "mode {:?}", mode);
            assert!(prompt.contains("Role: locate files."), "mode {:?}", mode);
            assert!(prompt.contains("You translate requests into commands."));
        }
    }

    #[test]
    fn an_empty_tool_list_is_still_rejected() {
        let empty = PromptConfig {
            meta_prompt: None,
            tools: Vec::new(),
        };
        assert!(build_system_prompt(&empty, SafetyMode::Unrestricted).is_err());
    }
}
