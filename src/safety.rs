use anyhow::{anyhow, Context, Result};
use shell_words;

pub fn validate_and_split_command(
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

pub fn detect_forbidden_operator(cmd_line: &str) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_pipe_even_without_spaces() {
        assert_eq!(detect_forbidden_operator("ls|wc"), Some("|".to_string()));
    }

    #[test]
    fn allows_safe_command() {
        let tokens =
            validate_and_split_command("jq '.foo' file.json", &["jq".to_string()], false).unwrap();
        assert_eq!(tokens[0], "jq");
    }
}
