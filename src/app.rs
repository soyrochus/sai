use crate::cli::{resolve_prompt_config_path, resolve_prompt_source, Cli, PromptSource};
use crate::config::{
    find_global_config_path, load_global_config, load_prompt_config, resolve_ai_config,
};
use crate::executor::{CommandExecutor, ShellCommandExecutor};
use crate::help;
use crate::history::{self, HistoryEntry};
use crate::llm::{ChatClient, CommandGenerator, HttpCommandGenerator};
use crate::ops;
use crate::editor;
use crate::peek::build_peek_context;
use crate::prompt_history;
use crate::prompt::build_system_prompt;
use crate::safety::{risk_markers, validate_and_split_command};
use crate::safety_mode::SafetyMode;
use anyhow::{anyhow, Context, Result};
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

    let (confirm, explain, unsafe_mode, unrestricted, scope, peek_files, generated_command) =
        if let Some(ref s) = summary {
            (
                s.confirm,
                s.explain,
                s.unsafe_mode,
                s.unrestricted,
                s.scope.clone(),
                s.peek_files.clone(),
                s.generated_command.clone(),
            )
        } else {
            (
                cli.confirm || cli.unsafe_mode || cli.explain,
                cli.explain,
                SafetyMode::from_cli(&cli).is_unsafe_for_history(),
                cli.unrestricted,
                cli.scope.clone(),
                cli.peek.clone(),
                None,
            )
        };

    let entry = HistoryEntry {
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
    };

    if let Err(err) = history::write_entry(entry) {
        eprintln!("Warning: failed to write history: {:#}", err);
    }

    exit_code
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
    // Under an unrestricted run these are true unconditionally: no flag, config
    // value or per-tool setting is consulted, so nothing can suppress them.
    let effective_explain =
        safety_mode.forces_inspection() || cli.explain || tool_requires_explain;
    let effective_confirm =
        safety_mode.forces_inspection() || cli.confirm || cli.unsafe_mode || effective_explain;

    let mut summary = RunSummary::from_cli(&cli);
    summary.generated_command = Some(cmd_line.clone());
    summary.explain = effective_explain;
    summary.confirm = effective_confirm;

    if tool_requires_explain && !cli.explain {
        eprintln!("Note: This tool requires explanation mode (force_explain is enabled)");
        eprintln!();
    }

    if effective_explain {
        print_command_explanation(generator, &effective_ai, &cmd_line)?;
    }

    let confirmed = if !effective_confirm {
        true
    } else if safety_mode.is_unrestricted() {
        confirm_unrestricted(
            reader,
            &global_config_path,
            prompt_source.as_deref(),
            &nl_prompt,
            cli.scope.as_deref(),
            &cmd_line,
        )?
    } else {
        confirm(
            reader,
            &global_config_path,
            prompt_source.as_deref(),
            &nl_prompt,
            cli.scope.as_deref(),
            &cmd_line,
        )?
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
    io::stdout().flush().ok();
    let mut buf = String::new();
    reader.read_line(&mut buf)?;
    Ok(buf.trim().to_lowercase())
}

/// Print the shared header both confirmations show.
fn print_confirm_context(
    global_cfg_path: &Path,
    prompt_cfg_path: Option<&Path>,
    nl_prompt: &str,
    scope_hint: Option<&str>,
    cmd_line: &str,
) {
    eprintln!("Global config file: {}", global_cfg_path.display());
    if let Some(p) = prompt_cfg_path {
        eprintln!("Prompt config file: {}", p.display());
    } else {
        eprintln!("Prompt config: default_prompt from global config");
    }
    eprintln!();
    eprintln!("Natural language prompt:");
    eprintln!("  {}", nl_prompt);
    eprintln!();
    if let Some(scope) = scope_hint {
        eprintln!("Scope hint:");
        eprintln!("  {}", scope);
        eprintln!();
    }
    eprintln!("LLM output (command):");
    eprintln!("  {}", cmd_line);
    eprintln!();
}

/// The confirmation shown under `--unrestricted`.
///
/// Requires the full word `yes`. A bare `y` clears every other prompt in SAI
/// and deliberately does not clear this one: this is the mode where a wrong
/// command is unbounded, so muscle memory should not be enough.
fn confirm_unrestricted(
    reader: &mut dyn BufRead,
    global_cfg_path: &Path,
    prompt_cfg_path: Option<&Path>,
    nl_prompt: &str,
    scope_hint: Option<&str>,
    cmd_line: &str,
) -> Result<bool> {
    print_confirm_context(
        global_cfg_path,
        prompt_cfg_path,
        nl_prompt,
        scope_hint,
        cmd_line,
    );

    // Locally computed, unlike the explanation above, which came from the same
    // model that wrote the command.
    let markers = risk_markers(cmd_line);
    if !markers.is_empty() {
        eprintln!("Risk markers (computed locally, not by the model):");
        for marker in &markers {
            eprintln!("  [{}] {}", marker.kind.label(), marker.detail);
        }
        eprintln!();
    }

    eprintln!("UNRESTRICTED: no tool whitelist is in effect for this command.");
    eprint!("Type 'yes' to execute: ");

    Ok(read_answer(reader)? == "yes")
}

fn confirm(
    reader: &mut dyn BufRead,
    global_cfg_path: &Path,
    prompt_cfg_path: Option<&Path>,
    nl_prompt: &str,
    scope_hint: Option<&str>,
    cmd_line: &str,
) -> Result<bool> {
    print_confirm_context(
        global_cfg_path,
        prompt_cfg_path,
        nl_prompt,
        scope_hint,
        cmd_line,
    );

    eprint!("Execute this command? [y/N] ");
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

    #[derive(Default)]
    struct RecordingExecutor {
        ran: Cell<bool>,
    }

    impl RecordingExecutor {
        fn ran(&self) -> bool {
            self.ran.get()
        }
    }

    impl CommandExecutor for RecordingExecutor {
        fn execute(&self, _cmd_line: &str, _tokens: &[String], _unsafe_mode: bool) -> Result<i32> {
            self.ran.set(true);
            Ok(0)
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
        assert!(!ran, "a bare 'y' must not clear the unrestricted confirmation");
        assert_eq!(summary.notes.as_deref(), Some("cancelled"));
        assert_eq!(summary.exit_code, 0);
    }

    #[test]
    fn other_answers_do_not_execute() {
        for answer in [&b"n\n"[..], &b"\n"[..], &b"yep\n"[..], &b"YES please\n"[..], &b""[..]] {
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

        assert!(executor.ran(), "a bare 'y' must still clear an ordinary confirmation");
    }
}
