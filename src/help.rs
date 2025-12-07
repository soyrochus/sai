#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpTopic {
    Overview,
    Quickstart,
    Config,
    Tools,
    Scope,
    Peek,
    Safety,
    Unsafe,
    Explain,
    Analyze,
    History,
    Packages,
    Ops,
    Advanced,
    Topics,
}

impl HelpTopic {
    pub fn from_str(raw: &str) -> Option<Self> {
        let normalized = raw.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "overview" => Some(Self::Overview),
            "quickstart" | "quick-start" | "getting-started" | "getting_started" => {
                Some(Self::Quickstart)
            }
            "config" | "configuration" => Some(Self::Config),
            "tools" | "tool" | "prompt" | "prompts" => Some(Self::Tools),
            "scope" => Some(Self::Scope),
            "peek" => Some(Self::Peek),
            "safety" | "confirm" => Some(Self::Safety),
            "unsafe" => Some(Self::Unsafe),
            "explain" => Some(Self::Explain),
            "analyze" | "analyse" => Some(Self::Analyze),
            "history" | "logs" => Some(Self::History),
            "packages" | "package" => Some(Self::Packages),
            "ops" | "operations" => Some(Self::Ops),
            "advanced" => Some(Self::Advanced),
            "topics" => Some(Self::Topics),
            _ => None,
        }
    }

    pub const fn name(&self) -> &'static str {
        match self {
            HelpTopic::Overview => "overview",
            HelpTopic::Quickstart => "quickstart",
            HelpTopic::Config => "config",
            HelpTopic::Tools => "tools",
            HelpTopic::Scope => "scope",
            HelpTopic::Peek => "peek",
            HelpTopic::Safety => "safety",
            HelpTopic::Unsafe => "unsafe",
            HelpTopic::Explain => "explain",
            HelpTopic::Analyze => "analyze",
            HelpTopic::History => "history",
            HelpTopic::Packages => "packages",
            HelpTopic::Ops => "ops",
            HelpTopic::Advanced => "advanced",
            HelpTopic::Topics => "topics",
        }
    }

    pub const fn short_description(&self) -> &'static str {
        match self {
            HelpTopic::Overview => "High-level introduction to SAI",
            HelpTopic::Quickstart => "Minimal setup and first commands",
            HelpTopic::Config => "Global config, AI providers, defaults",
            HelpTopic::Tools => "Tool definitions and prompt configs",
            HelpTopic::Scope => "How to focus SAI on the right files",
            HelpTopic::Peek => "Sample data for schema inference (--peek)",
            HelpTopic::Safety => "Safety model, operator blocking, confirmation",
            HelpTopic::Unsafe => "What --unsafe relaxes and when to use it",
            HelpTopic::Explain => "Explain generated commands before running them",
            HelpTopic::Analyze => "Analyze the last SAI invocation",
            HelpTopic::History => "Where history is stored and how it is used",
            HelpTopic::Packages => "Built-in prompt configs under prompts/",
            HelpTopic::Ops => "Helper commands (--init, --add-prompt, --list-tools)",
            HelpTopic::Advanced => "Simple vs advanced mode, combining flags",
            HelpTopic::Topics => "List available help topics",
        }
    }

    pub const fn render(&self) -> &'static str {
        match self {
            HelpTopic::Overview => OVERVIEW_HELP,
            HelpTopic::Quickstart => QUICKSTART_HELP,
            HelpTopic::Config => CONFIG_HELP,
            HelpTopic::Tools => TOOLS_HELP,
            HelpTopic::Scope => SCOPE_HELP,
            HelpTopic::Peek => PEEK_HELP,
            HelpTopic::Safety => SAFETY_HELP,
            HelpTopic::Unsafe => UNSAFE_HELP,
            HelpTopic::Explain => EXPLAIN_HELP,
            HelpTopic::Analyze => ANALYZE_HELP,
            HelpTopic::History => HISTORY_HELP,
            HelpTopic::Packages => PACKAGES_HELP,
            HelpTopic::Ops => OPS_HELP,
            HelpTopic::Advanced => ADVANCED_HELP,
            HelpTopic::Topics => TOPICS_HELP,
        }
    }
}

pub struct TopicEntry {
    pub topic: HelpTopic,
    pub description: &'static str,
}

