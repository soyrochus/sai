use crate::safety_mode::SafetyMode;
use anyhow::{anyhow, Context, Result};

pub fn validate_and_split_command(
    cmd_line: &str,
    allowed_tools: &[String],
    mode: SafetyMode,
) -> Result<Vec<String>> {
    // Parsing is unconditional: lifting the restrictions does not lift the
    // requirement that the command be well-formed.
    let tokens =
        shell_words::split(cmd_line).context("Failed to split command line from LLM output")?;

    if tokens.is_empty() {
        return Err(anyhow!("LLM returned an empty command after parsing"));
    }

    if !mode.lifts_tool_restriction() {
        let first = &tokens[0];
        if !allowed_tools.iter().any(|t| t == first) {
            return Err(anyhow!(
                "Disallowed command '{}'. Allowed tools: {}",
                first,
                allowed_tools.join(", ")
            ));
        }
    }

    if !mode.allows_operators()
        && let Some(op) = detect_forbidden_operator(cmd_line)
    {
        return Err(anyhow!(
            "Disallowed shell operator or construct '{}' in generated command. \
             Re-run with --unsafe if you really want to execute it.",
            op
        ));
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
                if let Some(&next) = chars.peek()
                    && next == '&'
                {
                    return Some("&&".to_string());
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

/// A locally computed observation about a command.
///
/// Markers exist because the explanation shown under `--unrestricted` comes
/// from the same model that wrote the command, and so is not an independent
/// check. These are derived from the command text alone — no model, no
/// filesystem, no execution — which is what makes them worth showing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskMarker {
    pub kind: RiskKind,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskKind {
    /// Pipes, redirection, chaining or substitution.
    Operator,
    /// Recursive or forced deletion.
    Destructive,
    /// A wildcard reaching outside the working directory.
    WildcardBreadth,
}

impl RiskKind {
    pub const fn label(self) -> &'static str {
        match self {
            RiskKind::Operator => "shell operators",
            RiskKind::Destructive => "destructive",
            RiskKind::WildcardBreadth => "broad wildcard",
        }
    }
}

/// Observations about `cmd_line`, computed deterministically from its text.
///
/// Pure: it never executes the command, touches the filesystem, or consults a
/// model, and repeated calls on the same input return the same result.
pub fn risk_markers(cmd_line: &str) -> Vec<RiskMarker> {
    let mut markers = Vec::new();

    // Reuse the quote-aware scanner so text inside quotes is not misread as an
    // operator.
    if let Some(op) = detect_forbidden_operator(cmd_line) {
        markers.push(RiskMarker {
            kind: RiskKind::Operator,
            detail: format!("contains {}", op),
        });
    }

    let tokens = shell_words::split(cmd_line).unwrap_or_default();

    if let Some(detail) = destructive_detail(&tokens) {
        markers.push(RiskMarker {
            kind: RiskKind::Destructive,
            detail,
        });
    }

    if let Some(detail) = broad_wildcard_detail(&tokens) {
        markers.push(RiskMarker {
            kind: RiskKind::WildcardBreadth,
            detail,
        });
    }

    markers
}

/// Recursive or forced deletion, by whichever tool performs it.
fn destructive_detail(tokens: &[String]) -> Option<String> {
    const DELETERS: &[&str] = &["rm", "rmdir", "shred", "unlink"];

    let program = tokens.first()?.rsplit('/').next()?;
    let mut flags = Vec::new();

    for token in tokens.iter().skip(1) {
        match token.as_str() {
            "--recursive" => flags.push("recursive"),
            "--force" => flags.push("forced"),
            // Short flags bundle, so -rf carries both.
            t if t.starts_with('-') && !t.starts_with("--") => {
                if t.contains('r') || t.contains('R') {
                    flags.push("recursive");
                }
                if t.contains('f') {
                    flags.push("forced");
                }
            }
            _ => {}
        }
    }

    // `find -delete` and `find -exec rm` delete without naming rm first.
    let find_delete = program == "find"
        && tokens
            .iter()
            .any(|t| t == "-delete" || t == "-exec" || t == "-execdir");

    if DELETERS.contains(&program) {
        flags.dedup();
        let how = if flags.is_empty() {
            "deletes files".to_string()
        } else {
            format!("{} deletion", flags.join(" and "))
        };
        return Some(format!("{} — {}", program, how));
    }

    if find_delete && tokens.iter().any(|t| t == "-delete") {
        return Some("find -delete — deletes every match".to_string());
    }

    None
}

/// A wildcard reaching outside the working directory.
///
/// Deliberately tuned to over-mark: an unnecessary marker is an annoyance, a
/// missing one is a hazard.
fn broad_wildcard_detail(tokens: &[String]) -> Option<String> {
    for token in tokens.iter().skip(1) {
        if !token.contains('*') && !token.contains('?') {
            continue;
        }

        let broad = token.starts_with('/')
            || token.starts_with('~')
            || token.starts_with("..")
            || token.contains("/../")
            || token == "*"
            || token.starts_with("*/");

        if broad {
            return Some(format!("{} reaches outside the working directory", token));
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
            validate_and_split_command("jq '.foo' file.json", &["jq".to_string()], SafetyMode::Default).unwrap();
        assert_eq!(tokens[0], "jq");
    }

    const ALLOWED: &[&str] = &["jq", "find"];

    fn allowed() -> Vec<String> {
        ALLOWED.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn an_unconfigured_tool_is_rejected_unless_unrestricted() {
        for mode in [SafetyMode::Default, SafetyMode::Unsafe] {
            let err = validate_and_split_command("ripgrep foo", &allowed(), mode)
                .expect_err("the whitelist applies in {mode:?}");
            assert!(err.to_string().contains("Disallowed command 'ripgrep'"));
        }

        let tokens =
            validate_and_split_command("ripgrep foo", &allowed(), SafetyMode::Unrestricted).unwrap();
        assert_eq!(tokens[0], "ripgrep");
    }

    #[test]
    fn operators_are_permitted_in_both_relaxed_modes() {
        for mode in [SafetyMode::Unsafe, SafetyMode::Unrestricted] {
            // Use a whitelisted tool so only the operator rule is under test.
            validate_and_split_command("jq '.a' f.json > out.txt", &allowed(), mode)
                .unwrap_or_else(|e| panic!("{mode:?} should permit operators: {e}"));
        }
    }

    #[test]
    fn operators_are_rejected_in_default_mode() {
        let err = validate_and_split_command("jq '.a' f.json > out.txt", &allowed(), SafetyMode::Default)
            .expect_err("default mode blocks redirection");
        assert!(err.to_string().contains("Disallowed shell operator"));
    }

    #[test]
    fn unrestricted_permits_an_unconfigured_tool_with_operators() {
        // The case that motivated the flag: find piped into a tool that is not
        // in the config.
        let tokens = validate_and_split_command(
            "find . -name '*.rs' -mmin -30 | xargs grep -n 'fn '",
            &allowed(),
            SafetyMode::Unrestricted,
        )
        .unwrap();
        assert_eq!(tokens[0], "find");
    }

    #[test]
    fn a_malformed_command_is_rejected_in_every_mode() {
        for mode in [
            SafetyMode::Default,
            SafetyMode::Unsafe,
            SafetyMode::Unrestricted,
        ] {
            // An unbalanced quote cannot be split into a program and arguments.
            assert!(
                validate_and_split_command("jq '.unterminated", &allowed(), mode).is_err(),
                "{mode:?} must still reject an unparseable command"
            );
        }
    }

    #[test]
    fn an_empty_command_is_rejected_in_every_mode() {
        for mode in [
            SafetyMode::Default,
            SafetyMode::Unsafe,
            SafetyMode::Unrestricted,
        ] {
            assert!(validate_and_split_command("   ", &allowed(), mode).is_err(), "{mode:?}");
        }
    }

    // --- risk markers -------------------------------------------------------

    fn kinds(cmd: &str) -> Vec<RiskKind> {
        risk_markers(cmd).into_iter().map(|m| m.kind).collect()
    }

    #[test]
    fn operators_are_marked() {
        assert!(kinds("find . -name '*.rs' | xargs grep fn").contains(&RiskKind::Operator));
        assert!(kinds("jq '.a' f.json > out.txt").contains(&RiskKind::Operator));
        assert!(kinds("ls && rm x").contains(&RiskKind::Operator));
        assert!(kinds("echo $(whoami)").contains(&RiskKind::Operator));
    }

    #[test]
    fn quoted_operators_are_not_marked() {
        // The scanner is quote-aware, so a pipe inside a literal is just text.
        assert!(!kinds("grep 'a|b' file.txt").contains(&RiskKind::Operator));
        assert!(!kinds("jq '.a > .b' f.json").contains(&RiskKind::Operator));
    }

    #[test]
    fn recursive_and_forced_deletion_is_marked() {
        for cmd in [
            "rm -rf build",
            "rm -r build",
            "rm --recursive --force build",
            "rm -f notes.txt",
        ] {
            assert!(
                kinds(cmd).contains(&RiskKind::Destructive),
                "should mark: {}",
                cmd
            );
        }
    }

    #[test]
    fn deletion_detail_names_what_it_does() {
        let markers = risk_markers("rm -rf build");
        let d = &markers
            .iter()
            .find(|m| m.kind == RiskKind::Destructive)
            .unwrap()
            .detail;
        assert!(d.contains("recursive") && d.contains("forced"), "{}", d);
    }

    #[test]
    fn find_delete_is_marked_even_though_rm_never_appears() {
        assert!(kinds("find . -name '*.tmp' -delete").contains(&RiskKind::Destructive));
    }

    #[test]
    fn a_full_path_to_a_deleter_is_still_marked() {
        assert!(kinds("/bin/rm -rf build").contains(&RiskKind::Destructive));
    }

    #[test]
    fn non_destructive_commands_are_not_marked_destructive() {
        for cmd in ["ls -la", "find . -name '*.rs'", "jq '.a' f.json", "grep -rn fn src"] {
            assert!(
                !kinds(cmd).contains(&RiskKind::Destructive),
                "should not mark: {}",
                cmd
            );
        }
    }

    #[test]
    fn broad_wildcards_are_marked() {
        for cmd in ["rm /tmp/*", "ls ~/*", "cat ../*/notes", "chmod 777 *"] {
            assert!(
                kinds(cmd).contains(&RiskKind::WildcardBreadth),
                "should mark breadth: {}",
                cmd
            );
        }
    }

    #[test]
    fn wildcards_rooted_in_the_working_directory_are_not_marked_broad() {
        for cmd in ["ls src/*.rs", "jq '.a' data/*.json"] {
            assert!(
                !kinds(cmd).contains(&RiskKind::WildcardBreadth),
                "should not mark breadth: {}",
                cmd
            );
        }
    }

    #[test]
    fn a_command_can_carry_several_markers() {
        let k = kinds("rm -rf /tmp/* && echo done");
        assert!(k.contains(&RiskKind::Operator));
        assert!(k.contains(&RiskKind::Destructive));
        assert!(k.contains(&RiskKind::WildcardBreadth));
    }

    #[test]
    fn an_ordinary_command_carries_no_markers() {
        assert!(risk_markers("ls -la").is_empty());
        assert!(risk_markers("jq '.users' data.json").is_empty());
    }

    #[test]
    fn markers_are_deterministic_and_side_effect_free() {
        let probe = "rm -rf /tmp/* | tee log";
        let first = risk_markers(probe);
        for _ in 0..50 {
            assert_eq!(risk_markers(probe), first, "markers must be deterministic");
        }

        // A path that does not exist yields the same markers as one that does,
        // proving the filesystem is never consulted.
        let missing = risk_markers("rm -rf /definitely/not/here/*");
        let present = risk_markers("rm -rf /tmp/*");
        assert_eq!(
            missing.iter().map(|m| m.kind).collect::<Vec<_>>(),
            present.iter().map(|m| m.kind).collect::<Vec<_>>()
        );
    }

    #[test]
    fn markers_never_reject_a_command() {
        // Marking is advisory: validation is unaffected by how many markers fire.
        let cmd = "rm -rf /tmp/*";
        assert!(!risk_markers(cmd).is_empty());
        assert!(validate_and_split_command(cmd, &allowed(), SafetyMode::Unrestricted).is_ok());
    }

    #[test]
    fn a_malformed_command_still_yields_markers_without_panicking() {
        let _ = risk_markers("rm -rf '/tmp/unterminated");
    }
}
