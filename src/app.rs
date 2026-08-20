use crate::cli::{Cli, PromptSource, resolve_prompt_config_path, resolve_prompt_source};
use crate::config::{
    find_global_config_path, load_global_config, load_prompt_config, resolve_ai_config,
};
use crate::editor;
use crate::executor::{CommandExecutor, ShellCommandExecutor};
use crate::help;
use crate::history::{self, HistoryEntry};
use crate::llm::{ChatClient, CommandGenerator, HttpCommandGenerator};
use crate::ops;
use crate::peek::build_peek_context;
use crate::prompt::build_system_prompt;
use crate::prompt_history;
use crate::safety::{RiskMarker, risk_markers, validate_and_split_command};
use crate::safety_mode::SafetyMode;
use anyhow::{Context, Result, anyhow};
use clap::Parser;
use std::env;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RunSummary {
    pub exit_code: i32,
    pub generated_command: Option<String>,
    pub unsafe_mode: bool,
    pub unrestricted: bool,
    pub confirm: bool,
    pub explain: bool,
    pub scope: Option<String>,
    pub peek_files: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExplainSource {
    None,
    Flag,
    ToolConfig(String),
    UnrestrictedMode,
}

impl ExplainSource {
    fn resolve(
        safety_mode: SafetyMode,
        explain_flag: bool,
        tool_requires_explain: bool,
        primary_tool: &str,
    ) -> Self {
        if safety_mode.forces_inspection() {
            Self::UnrestrictedMode
        } else if explain_flag {
            Self::Flag
        } else if tool_requires_explain {
            Self::ToolConfig(primary_tool.to_string())
        } else {
            Self::None
        }
    }

    fn is_enabled(&self) -> bool {
        !matches!(self, Self::None)
    }

    fn card_value(&self) -> Option<String> {
        match self {
            Self::None => None,
            Self::Flag => Some("--explain flag".to_string()),
            Self::ToolConfig(tool) => {
                Some(format!("tool config ({tool}: force_explain)"))
            }
            Self::UnrestrictedMode => {
                Some("unrestricted mode (mandatory inspection)".to_string())
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum PromptConfigProvenance<'a> {
    GlobalDefault(&'a Path),
    PerCall(&'a Path),
}

#[derive(Debug)]
struct PreflightCard<'a> {
    prompt: &'a str,
    command: &'a str,
    primary_tool: &'a str,
    scope_hint: Option<&'a str>,
    safety_mode: SafetyMode,
    explain_source: &'a ExplainSource,
    risk_markers: &'a [RiskMarker],
    config_provenance: PromptConfigProvenance<'a>,
}

impl PreflightCard<'_> {
    fn render(&self) -> String {
        const LABEL_WIDTH: usize = 9;

        fn push_field(lines: &mut Vec<String>, label: &str, value: &str) {
            const LABEL_WIDTH: usize = 9;
            for (index, value_line) in value.split('\n').enumerate() {
                let rendered_label = if index == 0 {
                    format!("{label}:")
                } else {
                    String::new()
                };
                lines.push(format!(
                    "  {rendered_label:<LABEL_WIDTH$}{value_line}",
                    LABEL_WIDTH = LABEL_WIDTH
                ));
            }
        }

        let mut lines = vec!["Preflight:".to_string()];
        push_field(&mut lines, "Prompt", self.prompt);
        push_field(&mut lines, "Command", self.command);
        push_field(&mut lines, "Tool", self.primary_tool);
        if let Some(scope_hint) = self.scope_hint {
            push_field(&mut lines, "Scope", scope_hint);
        }

        let safety_mode = match self.safety_mode {
            SafetyMode::Default => "default",
            SafetyMode::Unsafe => "unsafe",
            SafetyMode::Unrestricted => "unrestricted",
        };
        push_field(&mut lines, "Safety", safety_mode);

        if let Some(explain_source) = self.explain_source.card_value() {
            push_field(&mut lines, "Explain", &explain_source);
        }

        if self.risk_markers.is_empty() {
            push_field(&mut lines, "Risk", "none found");
        } else {
            for (index, marker) in self.risk_markers.iter().enumerate() {
                let label = if index == 0 { "Risk:" } else { "" };
                lines.push(format!(
                    "  {label:<LABEL_WIDTH$}[{}] {}",
                    marker.kind.label(),
                    marker.detail,
                    LABEL_WIDTH = LABEL_WIDTH
                ));
            }
        }

        let config = match self.config_provenance {
            PromptConfigProvenance::GlobalDefault(path) => {
                format!("global default ({})", path.display())
            }
            PromptConfigProvenance::PerCall(path) => {
                format!("per-call prompt config ({})", path.display())
            }
        };
        push_field(&mut lines, "Config", &config);

        debug_assert!(LABEL_WIDTH >= "Explain:".len());
        lines.join("\n")
    }
}

impl RunSummary {
    fn from_cli(cli: &Cli) -> Self {
        Self {
            exit_code: 0,
            generated_command: None,
            unsafe_mode: SafetyMode::from_cli(cli).is_unsafe_for_history(),
            unrestricted: cli.unrestricted,
            confirm: cli.confirm || cli.unsafe_mode || cli.explain,
            explain: cli.explain,
            scope: cli.scope.clone(),
            peek_files: cli.peek.clone(),
            notes: None,
        }
    }

    fn analyze_mode() -> Self {
        Self {
            exit_code: 0,
            generated_command: None,
            unsafe_mode: false,
            unrestricted: false,
            confirm: false,
            explain: false,
            scope: None,
            peek_files: Vec::new(),
            notes: None,
        }
    }
}

pub fn run() -> Result<()> {
    let raw_args: Vec<String> = env::args().collect();
    if let Some(help) = help::try_handle_help(&raw_args[1..]) {
        match help {
            Ok(text) => {
                println!("{}", text);
                std::process::exit(0);
            }
            Err(err) => {
                eprintln!("{}", err);
                std::process::exit(1);
            }
        }
    }

    let cli = Cli::parse();
    let executor = ShellCommandExecutor;
    let exit_code = if requires_generator(&cli) {
        let generator = HttpCommandGenerator::new();
        run_and_log(cli, &generator, &executor)
    } else {
        let generator = NoopGenerator;
        run_and_log(cli, &generator, &executor)
    };
    std::process::exit(exit_code);
}

fn requires_generator(cli: &Cli) -> bool {
    !cli.init && cli.create_prompt.is_none() && cli.add_prompt.is_none() && !cli.list_tools
}

fn run_and_log<G, E>(cli: Cli, generator: &G, executor: &E) -> i32
where
    G: CommandGenerator + ChatClient,
    E: CommandExecutor,
{
    let argv: Vec<String> = env::args().collect();
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let mut exit_code = 1;
    let mut summary: Option<RunSummary> = None;
    let notes: Option<String>;

    let run_result = {
        let stdin = io::stdin();
        let mut stdin_lock = stdin.lock();
        run_with_reader(cli.clone(), generator, executor, &mut stdin_lock)
    };

    match run_result {
        Ok(res) => {
            exit_code = res.exit_code;
            notes = res.notes.clone();
            summary = Some(res);
        }
        Err(err) => {
            eprintln!("Error: {:#}", err);
            notes = Some(err.to_string());
        }
    }

    let entry = history_entry_for_run(
        &cli,
        argv,
        &cwd,
        exit_code,
        summary.as_ref(),
        notes,
    );

    if let Err(err) = history::write_entry(entry) {
        eprintln!("Warning: failed to write history: {:#}", err);
    }

    exit_code
}

fn history_entry_for_run(
    cli: &Cli,
    argv: Vec<String>,
    cwd: &Path,
    exit_code: i32,
    summary: Option<&RunSummary>,
    notes: Option<String>,
) -> HistoryEntry {
    let (confirm, explain, unsafe_mode, unrestricted, scope, peek_files, generated_command) =
        if let Some(summary) = summary {
            (
                summary.confirm,
                summary.explain,
                summary.unsafe_mode,
                summary.unrestricted,
                summary.scope.clone(),
                summary.peek_files.clone(),
                summary.generated_command.clone(),
            )
        } else {
            (
                cli.confirm || cli.unsafe_mode || cli.explain,
                cli.explain,
                SafetyMode::from_cli(cli).is_unsafe_for_history(),
                cli.unrestricted,
                cli.scope.clone(),
                cli.peek.clone(),
                None,
            )
        };

    HistoryEntry {
        ts: history::now_iso_ts(),
        cwd: cwd.to_string_lossy().to_string(),
        argv,
        exit_code,
        generated_command,
        unsafe_mode,
        unrestricted,
        confirm,
        explain,
        scope,
        peek_files,
        notes,
    }
}

#[allow(dead_code)]
pub fn run_with_dependencies<G, E>(cli: Cli, generator: &G, executor: &E) -> Result<RunSummary>
where
    G: CommandGenerator + ChatClient,
    E: CommandExecutor,
{
    let stdin = io::stdin();
    let mut stdin_lock = stdin.lock();
    run_with_reader(cli, generator, executor, &mut stdin_lock)
}

pub fn run_with_reader<G, E, R>(
    cli: Cli,
    generator: &G,
    executor: &E,
    reader: &mut R,
) -> Result<RunSummary>
where
    G: CommandGenerator + ChatClient,
    E: CommandExecutor,
    R: BufRead,
{
    let mut confirmation_output = io::stderr();
    run_with_reader_and_confirmation_output(
        cli,
        generator,
        executor,
        reader,
        &mut confirmation_output,
    )
}

fn run_with_reader_and_confirmation_output<G, E, R, W>(
    cli: Cli,
    generator: &G,
    executor: &E,
    reader: &mut R,
    confirmation_output: &mut W,
) -> Result<RunSummary>
where
    G: CommandGenerator + ChatClient,
    E: CommandExecutor,
    R: BufRead,
    W: Write,
{
    let global_config_path = find_global_config_path();

    if cli.init {
        ops::init_global_config(&global_config_path)?;
        let mut summary = RunSummary::from_cli(&cli);
        summary.notes = Some("init".to_string());
        return Ok(summary);
    }

    if let Some(values) = cli.create_prompt.as_ref() {
        ops::create_prompt_template(values)?;
        let mut summary = RunSummary::from_cli(&cli);
        summary.notes = Some("create_prompt".to_string());
        return Ok(summary);
    }

    if let Some(path) = cli.add_prompt.as_ref() {
        ops::add_prompt_to_global(&global_config_path, Path::new(path))?;
        let mut summary = RunSummary::from_cli(&cli);
        summary.notes = Some("add_prompt".to_string());
        return Ok(summary);
    }

    if cli.list_tools {
        ops::list_tools(&global_config_path, cli.arg1.as_deref())?;
        let mut summary = RunSummary::from_cli(&cli);
        summary.notes = Some("list_tools".to_string());
        return Ok(summary);
    }

    let global_cfg = load_global_config(&global_config_path)?;

    let safety_mode = SafetyMode::from_cli(&cli);

    // Refuse a forbidden mode before contacting the model, so a run that was
    // never permitted costs no tokens and records no command.
    if safety_mode.is_unrestricted() && !global_cfg.allows_unrestricted() {
        return Err(anyhow!(
            "Unrestricted mode is disabled by {}. Remove 'safety.allow_unrestricted: false' from that file to enable it.",
            global_config_path.display()
        ));
    }

    if cli.analyze {
        return run_analyze(&global_cfg, generator);
    }

    let (prompt_cfg, prompt_source): (crate::config::PromptConfig, Option<PathBuf>) =
        match resolve_prompt_config_path(&cli) {
            Some(path) => {
                let cfg_path = PathBuf::from(path);
                let prompt_cfg = load_prompt_config(&cfg_path)?;
                (prompt_cfg, Some(cfg_path))
            }
            None => {
                let prompt_cfg = global_cfg.default_prompt.clone().ok_or_else(|| {
                    anyhow!("No default_prompt found in global config for simple mode")
                })?;
                (prompt_cfg, None)
            }
        };

    let nl_prompt = match acquire_prompt(&cli, reader)? {
        Some(prompt) => prompt,
        None => {
            eprintln!("Cancelled.");
            let mut summary = RunSummary::from_cli(&cli);
            summary.exit_code = 0;
            summary.notes = Some("cancelled".to_string());
            return Ok(summary);
        }
    };

    prompt_history::record(&nl_prompt);

    let (system_prompt, allowed_tools) = build_system_prompt(&prompt_cfg, safety_mode)?;
    let peek_context = build_peek_context(&cli.peek)?;
    let effective_ai = resolve_ai_config(global_cfg.ai)?;

    let cmd_line = generator
        .generate(
            &effective_ai,
            &system_prompt,
            &nl_prompt,
            cli.scope.as_deref(),
            peek_context.as_deref(),
        )
        .context("Failed to obtain command from LLM")?;

    eprintln!(">> {}", cmd_line);

    let tokens = validate_and_split_command(&cmd_line, &allowed_tools, safety_mode)?;

    // Check if the generated command uses a tool that requires forced explain mode
    let tool_requires_explain = crate::prompt::should_force_explain(&prompt_cfg.tools, &cmd_line);
    // Under an unrestricted run mandatory inspection wins over every other
    // explanation source, so the card reports the effective reason.
    let explain_source = ExplainSource::resolve(
        safety_mode,
        cli.explain,
        tool_requires_explain,
        &tokens[0],
    );
    let effective_explain = explain_source.is_enabled();
    let effective_confirm =
        safety_mode.forces_inspection() || cli.confirm || cli.unsafe_mode || effective_explain;

    let mut summary = RunSummary::from_cli(&cli);
    summary.generated_command = Some(cmd_line.clone());
    summary.explain = effective_explain;
    summary.confirm = effective_confirm;

    if effective_explain {
        print_command_explanation(generator, &effective_ai, &cmd_line)?;
    }

    let confirmed = if !effective_confirm {
        true
    } else {
        let markers = risk_markers(&cmd_line);
        let config_provenance = match prompt_source.as_deref() {
            Some(path) => PromptConfigProvenance::PerCall(path),
            None => PromptConfigProvenance::GlobalDefault(&global_config_path),
        };
        let card = PreflightCard {
            prompt: &nl_prompt,
            command: &cmd_line,
            primary_tool: &tokens[0],
            scope_hint: cli.scope.as_deref(),
            safety_mode,
            explain_source: &explain_source,
            risk_markers: &markers,
            config_provenance,
        };

        if safety_mode.is_unrestricted() {
            confirm_unrestricted(reader, confirmation_output, &card)?
        } else {
            confirm(reader, confirmation_output, &card)?
        }
    };

    if !confirmed {
        eprintln!("Cancelled.");
        summary.exit_code = 0;
        summary.notes = Some("cancelled".to_string());
        return Ok(summary);
    }

    let status = executor.execute(&cmd_line, &tokens, safety_mode.uses_shell())?;
    summary.exit_code = status;
    Ok(summary)
}

/// Obtain the natural language prompt for this run.
///
/// Returns `Ok(None)` when the user cancelled composition. The chosen input
/// mode is the only thing that varies here; the prompt string it produces goes
/// through exactly the same downstream flow either way.
fn acquire_prompt<R>(cli: &Cli, reader: &mut R) -> Result<Option<String>>
where
    R: BufRead,
{
    let is_tty = io::stdin().is_terminal();

    match resolve_prompt_source(cli, is_tty) {
        PromptSource::Argument(text) => Ok(Some(text)),
        PromptSource::PlainRead => editor::read_plain_line(reader, is_tty).map(Some),
        PromptSource::Editor { prefill } => {
            let history = prompt_history::load();
            match editor::compose(prefill.clone(), history)? {
                Some(editor::EditorOutcome::Submitted(text)) => Ok(Some(text)),
                Some(editor::EditorOutcome::Cancelled) => Ok(None),
                // The terminal refused raw mode; fall back rather than fail.
                None => editor::read_plain_line(reader, is_tty).map(Some),
            }
        }
    }
}

/// Read one line of answer from `reader`.
fn read_answer(reader: &mut dyn BufRead) -> Result<String> {
    let mut buf = String::new();
    reader.read_line(&mut buf)?;
    Ok(buf.trim().to_lowercase())
}

fn print_preflight_card(output: &mut dyn Write, card: &PreflightCard<'_>) -> Result<()> {
    writeln!(output, "{}", card.render()).context("Failed to write preflight card")?;
    Ok(())
}

/// The confirmation shown under `--unrestricted`.
///
/// Requires the full word `yes`. A bare `y` clears every other prompt in SAI
/// and deliberately does not clear this one: this is the mode where a wrong
/// command is unbounded, so muscle memory should not be enough.
fn confirm_unrestricted(
    reader: &mut dyn BufRead,
    output: &mut dyn Write,
    card: &PreflightCard<'_>,
) -> Result<bool> {
    print_preflight_card(output, card)?;
    writeln!(
        output,
        "UNRESTRICTED: no tool whitelist is in effect for this command."
    )?;
    write!(output, "Type 'yes' to execute: ")?;
    output.flush()?;

    Ok(read_answer(reader)? == "yes")
}

fn confirm(
    reader: &mut dyn BufRead,
    output: &mut dyn Write,
    card: &PreflightCard<'_>,
) -> Result<bool> {
    print_preflight_card(output, card)?;
    write!(output, "Execute this command? [y/N] ")?;
    output.flush()?;
    let ans = read_answer(reader)?;
    Ok(ans == "y" || ans == "yes")
}

fn print_command_explanation<G>(
    generator: &G,
    ai: &crate::config::EffectiveAiConfig,
    cmd_line: &str,
) -> Result<()>
where
    G: ChatClient,
{
    let system_prompt = "You are a shell and tool usage explainer. \
Given a shell command, explain in concise technical language what it will do, \
describing each flag and argument, and the overall effect. \
Do not invent behaviour not implied by the command.";
    let user_prompt = format!(
        "Explain this command in detail, but concisely:\n\n{}",
        cmd_line
    );

    println!("Generated command:\n  {}\n", cmd_line);
    match generator.respond(ai, system_prompt, &user_prompt, 0.0) {
        Ok(explanation) => {
            println!("Explanation:\n{}", explanation);
        }
        Err(err) => {
            eprintln!("Failed to explain command: {:#}", err);
        }
    }

    Ok(())
}

fn run_analyze<G>(global_cfg: &crate::config::GlobalConfig, generator: &G) -> Result<RunSummary>
where
    G: ChatClient,
{
    let mut summary = RunSummary::analyze_mode();
    summary.notes = Some("analyze mode".to_string());

    let latest = history::read_latest_entry()?;
    let Some(entry) = latest else {
        println!("No history available to analyze yet.");
        summary.exit_code = 2;
        return Ok(summary);
    };

    let entry_json = serde_json::to_string_pretty(&entry)?;
    let system_prompt = "You are a debugging assistant for the SAI CLI. You receive structured information about the last SAI invocation (command line, generated shell command, exit code, etc.). Explain in concise technical terms what likely happened and why, and suggest what the user might try next. If information is missing, state the limitations.";
    let user_prompt = format!(
        "Here is the last SAI invocation as a JSON object:\n\n{}\n\nPlease explain what likely happened and why.",
        entry_json
    );

    let effective_ai = resolve_ai_config(global_cfg.ai.clone())?;
    let explanation = generator.respond(&effective_ai, system_prompt, &user_prompt, 0.0)?;

    println!("{}", explanation);
    Ok(summary)
}

struct NoopGenerator;

impl CommandGenerator for NoopGenerator {
    fn generate(
        &self,
        _ai: &crate::config::EffectiveAiConfig,
        _system_prompt: &str,
        _nl_prompt: &str,
        _scope_hint: Option<&str>,
        _peek_text: Option<&str>,
    ) -> Result<String> {
        Err(anyhow!("LLM generator should not be used for this command"))
    }
}

impl ChatClient for NoopGenerator {
    fn respond(
        &self,
        _ai: &crate::config::EffectiveAiConfig,
        _system_prompt: &str,
        _user_prompt: &str,
        _temperature: f32,
    ) -> Result<String> {
        Err(anyhow!("Chat client should not be used for this command"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use crate::config::set_config_dir_override_for_tests;
    use crate::llm::{ChatClient, CommandGenerator};
    use crate::safety::RiskKind;
    use std::cell::Cell;
    use std::fs;
    use std::io::Cursor;
    use std::path::Path;
    use tempfile::TempDir;

    struct StubGenerator {
        command: String,
        response: String,
    }

    impl StubGenerator {
        fn new(command: &str, response: &str) -> Self {
            Self {
                command: command.to_string(),
                response: response.to_string(),
            }
        }
    }

    impl CommandGenerator for StubGenerator {
        fn generate(
            &self,
            _ai: &crate::config::EffectiveAiConfig,
            _system_prompt: &str,
            _nl_prompt: &str,
            _scope_hint: Option<&str>,
            _peek_text: Option<&str>,
        ) -> Result<String> {
            Ok(self.command.clone())
        }
    }

    impl ChatClient for StubGenerator {
        fn respond(
            &self,
            _ai: &crate::config::EffectiveAiConfig,
            _system_prompt: &str,
            _user_prompt: &str,
            _temperature: f32,
        ) -> Result<String> {
            Ok(self.response.clone())
        }
    }

    struct ExplanationTrackingGenerator<'a> {
        explained: &'a Cell<bool>,
    }

    impl CommandGenerator for ExplanationTrackingGenerator<'_> {
        fn generate(
            &self,
            _ai: &crate::config::EffectiveAiConfig,
            _system_prompt: &str,
            _nl_prompt: &str,
            _scope_hint: Option<&str>,
            _peek_text: Option<&str>,
        ) -> Result<String> {
            Ok("echo hello".to_string())
        }
    }

    impl ChatClient for ExplanationTrackingGenerator<'_> {
        fn respond(
            &self,
            _ai: &crate::config::EffectiveAiConfig,
            _system_prompt: &str,
            _user_prompt: &str,
            _temperature: f32,
        ) -> Result<String> {
            self.explained.set(true);
            Ok("prints hello".to_string())
        }
    }

    struct AfterExplanationWriter<'a> {
        explained: &'a Cell<bool>,
        bytes: Vec<u8>,
    }

    impl Write for AfterExplanationWriter<'_> {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            assert!(
                self.explained.get(),
                "the explanation must complete before the card is written"
            );
            self.bytes.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct RecordingExecutor {
        ran: Cell<bool>,
        status: i32,
    }

    impl Default for RecordingExecutor {
        fn default() -> Self {
            Self {
                ran: Cell::new(false),
                status: 0,
            }
        }
    }

    impl RecordingExecutor {
        fn with_status(status: i32) -> Self {
            Self {
                ran: Cell::new(false),
                status,
            }
        }

        fn ran(&self) -> bool {
            self.ran.get()
        }
    }

    impl CommandExecutor for RecordingExecutor {
        fn execute(&self, _cmd_line: &str, _tokens: &[String], _unsafe_mode: bool) -> Result<i32> {
            self.ran.set(true);
            Ok(self.status)
        }
    }

    fn write_minimal_config(dir: &Path) {
        fs::create_dir_all(dir).unwrap();
        let cfg = r#"
ai:
  provider: openai
  openai_api_key: test-key
  openai_model: test-model
default_prompt:
  tools:
    - name: echo
      config: "echo tool"
"#;
        fs::write(dir.join("config.yaml"), cfg).unwrap();
    }

    fn write_config_with_tools(dir: &Path) {
        fs::create_dir_all(dir).unwrap();
        let cfg = r#"
ai:
  provider: openai
  openai_api_key: test-key
  openai_model: test-model
default_prompt:
  tools:
    - name: echo
      config: "echo tool"
    - name: rm
      config: "remove files"
    - name: forced
      config: "forced explanation tool"
      force_explain: true
"#;
        fs::write(dir.join("config.yaml"), cfg).unwrap();
    }

    fn run_with_output(cli: Cli, input: &[u8], command: &str) -> (RunSummary, bool, String) {
        let temp = TempDir::new().unwrap();
        let config_root = temp.path().join("config");
        let _guard = set_config_dir_override_for_tests(&config_root);
        write_config_with_tools(&config_root);

        let generator = StubGenerator::new(command, "explanation");
        let executor = RecordingExecutor::default();
        let mut reader = Cursor::new(input.to_vec());
        let mut output = Vec::new();
        let summary = run_with_reader_and_confirmation_output(
            cli,
            &generator,
            &executor,
            &mut reader,
            &mut output,
        )
        .unwrap();
        (
            summary,
            executor.ran(),
            String::from_utf8(output).unwrap(),
        )
    }

    #[test]
    fn explain_source_resolves_effective_precedence() {
        assert_eq!(
            ExplainSource::resolve(SafetyMode::Unrestricted, true, true, "forced"),
            ExplainSource::UnrestrictedMode,
            "mandatory unrestricted inspection must outrank all optional sources"
        );
        assert_eq!(
            ExplainSource::resolve(SafetyMode::Default, true, true, "forced"),
            ExplainSource::Flag,
            "an explicit flag must outrank a tool setting"
        );
        assert_eq!(
            ExplainSource::resolve(SafetyMode::Default, false, true, "forced"),
            ExplainSource::ToolConfig("forced".to_string())
        );
        assert_eq!(
            ExplainSource::resolve(SafetyMode::Default, false, false, "echo"),
            ExplainSource::None
        );
    }

    #[test]
    fn preflight_card_renders_every_applicable_field_and_multiple_markers() {
        let explain_source = ExplainSource::ToolConfig("forced".to_string());
        let markers = vec![
            RiskMarker {
                kind: RiskKind::Operator,
                detail: "contains |".to_string(),
            },
            RiskMarker {
                kind: RiskKind::Destructive,
                detail: "rm — recursive and forced deletion".to_string(),
            },
        ];
        let card = PreflightCard {
            prompt: "clean generated files",
            command: "rm -rf target | tee cleanup.log",
            primary_tool: "rm",
            scope_hint: Some("./target"),
            safety_mode: SafetyMode::Unsafe,
            explain_source: &explain_source,
            risk_markers: &markers,
            config_provenance: PromptConfigProvenance::PerCall(Path::new("prompt.yml")),
        };

        let rendered = card.render();
        for expected in [
            "Preflight:",
            "Prompt:  clean generated files",
            "Command: rm -rf target | tee cleanup.log",
            "Tool:    rm",
            "Scope:   ./target",
            "Safety:  unsafe",
            "Explain: tool config (forced: force_explain)",
            "Risk:    [shell operators] contains |",
            "         [destructive] rm — recursive and forced deletion",
            "Config:  per-call prompt config (prompt.yml)",
        ] {
            assert!(
                rendered.contains(expected),
                "card should contain {expected:?}:\n{rendered}"
            );
        }
    }

    #[test]
    fn preflight_card_omits_inapplicable_fields_and_states_no_risk() {
        let explain_source = ExplainSource::None;
        let card = PreflightCard {
            prompt: "say hi",
            command: "echo hello",
            primary_tool: "echo",
            scope_hint: None,
            safety_mode: SafetyMode::Default,
            explain_source: &explain_source,
            risk_markers: &[],
            config_provenance: PromptConfigProvenance::GlobalDefault(Path::new("config.yaml")),
        };

        let rendered = card.render();
        assert!(!rendered.contains("Scope:"));
        assert!(!rendered.contains("Explain:"));
        assert!(rendered.contains("Risk:    none found"));
        assert!(rendered.contains("Config:  global default (config.yaml)"));
    }

    #[test]
    fn preflight_card_never_truncates_the_command() {
        let explain_source = ExplainSource::Flag;
        let command = format!("echo {}", "x".repeat(240));
        let card = PreflightCard {
            prompt: "print a long value",
            command: &command,
            primary_tool: "echo",
            scope_hint: None,
            safety_mode: SafetyMode::Default,
            explain_source: &explain_source,
            risk_markers: &[],
            config_provenance: PromptConfigProvenance::GlobalDefault(Path::new("config.yaml")),
        };

        assert!(card.render().contains(&command));
    }

    #[test]
    fn card_precedes_each_confirmation_but_not_direct_execution() {
        let (_, ordinary_ran, ordinary_output) = run_with_output(
            Cli {
                confirm: true,
                arg1: Some("say hi".to_string()),
                ..Default::default()
            },
            b"y\n",
            "echo hello",
        );
        assert!(ordinary_ran);
        assert!(ordinary_output.starts_with("Preflight:\n"));
        assert!(ordinary_output.ends_with("Execute this command? [y/N] "));

        let (_, unrestricted_ran, unrestricted_output) = run_with_output(
            Cli {
                unrestricted: true,
                arg1: Some("search for hello".to_string()),
                ..Default::default()
            },
            b"yes\n",
            "ripgrep hello",
        );
        assert!(unrestricted_ran);
        assert!(unrestricted_output.starts_with("Preflight:\n"));
        let card = unrestricted_output.find("Preflight:").unwrap();
        let announcement = unrestricted_output.find("UNRESTRICTED:").unwrap();
        let prompt = unrestricted_output.find("Type 'yes' to execute: ").unwrap();
        assert!(card < announcement && announcement < prompt);

        let (_, direct_ran, direct_output) = run_with_output(
            Cli {
                arg1: Some("say hi".to_string()),
                ..Default::default()
            },
            b"",
            "echo hello",
        );
        assert!(direct_ran);
        assert!(direct_output.is_empty(), "direct runs must not build or print a card");
    }

    #[test]
    fn explanation_is_completed_before_the_card_is_written() {
        let temp = TempDir::new().unwrap();
        let config_root = temp.path().join("config");
        let _guard = set_config_dir_override_for_tests(&config_root);
        write_minimal_config(&config_root);

        let explained = Cell::new(false);
        let generator = ExplanationTrackingGenerator {
            explained: &explained,
        };
        let executor = RecordingExecutor::default();
        let mut reader = Cursor::new(b"n\n".to_vec());
        let mut output = AfterExplanationWriter {
            explained: &explained,
            bytes: Vec::new(),
        };
        run_with_reader_and_confirmation_output(
            Cli {
                explain: true,
                arg1: Some("say hi".to_string()),
                ..Default::default()
            },
            &generator,
            &executor,
            &mut reader,
            &mut output,
        )
        .unwrap();

        assert!(explained.get());
        assert!(String::from_utf8(output.bytes).unwrap().starts_with("Preflight:\n"));
    }

    #[test]
    fn force_explain_tool_is_named_as_the_effective_source() {
        let (summary, ran, output) = run_with_output(
            Cli {
                arg1: Some("perform forced action".to_string()),
                ..Default::default()
            },
            b"n\n",
            "forced action",
        );

        assert!(summary.explain && summary.confirm);
        assert!(!ran);
        assert!(output.contains("Explain: tool config (forced: force_explain)"));
    }

    #[test]
    fn risk_markers_appear_on_unsafe_and_ordinary_confirmations() {
        let (_, _, unsafe_output) = run_with_output(
            Cli {
                unsafe_mode: true,
                arg1: Some("pipe two commands".to_string()),
                ..Default::default()
            },
            b"n\n",
            "echo first | echo second",
        );
        assert!(unsafe_output.contains("Safety:  unsafe"));
        assert!(unsafe_output.contains("[shell operators] contains |"));

        let (_, _, ordinary_output) = run_with_output(
            Cli {
                confirm: true,
                arg1: Some("remove generated files".to_string()),
                ..Default::default()
            },
            b"n\n",
            "rm -rf target",
        );
        assert!(ordinary_output.contains("Safety:  default"));
        assert!(ordinary_output.contains("[destructive] rm — recursive and forced deletion"));
    }

    #[test]
    fn card_scope_is_present_only_when_supplied() {
        let (_, _, with_scope) = run_with_output(
            Cli {
                confirm: true,
                scope: Some("./logs".to_string()),
                arg1: Some("say hi".to_string()),
                ..Default::default()
            },
            b"n\n",
            "echo hello",
        );
        assert!(with_scope.contains("Scope:   ./logs"));

        let (_, _, without_scope) = run_with_output(
            Cli {
                confirm: true,
                arg1: Some("say hi".to_string()),
                ..Default::default()
            },
            b"n\n",
            "echo hello",
        );
        assert!(!without_scope.contains("Scope:"));
    }

    #[test]
    fn card_reports_global_and_per_call_config_provenance() {
        let (_, _, global_output) = run_with_output(
            Cli {
                confirm: true,
                arg1: Some("say hi".to_string()),
                ..Default::default()
            },
            b"n\n",
            "echo hello",
        );
        assert!(global_output.contains("Config:  global default ("));
        assert!(global_output.contains("config.yaml)"));

        let temp = TempDir::new().unwrap();
        let config_root = temp.path().join("config");
        let _guard = set_config_dir_override_for_tests(&config_root);
        write_minimal_config(&config_root);
        let prompt_path = temp.path().join("prompt.yml");
        fs::write(
            &prompt_path,
            "tools:\n  - name: echo\n    config: \"echo tool\"\n",
        )
        .unwrap();

        let cli = Cli {
            confirm: true,
            prompt_config: Some(prompt_path.to_string_lossy().to_string()),
            arg1: Some("say hi".to_string()),
            ..Default::default()
        };
        let generator = StubGenerator::new("echo hello", "explanation");
        let executor = RecordingExecutor::default();
        let mut reader = Cursor::new(b"n\n".to_vec());
        let mut output = Vec::new();
        run_with_reader_and_confirmation_output(
            cli,
            &generator,
            &executor,
            &mut reader,
            &mut output,
        )
        .unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains(&format!(
            "Config:  per-call prompt config ({})",
            prompt_path.display()
        )));
    }

    #[test]
    fn card_preserves_executor_exit_code_and_execution_history_fields() {
        let temp = TempDir::new().unwrap();
        let config_root = temp.path().join("config");
        let _guard = set_config_dir_override_for_tests(&config_root);
        write_minimal_config(&config_root);
        let sample_path = temp.path().join("sample.log");
        fs::write(&sample_path, "hello\n").unwrap();

        let cli = Cli {
            confirm: true,
            scope: Some("./logs".to_string()),
            peek: vec![sample_path.to_string_lossy().to_string()],
            arg1: Some("say hi".to_string()),
            ..Default::default()
        };
        let generator = StubGenerator::new("echo hello", "explanation");
        let executor = RecordingExecutor::with_status(23);
        let mut reader = Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();
        let summary = run_with_reader_and_confirmation_output(
            cli.clone(),
            &generator,
            &executor,
            &mut reader,
            &mut output,
        )
        .unwrap();

        assert!(executor.ran());
        assert_eq!(summary.exit_code, 23);
        assert_eq!(summary.generated_command.as_deref(), Some("echo hello"));
        assert!(summary.confirm);
        assert!(!summary.explain);

        let entry = history_entry_for_run(
            &cli,
            vec!["sai".to_string(), "--confirm".to_string()],
            Path::new("/work"),
            summary.exit_code,
            Some(&summary),
            summary.notes.clone(),
        );
        history::write_entry(entry.clone()).unwrap();
        let stored = history::read_latest_entry().unwrap().unwrap();
        assert_eq!(stored, entry);
        assert_eq!(stored.exit_code, 23);
        assert_eq!(stored.generated_command.as_deref(), Some("echo hello"));
        assert_eq!(stored.scope.as_deref(), Some("./logs"));
        assert_eq!(stored.peek_files, vec![sample_path.to_string_lossy().to_string()]);
    }

    #[test]
    fn rendering_a_card_is_deterministic_and_has_no_execution_dependencies() {
        let model_calls = Cell::new(0);
        let execution_calls = Cell::new(0);
        let explain_source = ExplainSource::None;
        let card = PreflightCard {
            prompt: "say hi",
            command: "echo hello",
            primary_tool: "echo",
            scope_hint: None,
            safety_mode: SafetyMode::Default,
            explain_source: &explain_source,
            risk_markers: &[],
            config_provenance: PromptConfigProvenance::GlobalDefault(Path::new("config.yaml")),
        };

        assert_eq!(card.render(), card.render());
        assert_eq!(model_calls.get(), 0);
        assert_eq!(execution_calls.get(), 0);
    }

    #[test]
    fn preflight_cards_are_legible_in_each_confirmation_mode() {
        let cases = [
            (
                "--confirm",
                Cli {
                    confirm: true,
                    arg1: Some("say hi".to_string()),
                    ..Default::default()
                },
                "echo hello",
            ),
            (
                "--explain",
                Cli {
                    explain: true,
                    arg1: Some("say hi".to_string()),
                    ..Default::default()
                },
                "echo hello",
            ),
            (
                "--unsafe",
                Cli {
                    unsafe_mode: true,
                    arg1: Some("pipe greetings".to_string()),
                    ..Default::default()
                },
                "echo hello | echo goodbye",
            ),
            (
                "--unrestricted",
                Cli {
                    unrestricted: true,
                    arg1: Some("search freely".to_string()),
                    ..Default::default()
                },
                "ripgrep hello",
            ),
        ];

        for (mode, cli, command) in cases {
            let (_, _, output) = run_with_output(cli, b"n\n", command);
            assert!(output.starts_with("Preflight:\n"), "{mode}:\n{output}");
            assert!(output.contains(command), "{mode} truncated its command:\n{output}");
            assert!(!output.contains('\u{1b}'), "{mode} emitted terminal-only styling");
            assert!(
                output.lines().count() <= 11,
                "{mode} card is not compact:\n{output}"
            );
            println!("{mode}\n{output}\n");
        }
    }

    #[test]
    fn analyze_without_history_returns_message() {
        let temp = TempDir::new().unwrap();
        let config_root = temp.path().join("config");
        let _guard = set_config_dir_override_for_tests(&config_root);
        write_minimal_config(&config_root);

        let cli = Cli {
            analyze: true,
            ..Default::default()
        };

        let generator = StubGenerator::new("echo hi", "analysis");
        let executor = RecordingExecutor::default();
        let mut reader = Cursor::new(Vec::<u8>::new());
        let summary = run_with_reader(cli, &generator, &executor, &mut reader).unwrap();

        assert_eq!(summary.exit_code, 2);
        assert!(!summary.confirm);
        assert!(!executor.ran());
    }

    #[test]
    fn explain_forces_confirmation_and_allows_cancel() {
        let temp = TempDir::new().unwrap();
        let config_root = temp.path().join("config");
        let _guard = set_config_dir_override_for_tests(&config_root);
        write_minimal_config(&config_root);

        let cli = Cli {
            explain: true,
            arg1: Some("say hi".to_string()),
            ..Default::default()
        };

        let generator = StubGenerator::new("echo hello", "will echo hello");
        let executor = RecordingExecutor::default();
        let mut reader = Cursor::new(b"n\n".to_vec());
        let summary = run_with_reader(cli, &generator, &executor, &mut reader).unwrap();

        assert_eq!(summary.exit_code, 0);
        assert_eq!(summary.notes.as_deref(), Some("cancelled"));
        assert!(summary.confirm);
        assert!(!executor.ran());
    }

    /// Run the same prompt through a given CLI shape and report the summary.
    fn run_with(cli: Cli, input: &[u8]) -> (RunSummary, bool) {
        let temp = TempDir::new().unwrap();
        let config_root = temp.path().join("config");
        let _guard = set_config_dir_override_for_tests(&config_root);
        write_minimal_config(&config_root);

        let generator = StubGenerator::new("echo hello", "explanation");
        let executor = RecordingExecutor::default();
        let mut reader = Cursor::new(input.to_vec());
        let summary = run_with_reader(cli, &generator, &executor, &mut reader).unwrap();
        let ran = executor.ran();
        (summary, ran)
    }

    #[test]
    fn argument_and_piped_prompts_take_the_same_path() {
        let (from_argument, argument_ran) = run_with(
            Cli {
                arg1: Some("say hi".to_string()),
                ..Default::default()
            },
            b"",
        );

        // Same prompt, delivered on stdin instead of as an argument.
        let (from_stdin, stdin_ran) = run_with(
            Cli {
                no_interactive: true,
                ..Default::default()
            },
            b"say hi\n",
        );

        assert_eq!(from_argument.exit_code, from_stdin.exit_code);
        assert_eq!(
            from_argument.generated_command,
            from_stdin.generated_command
        );
        assert_eq!(from_argument.confirm, from_stdin.confirm);
        assert_eq!(from_argument.explain, from_stdin.explain);
        assert_eq!(from_argument.notes, from_stdin.notes);
        assert!(argument_ran && stdin_ran);
    }

    #[test]
    fn a_submitted_prompt_is_recorded_in_prompt_history() {
        let temp = TempDir::new().unwrap();
        let config_root = temp.path().join("config");
        let _guard = set_config_dir_override_for_tests(&config_root);
        write_minimal_config(&config_root);

        let cli = Cli {
            arg1: Some("say hi".to_string()),
            ..Default::default()
        };
        let generator = StubGenerator::new("echo hello", "explanation");
        let executor = RecordingExecutor::default();
        let mut reader = Cursor::new(Vec::<u8>::new());
        run_with_reader(cli, &generator, &executor, &mut reader).unwrap();

        assert_eq!(prompt_history::load(), vec!["say hi".to_string()]);
    }

    #[test]
    fn explain_and_unsafe_still_apply_to_a_stdin_prompt() {
        let (summary, ran) = run_with(
            Cli {
                no_interactive: true,
                explain: true,
                ..Default::default()
            },
            b"say hi\nn\n",
        );

        assert!(summary.explain, "explain must survive the input mode");
        assert!(summary.confirm, "explain must still force confirmation");
        assert_eq!(summary.notes.as_deref(), Some("cancelled"));
        assert!(!ran, "declining the confirmation must not execute");
    }

    #[test]
    fn unsafe_mode_still_forces_confirmation_for_a_stdin_prompt() {
        let (summary, ran) = run_with(
            Cli {
                no_interactive: true,
                unsafe_mode: true,
                ..Default::default()
            },
            b"say hi\nn\n",
        );

        assert!(summary.confirm);
        assert!(summary.unsafe_mode);
        assert!(!ran);
    }

    #[test]
    fn missing_prompt_outside_a_terminal_is_an_explicit_error() {
        let temp = TempDir::new().unwrap();
        let config_root = temp.path().join("config");
        let _guard = set_config_dir_override_for_tests(&config_root);
        write_minimal_config(&config_root);

        let cli = Cli {
            no_interactive: true,
            ..Default::default()
        };
        let generator = StubGenerator::new("echo hello", "explanation");
        let executor = RecordingExecutor::default();
        let mut reader = Cursor::new(Vec::<u8>::new());
        let err = run_with_reader(cli, &generator, &executor, &mut reader)
            .expect_err("an absent prompt must fail loudly");

        assert!(err.to_string().contains("No prompt provided"));
        assert!(!executor.ran());
    }

    #[test]
    fn prompt_config_flag_selects_the_per_call_config() {
        let temp = TempDir::new().unwrap();
        let config_root = temp.path().join("config");
        let _guard = set_config_dir_override_for_tests(&config_root);
        write_minimal_config(&config_root);

        let cfg_path = temp.path().join("jq-prompt.yaml");
        fs::write(
            &cfg_path,
            "tools:\n  - name: echo\n    config: \"echo tool from per-call config\"\n",
        )
        .unwrap();

        let cli = Cli {
            prompt_config: Some(cfg_path.to_string_lossy().to_string()),
            arg1: Some("say hi".to_string()),
            ..Default::default()
        };
        let generator = StubGenerator::new("echo hello", "explanation");
        let executor = RecordingExecutor::default();
        let mut reader = Cursor::new(Vec::<u8>::new());
        let summary = run_with_reader(cli, &generator, &executor, &mut reader).unwrap();

        assert_eq!(summary.exit_code, 0);
        assert!(executor.ran());
    }

    // --- unrestricted mode: configuration gate ------------------------------

    /// A generator that fails loudly if anything asks it for a command, so a
    /// test can prove the model was never contacted.
    struct NeverCalledGenerator;

    impl CommandGenerator for NeverCalledGenerator {
        fn generate(
            &self,
            _ai: &crate::config::EffectiveAiConfig,
            _system_prompt: &str,
            _nl_prompt: &str,
            _scope_hint: Option<&str>,
            _peek_text: Option<&str>,
        ) -> Result<String> {
            panic!("the model must not be contacted when the mode is forbidden");
        }
    }

    impl ChatClient for NeverCalledGenerator {
        fn respond(
            &self,
            _ai: &crate::config::EffectiveAiConfig,
            _system_prompt: &str,
            _user_prompt: &str,
            _temperature: f32,
        ) -> Result<String> {
            panic!("the model must not be contacted when the mode is forbidden");
        }
    }

    fn write_config_forbidding_unrestricted(dir: &Path) {
        fs::create_dir_all(dir).unwrap();
        let cfg = r#"
ai:
  provider: openai
  openai_api_key: test-key
  openai_model: test-model
safety:
  allow_unrestricted: false
default_prompt:
  tools:
    - name: echo
      config: "echo tool"
"#;
        fs::write(dir.join("config.yaml"), cfg).unwrap();
    }

    #[test]
    fn forbidden_unrestricted_mode_fails_and_names_the_config_file() {
        let temp = TempDir::new().unwrap();
        let config_root = temp.path().join("config");
        let _guard = set_config_dir_override_for_tests(&config_root);
        write_config_forbidding_unrestricted(&config_root);

        let cli = Cli {
            unrestricted: true,
            arg1: Some("say hi".to_string()),
            ..Default::default()
        };
        let executor = RecordingExecutor::default();
        let mut reader = Cursor::new(Vec::<u8>::new());
        let err = run_with_reader(cli, &NeverCalledGenerator, &executor, &mut reader)
            .expect_err("a forbidden mode must fail");

        let msg = err.to_string();
        assert!(msg.contains("Unrestricted mode is disabled"));
        assert!(
            msg.contains(&config_root.join("config.yaml").display().to_string()),
            "the error should name the config file that forbade it: {}",
            msg
        );
        assert!(!executor.ran());
    }

    #[test]
    fn forbidding_the_mode_does_not_affect_ordinary_invocations() {
        let temp = TempDir::new().unwrap();
        let config_root = temp.path().join("config");
        let _guard = set_config_dir_override_for_tests(&config_root);
        write_config_forbidding_unrestricted(&config_root);

        let cli = Cli {
            arg1: Some("say hi".to_string()),
            ..Default::default()
        };
        let generator = StubGenerator::new("echo hello", "explanation");
        let executor = RecordingExecutor::default();
        let mut reader = Cursor::new(Vec::<u8>::new());
        let summary = run_with_reader(cli, &generator, &executor, &mut reader).unwrap();

        assert_eq!(summary.exit_code, 0);
        assert!(executor.ran());
    }

    #[test]
    fn an_absent_setting_allows_unrestricted_mode() {
        let temp = TempDir::new().unwrap();
        let config_root = temp.path().join("config");
        let _guard = set_config_dir_override_for_tests(&config_root);
        // write_minimal_config has no `safety:` section at all.
        write_minimal_config(&config_root);

        let cli = Cli {
            unrestricted: true,
            arg1: Some("say hi".to_string()),
            ..Default::default()
        };
        let generator = StubGenerator::new("echo hello", "explanation");
        let executor = RecordingExecutor::default();
        let mut reader = Cursor::new(b"yes\n".to_vec());
        let summary = run_with_reader(cli, &generator, &executor, &mut reader).unwrap();

        assert_eq!(summary.exit_code, 0);
        assert!(executor.ran());
    }

    // --- unrestricted mode: mandatory inspection ----------------------------

    /// Run an unrestricted invocation with `input` as the confirmation answer.
    fn run_unrestricted(input: &[u8]) -> (RunSummary, bool) {
        let temp = TempDir::new().unwrap();
        let config_root = temp.path().join("config");
        let _guard = set_config_dir_override_for_tests(&config_root);
        write_minimal_config(&config_root);

        let cli = Cli {
            unrestricted: true,
            arg1: Some("say hi".to_string()),
            ..Default::default()
        };
        // A tool that is deliberately not in the config.
        let generator = StubGenerator::new("ripgrep hello", "it searches for hello");
        let executor = RecordingExecutor::default();
        let mut reader = Cursor::new(input.to_vec());
        let summary = run_with_reader(cli, &generator, &executor, &mut reader).unwrap();
        let ran = executor.ran();
        (summary, ran)
    }

    #[test]
    fn typing_yes_executes() {
        let (summary, ran) = run_unrestricted(b"yes\n");
        assert!(ran, "'yes' must execute the command");
        assert_eq!(summary.exit_code, 0);
    }

    #[test]
    fn a_bare_y_does_not_execute() {
        let (summary, ran) = run_unrestricted(b"y\n");
        assert!(
            !ran,
            "a bare 'y' must not clear the unrestricted confirmation"
        );
        assert_eq!(summary.notes.as_deref(), Some("cancelled"));
        assert_eq!(summary.exit_code, 0);
    }

    #[test]
    fn other_answers_do_not_execute() {
        for answer in [
            &b"n\n"[..],
            &b"\n"[..],
            &b"yep\n"[..],
            &b"YES please\n"[..],
            &b""[..],
        ] {
            let (_, ran) = run_unrestricted(answer);
            assert!(!ran, "answer {:?} must not execute", answer);
        }
    }

    #[test]
    fn the_affirmative_is_case_insensitive() {
        for answer in [&b"YES\n"[..], &b"Yes\n"[..]] {
            let (_, ran) = run_unrestricted(answer);
            assert!(ran, "answer {:?} should execute", answer);
        }
    }

    #[test]
    fn an_unconfigured_tool_runs_under_unrestricted() {
        // The whole point: `ripgrep` is not in the config and is not rejected.
        let (summary, ran) = run_unrestricted(b"yes\n");
        assert_eq!(summary.generated_command.as_deref(), Some("ripgrep hello"));
        assert!(ran);
    }

    #[test]
    fn inspection_happens_without_explain_or_confirm_flags() {
        let (summary, _) = run_unrestricted(b"yes\n");
        assert!(
            summary.explain,
            "explanation must be forced even without --explain"
        );
        assert!(
            summary.confirm,
            "confirmation must be forced even without --confirm"
        );
    }

    #[test]
    fn ordinary_confirmations_still_accept_a_bare_y() {
        let temp = TempDir::new().unwrap();
        let config_root = temp.path().join("config");
        let _guard = set_config_dir_override_for_tests(&config_root);
        write_minimal_config(&config_root);

        let cli = Cli {
            confirm: true,
            arg1: Some("say hi".to_string()),
            ..Default::default()
        };
        let generator = StubGenerator::new("echo hello", "explanation");
        let executor = RecordingExecutor::default();
        let mut reader = Cursor::new(b"y\n".to_vec());
        run_with_reader(cli, &generator, &executor, &mut reader).unwrap();

        assert!(
            executor.ran(),
            "a bare 'y' must still clear an ordinary confirmation"
        );
    }
}
