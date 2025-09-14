use super::{Command, CommandContext};
use crate::progress::StatusReporter;
use anyhow::Result;
use console::style;
use std::path::PathBuf;

pub struct CleanCommand {
    pub path: PathBuf,
    pub orphans_only: bool,
    pub context: CommandContext,
}

impl CleanCommand {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            orphans_only: false,
            context: CommandContext::default(),
        }
    }

    pub fn orphans_only(mut self) -> Self {
        self.orphans_only = true;
        self
    }
}

#[async_trait::async_trait]
impl Command for CleanCommand {
    fn name(&self) -> &'static str {
        if self.orphans_only {
            "clean-orphans"
        } else {
            "clean"
        }
    }

    async fn execute(&self) -> Result<()> {
        let status = StatusReporter::new(self.context.verbose);

        if self.orphans_only {
            status.section_header("Cleaning Orphaned Files");
            status.info(&format!("Scanning for orphans in {}", self.path.display()));

            let clean_spinner = status.create_spinner("Removing orphaned sidecar files...");
            let removed = ck_index::clean_orphaned_sidecars(&self.path)?;
            clean_spinner.finish_and_clear();

            if removed > 0 {
                status.success(&format!(
                    "✅ Removed {} orphaned sidecar files",
                    style(removed).green()
                ));
            } else {
                status.info("No orphaned files found");
            }
        } else {
            status.section_header("Cleaning Index");
            status.warning(&format!(
                "This will remove the entire index for {}",
                self.path.display()
            ));

            if !self.context.quiet {
                use std::io::{self, Write};
                print!("Continue? [y/N] ");
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;

                if !input.trim().eq_ignore_ascii_case("y") {
                    status.info("Cancelled");
                    return Ok(());
                }
            }

            let clean_spinner = status.create_spinner("Removing index...");
            ck_index::clean_index(&self.path)?;
            clean_spinner.finish_and_clear();

            status.success("✅ Index cleaned successfully");
        }

        Ok(())
    }
}