const TOPIC_ENTRIES: &[TopicEntry] = &[
    TopicEntry {
        topic: HelpTopic::Overview,
        description: HelpTopic::Overview.short_description(),
    },
    TopicEntry {
        topic: HelpTopic::Quickstart,
        description: HelpTopic::Quickstart.short_description(),
    },
    TopicEntry {
        topic: HelpTopic::Config,
        description: HelpTopic::Config.short_description(),
    },
    TopicEntry {
        topic: HelpTopic::Tools,
        description: HelpTopic::Tools.short_description(),
    },
    TopicEntry {
        topic: HelpTopic::Scope,
        description: HelpTopic::Scope.short_description(),
    },
    TopicEntry {
        topic: HelpTopic::Peek,
        description: HelpTopic::Peek.short_description(),
    },
    TopicEntry {
        topic: HelpTopic::Safety,
        description: HelpTopic::Safety.short_description(),
    },
    TopicEntry {
        topic: HelpTopic::Explain,
        description: HelpTopic::Explain.short_description(),
    },
    TopicEntry {
        topic: HelpTopic::Analyze,
        description: HelpTopic::Analyze.short_description(),
    },
    TopicEntry {
        topic: HelpTopic::History,
        description: HelpTopic::History.short_description(),
    },
    TopicEntry {
        topic: HelpTopic::Packages,
        description: HelpTopic::Packages.short_description(),
    },
    TopicEntry {
        topic: HelpTopic::Ops,
        description: HelpTopic::Ops.short_description(),
    },
    TopicEntry {
        topic: HelpTopic::Advanced,
        description: HelpTopic::Advanced.short_description(),
    },
];

pub const CLI_USAGE: &str = "sai [FLAGS] [PROMPT_CONFIG] \"<natural language prompt>\"";
pub const CLI_ABOUT: &str = "Sai-cli ('sai') - Tell the shell what you want, not how to do it";
pub const CLI_LONG_ABOUT: &str = "Natural language to safe shell commands using whitelisted tools and an AI backend. Run 'sai help topics' for detailed guidance.";
pub const CLI_AFTER_HELP: &str = r#"Common flags:
  -s, --scope <SCOPE>     Provide a path or hint to restrict context
  -p, --peek <FILE>...    Send sample file(s) for schema inference
  -c, --confirm           Ask before executing the generated command
  -u, --unsafe            Allow pipes and redirects (always implies confirm)
  -e, --explain           Explain the generated command, then ask to confirm
      --analyze           Explain the last sai invocation, do not run anything
      --init              Create a starter config.yaml
      --add-prompt PATH   Merge tools from a prompt file into the global config
      --list-tools [PATH] List tools from global config and optional prompt file

Run:
  sai help topics    to list help topics
  sai help <topic>   for detailed help on <topic>"#;

pub fn try_handle_help(args: &[String]) -> Option<Result<String, String>> {
    if args.first().map(|s| s.eq_ignore_ascii_case("help")) != Some(true) {
        return None;
    }

    if args.len() > 2 {
        return Some(Err("The help command accepts at most one topic.\n\nRun 'sai help topics' to see all available topics.".to_string()));
    }

    let topic = args.get(1).map(|s| s.as_str());
    Some(render_help(topic))
}

pub fn render_help(topic: Option<&str>) -> Result<String, String> {
    match topic {
        None => Ok(render_top_level_help()),
        Some(raw) => {
            let topic = HelpTopic::from_str(raw).ok_or_else(|| unknown_topic_message(raw))?;

            if matches!(topic, HelpTopic::Topics) {
                Ok(render_topics_help())
            } else {
                Ok(topic.render().to_string())
            }
        }
    }
}

pub fn render_top_level_help() -> String {
    let mut output = String::new();
    output.push_str("SAI - Tell the shell what you want, not how to do it.\n\n");
    output.push_str("SAI turns natural language into validated shell commands using a whitelist\n");
    output.push_str("of tools and an AI backend. You pick the tools, SAI proposes a command.\n");
    output.push_str("Simple mode uses your default prompt; advanced mode lets you point at a\n");
    output.push_str("specific prompt config file when you need different tools.\n\n");
    output.push_str("Common usage:\n");
    output.push_str("  sai \"List all Rust files under src\"\n");
    output.push_str("  sai prompts/data-focussed-tool.yml \"Find the fields with ERROR in logs\"\n\n");
    output.push_str("Help topics:\n");
    for entry in TOPIC_ENTRIES {
        output.push_str(&format!(
            "  {:11} {}\n",
            entry.topic.name(),
            entry.description
        ));
    }
    output.push_str("\nRun:\n  sai help <topic>\n");
    output
}

pub fn render_topics_help() -> String {
    let mut output = String::new();
    output.push_str("Available help topics:\n\n");
    for entry in TOPIC_ENTRIES {
        output.push_str(&format!(
            "  {:11} {}\n",
            entry.topic.name(),
            entry.description
        ));
    }
    output
}

