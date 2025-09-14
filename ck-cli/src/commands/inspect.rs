use super::{Command, CommandContext};
use crate::progress::StatusReporter;
use anyhow::Result;
use console::style;
use std::path::PathBuf;

pub struct InspectCommand {
    pub file_path: PathBuf,
    pub model: Option<String>,
    pub context: CommandContext,
}

impl InspectCommand {
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            model: None,
            context: CommandContext::default(),
        }
    }

    pub fn with_model(mut self, model: String) -> Self {
        self.model = Some(model);
        self
    }
}

#[async_trait::async_trait]
impl Command for InspectCommand {
    fn name(&self) -> &'static str {
        "inspect"
    }

    async fn execute(&self) -> Result<()> {
        let status = StatusReporter::new(self.context.verbose);
        status.section_header("File Inspection");

        if !self.file_path.exists() {
            anyhow::bail!("File not found: {}", self.file_path.display());
        }

        if !self.file_path.is_file() {
            anyhow::bail!("Not a file: {}", self.file_path.display());
        }

        let content = std::fs::read_to_string(&self.file_path)?;
        let line_count = content.lines().count();
        let byte_count = content.len();

        let model_name = self.model.as_deref().unwrap_or("nomic-embed-text-v1.5");

        let tokenizer = ck_embed::TokenEstimator::new(model_name)?;
        let token_count = tokenizer.estimate_tokens(&content);

        println!(
            "{}",
            style(format!(
                "File: {} ({:.1} KB, {} lines, {} tokens)",
                self.file_path.display(),
                byte_count as f64 / 1024.0,
                line_count,
                token_count
            ))
            .bold()
        );

        let lang = ck_chunk::detect_language(&self.file_path);
        println!("Language: {}", style(lang.unwrap_or("unknown")).cyan());

        let chunks = ck_chunk::chunk_file_content(
            &content,
            &self.file_path,
            ck_models::get_model_chunk_config(model_name),
        )?;

        if chunks.is_empty() {
            println!("No chunks generated (file may be empty or binary)");
            return Ok(());
        }

        let token_counts: Vec<usize> = chunks
            .iter()
            .map(|c| tokenizer.estimate_tokens(&c.text))
            .collect();

        let min_tokens = *token_counts.iter().min().unwrap_or(&0);
        let max_tokens = *token_counts.iter().max().unwrap_or(&0);
        let avg_tokens = if !token_counts.is_empty() {
            token_counts.iter().sum::<usize>() as f64 / token_counts.len() as f64
        } else {
            0.0
        };

        println!(
            "\n{}",
            style(format!(
                "Chunks: {} (tokens: min={}, max={}, avg={:.0})",
                chunks.len(),
                min_tokens,
                max_tokens,
                avg_tokens
            ))
            .bold()
        );

        for (i, (chunk, tokens)) in chunks.iter().zip(token_counts.iter()).enumerate() {
            let preview = chunk
                .text
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("")
                .chars()
                .take(60)
                .collect::<String>()
                .trim()
                .to_string();

            let chunk_type = if let Some(ref sym) = chunk.symbol {
                format!("{}: ", sym.kind)
            } else {
                "text: ".to_string()
            };

            println!(
                "  {}. {}{} tokens | L{}-{} | {}...",
                style(i + 1).green().bold(),
                style(chunk_type).yellow(),
                style(tokens).cyan(),
                chunk.span.line_start,
                chunk.span.line_end,
                preview
            );
        }

        let parent_dir = self.file_path.parent().unwrap_or(Path::new("."));
        if let Ok(stats) = ck_index::get_index_stats(parent_dir) {
            if stats.total_files > 0 {
                println!(
                    "\nIndexed: {} files, {} chunks in directory",
                    style(stats.total_files).green(),
                    style(stats.total_chunks).green()
                );
            }
        }

        Ok(())
    }

    fn validate(&self) -> Result<()> {
        if !self.file_path.exists() {
            anyhow::bail!("File does not exist: {}", self.file_path.display());
        }
        Ok(())
    }
}