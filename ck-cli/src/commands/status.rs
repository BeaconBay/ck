use super::{Command, CommandContext};
use crate::progress::StatusReporter;
use anyhow::Result;
use console::style;
use std::path::PathBuf;

pub struct StatusCommand {
    pub path: PathBuf,
    pub verbose: bool,
    pub context: CommandContext,
}

impl StatusCommand {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            verbose: false,
            context: CommandContext::default(),
        }
    }
}

#[async_trait::async_trait]
impl Command for StatusCommand {
    fn name(&self) -> &'static str {
        "status"
    }

    async fn execute(&self) -> Result<()> {
        let status = StatusReporter::new(self.context.verbose);
        status.section_header("Index Status");

        let check_spinner = status.create_spinner("Reading index...");
        let stats = ck_index::get_index_stats(&self.path)?;
        check_spinner.finish_and_clear();

        if stats.total_files == 0 {
            status.info("No index found. Run 'ck --index' to create one.");
            return Ok(());
        }

        status.success(&format!(
            "Index contains {} files ({} chunks)",
            style(stats.total_files).green(),
            style(stats.total_chunks).green()
        ));

        if self.verbose {
            status.info(&format!("Index size: {:.2} MB", stats.index_size_bytes as f64 / 1_048_576.0));
            status.info(&format!("Last updated: {:?}", stats.last_modified));

            if !stats.orphaned_files.is_empty() {
                status.warn(&format!(
                    "Found {} orphaned sidecar files",
                    stats.orphaned_files.len()
                ));

                if stats.orphaned_files.len() <= 10 {
                    for file in &stats.orphaned_files {
                        println!("  • {}", file.display());
                    }
                } else {
                    for file in stats.orphaned_files.iter().take(10) {
                        println!("  • {}", file.display());
                    }
                    println!("  ... and {} more", stats.orphaned_files.len() - 10);
                }

                status.info("Run 'ck --clean-orphans' to remove orphaned files");
            }
        }

        Ok(())
    }
}