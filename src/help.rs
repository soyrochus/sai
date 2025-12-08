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
            HelpTopic::Overview => "High-level introduction to sai-cli",
            HelpTopic::Quickstart => "Minimal setup and first commands",
            HelpTopic::Config => "Global config, AI providers, defaults",
            HelpTopic::Tools => "Tool definitions and prompt configs",
            HelpTopic::Scope => "How to focus sai-cli on the right files",
            HelpTopic::Peek => "Sample data for schema inference (--peek)",
            HelpTopic::Safety => "Safety model, operator blocking, confirmation",
            HelpTopic::Unsafe => "What --unsafe relaxes and when to use it",
            HelpTopic::Explain => "Explain generated commands before running them",
            HelpTopic::Analyze => "Analyze the last sai invocation",
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
    output.push_str("Sai-cli - Tell the shell what you want, not how to do it.\n\n");
    output.push_str(
        "Sai-cli turns natural language into validated shell commands using a whitelist\n",
    );
    output
        .push_str("of tools and an AI backend. You pick the tools, sai-cli proposes a command.\n");
    output.push_str("Simple mode uses your default prompt; advanced mode lets you point at a\n");
    output.push_str("specific prompt config file when you need different tools, stricter rules,\n");
    output.push_str(
        "or a dedicated set of helpers. Explain and confirm modes keep execution safe.\n\n",
    );
    output.push_str("Common usage:\n");
    output.push_str("  sai \"List all Rust files under src\"\n");
    output
        .push_str("  sai prompts/data-focussed-tool.yml \"Summarize columns in access.log.csv\"\n");
    output.push_str("  sai --peek sample.json \"Suggest a jq filter for this structure\"\n\n");
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

const TOPICS_HELP: &str = include_str!("../templates/help/topics.txt");
const OVERVIEW_HELP: &str = include_str!("../templates/help/overview.txt");
const QUICKSTART_HELP: &str = include_str!("../templates/help/quickstart.txt");
const CONFIG_HELP: &str = include_str!("../templates/help/config.txt");
const TOOLS_HELP: &str = include_str!("../templates/help/tools.txt");
const SCOPE_HELP: &str = include_str!("../templates/help/scope.txt");
const PEEK_HELP: &str = include_str!("../templates/help/peek.txt");
const SAFETY_HELP: &str = include_str!("../templates/help/safety.txt");
const UNSAFE_HELP: &str = include_str!("../templates/help/unsafe.txt");
const EXPLAIN_HELP: &str = include_str!("../templates/help/explain.txt");
const ANALYZE_HELP: &str = include_str!("../templates/help/analyze.txt");
const HISTORY_HELP: &str = include_str!("../templates/help/history.txt");
const PACKAGES_HELP: &str = include_str!("../templates/help/packages.txt");
const OPS_HELP: &str = include_str!("../templates/help/ops.txt");
const ADVANCED_HELP: &str = include_str!("../templates/help/advanced.txt");

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
        assert!(help.contains("Sai-cli - Tell the shell what you want"));
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
    fn help_texts_come_from_templates() {
        let cases: &[(HelpTopic, &str)] = &[
            (
                HelpTopic::Overview,
                include_str!("../templates/help/overview.txt"),
            ),
            (
                HelpTopic::Quickstart,
                include_str!("../templates/help/quickstart.txt"),
            ),
            (
                HelpTopic::Config,
                include_str!("../templates/help/config.txt"),
            ),
            (
                HelpTopic::Tools,
                include_str!("../templates/help/tools.txt"),
            ),
            (
                HelpTopic::Scope,
                include_str!("../templates/help/scope.txt"),
            ),
            (HelpTopic::Peek, include_str!("../templates/help/peek.txt")),
            (
                HelpTopic::Safety,
                include_str!("../templates/help/safety.txt"),
            ),
            (
                HelpTopic::Unsafe,
                include_str!("../templates/help/unsafe.txt"),
            ),
            (
                HelpTopic::Explain,
                include_str!("../templates/help/explain.txt"),
            ),
            (
                HelpTopic::Analyze,
                include_str!("../templates/help/analyze.txt"),
            ),
            (
                HelpTopic::History,
                include_str!("../templates/help/history.txt"),
            ),
            (
                HelpTopic::Packages,
                include_str!("../templates/help/packages.txt"),
            ),
            (HelpTopic::Ops, include_str!("../templates/help/ops.txt")),
            (
                HelpTopic::Advanced,
                include_str!("../templates/help/advanced.txt"),
            ),
            (
                HelpTopic::Topics,
                include_str!("../templates/help/topics.txt"),
            ),
        ];

        for (topic, template) in cases {
            assert_eq!(
                topic.render(),
                *template,
                "help topic {} should render its template text",
                topic.name()
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
