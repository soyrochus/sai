mod app;
mod cli;
mod config;
mod editor;
mod executor;
mod help;
mod history;
mod llm;
mod ops;
mod peek;
mod prompt;
mod prompt_history;
mod safety;
mod safety_mode;
mod scope;

fn main() -> anyhow::Result<()> {
    app::run()
}
