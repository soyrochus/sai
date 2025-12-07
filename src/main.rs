mod app;
mod cli;
mod config;
mod executor;
mod help;
mod history;
mod llm;
mod ops;
mod peek;
mod prompt;
mod safety;
mod scope;

fn main() -> anyhow::Result<()> {
    app::run()
}
