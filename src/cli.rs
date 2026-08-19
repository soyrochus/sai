use crate::help;
use clap::Parser;

/// Command-line interface definition for sai.
#[derive(Parser, Debug, Clone, Default)]
#[command(
    name = "sai",
    version,
    about = help::CLI_ABOUT,
    long_about = help::CLI_LONG_ABOUT,
    override_usage = help::CLI_USAGE,
    after_help = help::CLI_AFTER_HELP
)]
pub struct Cli {
    /// Initialize the default config file with placeholder values
    #[arg(long)]
    pub init: bool,

    /// Create a per-call prompt config template for the specified command and optional path
    #[arg(long, value_names = ["COMMAND", "PATH"], num_args = 1..=2)]
    pub create_prompt: Option<Vec<String>>,

    /// Merge tools from a prompt config file into the global default prompt
    #[arg(long, value_name = "PATH")]
    pub add_prompt: Option<String>,

    /// List the configured tools (global config and optional prompt file) and exit
    #[arg(long = "list-tools")]
    pub list_tools: bool,

    /// Analyze the latest SAI invocation and explain what happened
    #[arg(
        long,
        conflicts_with_all = [
            "init",
            "create_prompt",
            "add_prompt",
            "list_tools",
            "confirm",
            "unsafe_mode",
            "peek",
            "scope",
            "arg1",
            "prompt",
            "explain"
        ]
    )]
    pub analyze: bool,

    /// Ask for confirmation before executing the generated command
    #[arg(short, long)]
    pub confirm: bool,

    /// Explain the generated command and always ask for confirmation
    #[arg(short = 'e', long, conflicts_with = "analyze")]
    pub explain: bool,

    /// Disable operator-level safety checks (pipes, redirects, etc.).
    /// This always forces an interactive confirmation before running.
    #[arg(short = 'u', long = "unsafe")]
    pub unsafe_mode: bool,

    /// Lift the tool whitelist and operator checks entirely for this call.
    /// Always explains the command and requires typing "yes" to execute it.
    /// Can be forbidden with `safety.allow_unrestricted: false` in the config.
    #[arg(long = "unrestricted")]
    pub unrestricted: bool,

    /// Sample data files to send to the LLM (truncated, for schema inference).
    /// Each file is read up to PEEK_MAX_BYTES and clearly marked as sample data.
    #[arg(short = 'p', long = "peek")]
    pub peek: Vec<String>,

    /// Provide a path or glob hint to narrow the LLM response
    #[arg(short = 's', long = "scope", value_name = "PATTERN")]
    pub scope: Option<String>,

    /// Compose the prompt in the interactive mini editor, even when a prompt
    /// argument is present (the argument becomes the editor's starting text).
    #[arg(short = 'i', long = "interactive", conflicts_with = "no_interactive")]
    pub interactive: bool,

    /// Never open the interactive mini editor; read a single line of prompt
    /// text from standard input instead (legacy behaviour).
    #[arg(long = "no-interactive")]
    pub no_interactive: bool,

    /// Per-call prompt config YAML file. When given, any positional argument is
    /// always the natural language prompt, never a config path.
    #[arg(long = "prompt-config", value_name = "PATH")]
    pub prompt_config: Option<String>,

    /// Either a per-call prompt config YAML file, or the natural language prompt (simple mode).
    /// Optional: when omitted in a terminal, the prompt is composed in the interactive editor.
    pub arg1: Option<String>,

    /// Natural language prompt (advanced mode, when arg1 is a config file)
    pub prompt: Option<String>,
}

/// Where the natural language prompt for this invocation comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptSource {
    /// The prompt was supplied on the command line; use it verbatim.
    Argument(String),

    /// Compose the prompt in the interactive mini editor, optionally starting
    /// from `prefill`.
    Editor { prefill: Option<String> },

    /// Read a single line of prompt text from standard input.
    PlainRead,
}

/// The per-call prompt config path for this invocation, if any.
///
/// `--prompt-config` always wins. Otherwise the historical positional rule
/// applies: `arg1` is a config path only when a second positional argument is
/// present to serve as the prompt.
pub fn resolve_prompt_config_path(cli: &Cli) -> Option<String> {
    if let Some(path) = cli.prompt_config.as_ref() {
        return Some(path.clone());
    }
    match (cli.arg1.as_ref(), cli.prompt.as_ref()) {
        (Some(path), Some(_)) => Some(path.clone()),
        _ => None,
    }
}

/// The natural language prompt text supplied on the command line, if any.
///
/// In advanced mode (`sai cfg.yaml "text"`) that is the second positional.
/// Everywhere else a lone positional is the prompt itself.
fn prompt_argument(cli: &Cli) -> Option<String> {
    if cli.prompt.is_some() {
        return cli.prompt.clone();
    }
    if cli.prompt_config.is_some() {
        return cli.arg1.clone();
    }
    match (cli.arg1.as_ref(), cli.prompt.as_ref()) {
        (Some(text), None) => Some(text.clone()),
        _ => None,
    }
}