fn unknown_topic_message(raw: &str) -> String {
    format!(
        "Unknown help topic '{}'.\n\nRun 'sai help topics' to see all available topics.",
        raw
    )
}

const TOPICS_HELP: &str = r#"Available help topics:

  overview     High-level introduction to SAI
  quickstart   Minimal setup and first commands
  config       Global config, AI providers, defaults
  tools        Tool definitions and prompt configs
  scope        How to focus SAI on the right files
  peek         Sample data for schema inference (--peek)
  safety       Safety model, operator blocking, confirmation
  explain      Explain generated commands before running them
  analyze      Analyze the last SAI invocation
  history      Where history is stored and how it is used
  packages     Built-in prompt configs under prompts/
  ops          Helper commands (--init, --add-prompt, --list-tools)
  advanced     Simple vs advanced mode, combining flags"#;

const OVERVIEW_HELP: &str = r#"SAI - Tell the shell what you want, not how to do it.

SAI converts natural language instructions into validated shell commands. You
describe the goal, SAI selects from a whitelist of tools in your prompt config
and proposes a single command to run.

Principles:
- The shell remains in control: SAI generates commands, it never becomes a shell.
- Safety first: default mode blocks pipes, redirects, substitution, and chaining.
- Context matters: scopes, prompt configs, and sample data guide the model.

Use `sai help quickstart` for setup, or `sai help tools` to shape the tool list."#;

const QUICKSTART_HELP: &str = r#"SAI needs an AI key (OpenAI or Azure) and the `sai` binary on your PATH.

1) Run `sai --init` to create a starter config at the platform default location.
2) Edit config.yaml: set provider, API key, model, and optionally default_prompt.
   This includes a number of standard tools.
3) Run a first command.

Copy-paste:
  sai --init
  sai "List all Rust files under src"

Simple mode (`sai "<prompt>"`) uses default_prompt. Advanced mode takes a prompt
file first so you can swap toolsets per run. Ensure both `sai` and your tools
are on PATH."#;

const CONFIG_HELP: &str = r#"Global config lives at:
- Linux: ~/.config/sai/config.yaml
- macOS: ~/Library/Application Support/sai/config.yaml
- Windows: %APPDATA%/sai/config.yaml

Sections:
- ai: provider (openai|azure), credentials, model, and optional base URL/endpoint.
  Env vars override file values: SAI_PROVIDER, SAI_OPENAI_API_KEY/BASE_URL/MODEL,
  SAI_AZURE_API_KEY/ENDPOINT/DEPLOYMENT/API_VERSION.
- default_prompt: meta_prompt plus tools[]. Used whenever you omit a per-call
  prompt YAML. Provide a prompt file as the first argument to override.

Use `sai --init` to generate a starter config with placeholder credentials."#;

const TOOLS_HELP: &str = r#"A tool is a named capability with instructions for the LLM. Tools are defined in
`default_prompt.tools` in the global config and in per-call prompt YAML files.

Common operations:
- Create a template: `sai --create-prompt <command> [path]`.
- Merge tools into the global default: `sai --add-prompt prompts/data-focussed-tool.yml`.
- List what is allowed: `sai --list-tools [prompt.yml]`.

Safety: only tools listed in the active prompt are allowed. Default mode also
blocks pipes/redirects; add `--unsafe` to relax operators, but tools stay
whitelisted. Use per-call prompt files to experiment without altering defaults."#;

const SCOPE_HELP: &str = r#"-s/--scope supplies a hint to focus the model. It can be free text, a path, or a
glob. Scope narrows the prompt context; it does not sandbox execution.

Special case: `-s .` injects a non-recursive directory listing of the current
working directory (bounded by an internal size limit) so the model sees nearby
filenames without extra typing."#;

const PEEK_HELP: &str = r#"--peek sends truncated sample data to the LLM for schema inference. Each file is
read up to an internal byte limit and clearly marked as sample data. Use it to
show record layout, not to process full datasets. Only include files you are
comfortable sending to the provider."#;

const SAFETY_HELP: &str = r#"Safety in SAI has three layers:

1) Tool whitelist: commands may only use tools defined in the active prompt.
2) Operator blocking: default mode rejects pipes, redirects, &&, ||, and subshells.
3) Confirmation: `-c/--confirm` asks before running; `--unsafe` and `--explain`
   always imply confirmation.

Commands are executed directly (no implicit `/bin/sh -c`). `--explain` and
`--analyze` are read-only operations that never run shell commands. Use
`--unsafe` sparingly when you intentionally need operators."#;

