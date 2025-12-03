mod app;
mod cli;
mod config;
mod executor;
mod llm;
mod ops;
mod peek;
mod prompt;
mod safety;

fn main() -> anyhow::Result<()> {
    app::run()
}
