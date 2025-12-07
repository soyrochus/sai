use crate::cli::Cli;
use crate::config::{
    find_global_config_path, load_global_config, load_prompt_config, resolve_ai_config,
};
use crate::executor::{CommandExecutor, ShellCommandExecutor};
use crate::history::{self, HistoryEntry};
use crate::llm::{ChatClient, CommandGenerator, HttpCommandGenerator};
use crate::ops;
use crate::peek::build_peek_context;
use crate::prompt::build_system_prompt;
use crate::safety::validate_and_split_command;
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use std::env;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RunSummary {
    pub exit_code: i32,
    pub generated_command: Option<String>,
    pub unsafe_mode: bool,
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
            unsafe_mode: cli.unsafe_mode,
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
            confirm: false,
            explain: false,
            scope: None,
            peek_files: Vec::new(),
            notes: None,
        }
    }
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let generator = HttpCommandGenerator::new();
    let executor = ShellCommandExecutor;
    let exit_code = run_and_log(cli, &generator, &executor);
    std::process::exit(exit_code);
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

    let (confirm, explain, unsafe_mode, scope, peek_files, generated_command) =
        if let Some(ref s) = summary {
            (
                s.confirm,
                s.explain,
                s.unsafe_mode,
                s.scope.clone(),
                s.peek_files.clone(),
                s.generated_command.clone(),
            )
        } else {
            (
                cli.confirm || cli.unsafe_mode || cli.explain,
                cli.explain,
                cli.unsafe_mode,
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

    if cli.analyze {
        return run_analyze(&global_cfg, generator);
    }

    let arg1 = cli.arg1.clone().ok_or_else(|| {
        anyhow!("Expected a prompt or prompt config path when not running with --init")
    })?;

    let (prompt_cfg, prompt_source): (crate::config::PromptConfig, Option<PathBuf>) =
        match cli.prompt.as_ref() {
            Some(_nl_prompt) => {
                let cfg_path = PathBuf::from(&arg1);
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

    let nl_prompt = cli.prompt.clone().unwrap_or_else(|| arg1.clone());

    let (system_prompt, allowed_tools) = build_system_prompt(&prompt_cfg)?;
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

    let tokens = validate_and_split_command(&cmd_line, &allowed_tools, cli.unsafe_mode)?;

    let mut summary = RunSummary::from_cli(&cli);
    summary.generated_command = Some(cmd_line.clone());

    if cli.explain {
        print_command_explanation(generator, &effective_ai, &cmd_line)?;
    }

    if summary.confirm
        && !confirm(
            reader,
            &global_config_path,
            prompt_source.as_deref(),
            &nl_prompt,
            cli.scope.as_deref(),
            &cmd_line,
        )?
    {
        eprintln!("Cancelled.");
        summary.exit_code = 0;
        summary.notes = Some("cancelled".to_string());
        return Ok(summary);
    }

    let status = executor.execute(&cmd_line, &tokens, cli.unsafe_mode)?;
    summary.exit_code = status;
    Ok(summary)
}

fn confirm(
    reader: &mut dyn BufRead,
    global_cfg_path: &Path,
    prompt_cfg_path: Option<&Path>,
    nl_prompt: &str,
    scope_hint: Option<&str>,
    cmd_line: &str,
) -> Result<bool> {
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

    eprint!("Execute this command? [y/N] ");
    io::stdout().flush().ok();
    let mut buf = String::new();
    reader.read_line(&mut buf)?;
    let ans = buf.trim().to_lowercase();
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
            init: false,
            create_prompt: None,
            add_prompt: None,
            list_tools: false,
            analyze: true,
            confirm: false,
            explain: false,
            unsafe_mode: false,
            peek: Vec::new(),
            scope: None,
            arg1: None,
            prompt: None,
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
            init: false,
            create_prompt: None,
            add_prompt: None,
            list_tools: false,
            analyze: false,
            confirm: false,
            explain: true,
            unsafe_mode: false,
            peek: Vec::new(),
            scope: None,
            arg1: Some("say hi".to_string()),
            prompt: None,
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
}
