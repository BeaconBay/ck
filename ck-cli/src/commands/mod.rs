pub mod index;
pub mod search;
pub mod status;
pub mod clean;
pub mod inspect;

use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;

#[async_trait]
pub trait Command {
    async fn execute(&self) -> Result<()>;
    fn name(&self) -> &'static str;
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct CommandContext {
    pub verbose: bool,
    pub quiet: bool,
    pub no_progress: bool,
    pub working_dir: PathBuf,
}

impl Default for CommandContext {
    fn default() -> Self {
        Self {
            verbose: false,
            quiet: false,
            no_progress: false,
            working_dir: PathBuf::from("."),
        }
    }
}