const UNSAFE_HELP: &str = r#"--unsafe keeps the tool whitelist but disables operator blocking so pipes,
redirects, substitution, and chaining are allowed. It always forces a prompt
before execution. Use it when a single safe command is impossible; prefer
explicit tools and scopes first."#;

const EXPLAIN_HELP: &str = r#"-e/--explain generates the command, asks the LLM to summarize it, prints the
explanation, and then asks for confirmation before execution.

Example shape:
Generated command:
  find . -name '*.py'

Explanation:
  Searches for Python files under the current directory.

This mode is useful for learning what will happen before you run the command."#;

const ANALYZE_HELP: &str = r#"--analyze reads the most recent history entry and asks the LLM to explain what
likely happened, why it succeeded or failed, and what to try next. It never
executes commands and cannot be combined with other options or prompts. Run it
after an error or surprising output to get guidance."#;

const HISTORY_HELP: &str = r#"SAI records each invocation as NDJSON in a history log with timestamp, cwd,
argv, generated command, flags, and notes.

Log locations:
- Linux: ~/.config/sai/history.log
- macOS: ~/Library/Application Support/sai/history.log
- Windows: %APPDATA%/sai/history.log

Files rotate around 1 MB, keeping one backup. `--analyze` reads this log."#;

const PACKAGES_HELP: &str = r#"Prompt packages ship under prompts/ so you can start quickly or extend them:

- prompts/data-focussed-tool.yml     Data-centric tools (jq, yq, csvkit, mlr, ...)
- prompts/safe-destructive-tools.yml Destructive-capable tools with guardrails
- prompts/git-safe.yml               Read-only git operations
- prompts/git-full.yml               Full git workflows (confirm strongly advised)

Use them directly: `sai prompts/data-focussed-tool.yml "List json files under logs"`.
Copy and edit to suit your environment or merge with `--add-prompt`."#;

const OPS_HELP: &str = r#"Helper operations that do not invoke the LLM:

- `--init` writes a starter config.yaml with placeholder AI credentials and
  standard Unix tools (grep, find, awk, sed, etc.) pre-configured.
- `--create-prompt <command> [path]` writes a per-call prompt template.
- `--add-prompt PATH` merges additional tools from PATH into the global default
  prompt, resolving conflicts interactively when a TTY is available.
- `--list-tools [PATH]` prints tools from the global config and optionally a
  prompt file, marking which ones are on PATH.

Examples:
  sai --init
  sai --add-prompt prompts/git-safe.yml      # Add git tools
  sai --add-prompt prompts/data-focussed-tool.yml  # Add jq, yq, csvkit
  sai --list-tools

The standard tools are already included by --init. Use --add-prompt to extend
with specialized toolsets like git or data processing tools."#;

const ADVANCED_HELP: &str = r#"Simple mode: `sai "<prompt>"` uses `default_prompt` from the global config.
Advanced mode: `sai prompt.yml "<prompt>"` uses a specific prompt file so you
can swap toolsets per request.

Combine flags as needed:
- `--scope` to steer the model toward the right files.
- `--peek` to show sample data.
- `--explain` or `--confirm` for interactive review.
- `--unsafe` when you explicitly allow operators.

Environment variables (`SAI_*`) override AI config, which is handy for switching
providers or models per shell session. Ensure required tools are on PATH or use
absolute paths in tool names for clarity."#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_help_topics_case_insensitively() {
        assert_eq!(HelpTopic::from_str("overview"), Some(HelpTopic::Overview));
        assert_eq!(
            HelpTopic::from_str("QuickStart"),
            Some(HelpTopic::Quickstart)
        );
        assert_eq!(HelpTopic::from_str("prompts"), Some(HelpTopic::Tools));
        assert_eq!(HelpTopic::from_str("TOPICS"), Some(HelpTopic::Topics));
    }

    #[test]
    fn top_level_help_has_header_and_topics() {
        let help = render_top_level_help();
        assert!(help.contains("SAI - Tell the shell what you want"));
        for entry in TOPIC_ENTRIES {
            assert!(
                help.contains(entry.topic.name()),
                "top-level help should list topic {}",
                entry.topic.name()
            );
        }
    }

    #[test]
    fn topics_help_lists_all_topics() {
        let topics = render_topics_help();
        for entry in TOPIC_ENTRIES {
            assert!(
                topics.contains(entry.topic.name()),
                "topics output should include {}",
                entry.topic.name()
            );
        }
    }

    #[test]
    fn unknown_topic_reports_error() {
        let err = render_help(Some("unknown-topic")).unwrap_err();
        assert!(err.contains("Unknown help topic"));
        assert!(err.contains("sai help topics"));
    }
}
