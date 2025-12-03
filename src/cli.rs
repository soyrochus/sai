use clap::Parser;

/// Command-line interface definition for sai.
#[derive(Parser, Debug, Clone)]
#[command(name = "sai")]
#[command(version)]
#[command(about = "AI-powered, YAML-configured command executor", long_about = None)]
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

    /// Ask for confirmation before executing the generated command
    #[arg(short, long)]
    pub confirm: bool,

    /// Disable operator-level safety checks (pipes, redirects, etc.).
    /// This always forces an interactive confirmation before running.
    #[arg(short = 'u', long = "unsafe")]
    pub unsafe_mode: bool,

    /// Sample data files to send to the LLM (truncated, for schema inference).
    /// Each file is read up to PEEK_MAX_BYTES and clearly marked as sample data.
    #[arg(short = 'p', long = "peek")]
    pub peek: Vec<String>,

    /// Provide a path or glob hint to narrow the LLM response
    #[arg(short = 's', long = "scope", value_name = "PATTERN")]
    pub scope: Option<String>,

    /// Either a per-call prompt config YAML file, or the natural language prompt (simple mode)
    #[arg(required_unless_present_any = ["init", "create_prompt", "add_prompt", "list_tools"])]
    pub arg1: Option<String>,

    /// Natural language prompt (advanced mode, when arg1 is a config file)
    pub prompt: Option<String>,
}