/// Decide how to obtain the prompt, without touching the terminal.
///
/// Precedence:
/// 1. `--no-interactive` never opens the editor.
/// 2. `--interactive` always opens it, using any prompt argument as prefill.
/// 3. A prompt argument is used verbatim.
/// 4. Outside a terminal, read a single line from stdin.
/// 5. Otherwise open the editor.
pub fn resolve_prompt_source(cli: &Cli, is_tty: bool) -> PromptSource {
    let argument = prompt_argument(cli);

    if cli.no_interactive {
        return match argument {
            Some(text) => PromptSource::Argument(text),
            None => PromptSource::PlainRead,
        };
    }

    if cli.interactive {
        return PromptSource::Editor { prefill: argument };
    }

    if let Some(text) = argument {
        return PromptSource::Argument(text);
    }

    if is_tty {
        PromptSource::Editor { prefill: None }
    } else {
        PromptSource::PlainRead
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Cli {
        let mut argv = vec!["sai"];
        argv.extend_from_slice(args);
        Cli::parse_from(argv)
    }

    #[test]
    fn prompt_argument_is_used_verbatim() {
        let cli = parse(&["find large files"]);
        assert_eq!(
            resolve_prompt_source(&cli, true),
            PromptSource::Argument("find large files".to_string())
        );
        assert_eq!(resolve_prompt_config_path(&cli), None);
    }

    #[test]
    fn advanced_mode_uses_second_positional_as_prompt() {
        let cli = parse(&["jq-prompt.yaml", "count records per file"]);
        assert_eq!(
            resolve_prompt_source(&cli, true),
            PromptSource::Argument("count records per file".to_string())
        );
        assert_eq!(
            resolve_prompt_config_path(&cli),
            Some("jq-prompt.yaml".to_string())
        );
    }

    #[test]
    fn prompt_config_flag_makes_positional_the_prompt() {
        let cli = parse(&["--prompt-config", "jq-prompt.yaml", "count records per file"]);
        assert_eq!(
            resolve_prompt_source(&cli, true),
            PromptSource::Argument("count records per file".to_string())
        );
        assert_eq!(
            resolve_prompt_config_path(&cli),
            Some("jq-prompt.yaml".to_string())
        );
    }

    #[test]
    fn bare_invocation_in_a_terminal_opens_the_editor() {
        let cli = parse(&[]);
        assert_eq!(
            resolve_prompt_source(&cli, true),
            PromptSource::Editor { prefill: None }
        );
    }

    #[test]
    fn interactive_flag_uses_the_argument_as_prefill() {
        let cli = parse(&["--interactive", "find large files"]);
        assert_eq!(
            resolve_prompt_source(&cli, true),
            PromptSource::Editor {
                prefill: Some("find large files".to_string())
            }
        );
    }

    #[test]
    fn positional_under_interactive_is_never_a_config_path() {
        let cli = parse(&["--interactive", "notes.yaml"]);
        assert_eq!(
            resolve_prompt_source(&cli, true),
            PromptSource::Editor {
                prefill: Some("notes.yaml".to_string())
            }
        );
        assert_eq!(resolve_prompt_config_path(&cli), None);
    }

    #[test]
    fn interactive_with_prompt_config_opens_an_empty_editor() {
        let cli = parse(&["--interactive", "--prompt-config", "jq-prompt.yaml"]);
        assert_eq!(
            resolve_prompt_source(&cli, true),
            PromptSource::Editor { prefill: None }
        );
        assert_eq!(
            resolve_prompt_config_path(&cli),
            Some("jq-prompt.yaml".to_string())
        );
    }

    #[test]
    fn interactive_opens_the_editor_even_without_a_tty() {
        let cli = parse(&["--interactive"]);
        assert_eq!(
            resolve_prompt_source(&cli, false),
            PromptSource::Editor { prefill: None }
        );
    }

    #[test]
    fn no_interactive_in_a_terminal_reads_a_plain_line() {
        let cli = parse(&["--no-interactive"]);
        assert_eq!(resolve_prompt_source(&cli, true), PromptSource::PlainRead);
    }

    #[test]
    fn no_interactive_still_honours_a_prompt_argument() {
        let cli = parse(&["--no-interactive", "find large files"]);
        assert_eq!(
            resolve_prompt_source(&cli, true),
            PromptSource::Argument("find large files".to_string())
        );
    }

    #[test]
    fn non_tty_without_an_argument_reads_a_plain_line() {
        let cli = parse(&[]);
        assert_eq!(resolve_prompt_source(&cli, false), PromptSource::PlainRead);
    }

    #[test]
    fn interactive_and_no_interactive_conflict() {
        let err = Cli::try_parse_from(["sai", "--interactive", "--no-interactive"])
            .expect_err("conflicting mode flags must be rejected");
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn bare_invocation_parses() {
        Cli::try_parse_from(["sai"]).expect("a bare `sai` must parse so the editor can open");
    }

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}

#[cfg(test)]
mod unrestricted_tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Cli {
        let mut argv = vec!["sai"];
        argv.extend_from_slice(args);
        Cli::parse_from(argv)
    }

    #[test]
    fn unrestricted_parses_alone() {
        let cli = parse(&["--unrestricted", "say hi"]);
        assert!(cli.unrestricted);
        assert!(!cli.unsafe_mode);
    }

    #[test]
    fn unrestricted_composes_with_unsafe() {
        let cli = parse(&["--unrestricted", "-u", "say hi"]);
        assert!(cli.unrestricted && cli.unsafe_mode);
    }

    #[test]
    fn unrestricted_composes_with_explain_and_confirm() {
        let cli = parse(&["--unrestricted", "-e", "-c", "say hi"]);
        assert!(cli.unrestricted && cli.explain && cli.confirm);
    }

    #[test]
    fn unrestricted_composes_with_the_interactive_editor() {
        let cli = parse(&["--unrestricted", "-i"]);
        assert!(cli.unrestricted && cli.interactive);
    }

    #[test]
    fn no_flag_can_suppress_inspection() {
        // There is deliberately no --no-explain / --no-confirm to find.
        let rendered = format!("{:?}", Cli::command().get_arguments().map(|a| a.get_id().as_str()).collect::<Vec<_>>());
        for suppressor in ["no_explain", "no_confirm", "quiet", "yes"] {
            assert!(
                !rendered.contains(suppressor),
                "a {} flag would let configuration suppress mandatory inspection",
                suppressor
            );
        }
    }
}
