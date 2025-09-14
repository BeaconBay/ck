use anyhow::Result;
use clap::Parser;

mod commands;
mod dispatcher;
// mod error; // Temporarily disabled
mod progress;

use dispatcher::{Cli, CommandDispatcher};

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{}", e);

        let mut source = e.source();
        while let Some(err) = source {
            eprintln!("Caused by: {}", err);
            source = err.source();
        }

        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .init();

    let cli = Cli::parse();
    let dispatcher = CommandDispatcher::new(cli);
    dispatcher.dispatch().await
}