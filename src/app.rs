use crate::cli::Cli;
use crate::config::{
    find_global_config_path, load_global_config, load_prompt_config, resolve_ai_config,
};
use crate::executor::{CommandExecutor, ShellCommandExecutor};
use crate::llm::{CommandGenerator, HttpCommandGenerator};
use crate::ops;
use crate::peek::build_peek_context;
use crate::prompt::build_system_prompt;
use crate::safety::validate_and_split_command;
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let generator = HttpCommandGenerator::new();
    let executor = ShellCommandExecutor;
    let status = run_with_dependencies(cli, &generator, &executor)?;
    std::process::exit(status);
}

pub fn run_with_dependencies<G, E>(cli: Cli, generator: &G, executor: &E) -> Result<i32>
where
    G: CommandGenerator,
    E: CommandExecutor,
{
    let global_config_path = find_global_config_path();

    if cli.init {
        ops::init_global_config(&global_config_path)?;
        return Ok(0);
    }

    if let Some(values) = cli.create_prompt.as_ref() {
        ops::create_prompt_template(values)?;
        return Ok(0);
    }

    if let Some(path) = cli.add_prompt.as_ref() {
        ops::add_prompt_to_global(&global_config_path, Path::new(path))?;
        return Ok(0);
    }

    if cli.list_tools {
        ops::list_tools(&global_config_path, cli.arg1.as_deref())?;
        return Ok(0);
    }

    let arg1 = cli.arg1.clone().ok_or_else(|| {
        anyhow!("Expected a prompt or prompt config path when not running with --init")
    })?;

    let global_cfg = load_global_config(&global_config_path)?;

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

    let need_confirm = cli.confirm || cli.unsafe_mode;
    if need_confirm
        && !confirm(
            &global_config_path,
            prompt_source.as_deref(),
            &nl_prompt,
            cli.scope.as_deref(),
            &cmd_line,
        )?
    {
        eprintln!("Cancelled.");
        return Ok(0);
    }

    let status = executor.execute(&cmd_line, &tokens, cli.unsafe_mode)?;
    Ok(status)
}

fn confirm(
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
    io::stdin().read_line(&mut buf)?;
    let ans = buf.trim().to_lowercase();
    Ok(ans == "y" || ans == "yes")
}
