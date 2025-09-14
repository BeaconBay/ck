use super::{Command, CommandContext};
use anyhow::Result;
use ck_core::{SearchMode, SearchOptions};
use ck_core::SearchResult;
use console::style;
use owo_colors::OwoColorize;
use regex::Regex;
use std::path::PathBuf;

pub struct SearchCommand {
    pub pattern: String,
    pub paths: Vec<PathBuf>,
    pub mode: SearchMode,
    pub options: SearchOptions,
    pub context: CommandContext,
    compiled_regex: Option<Regex>,
}

impl SearchCommand {
    pub fn new(pattern: String, paths: Vec<PathBuf>) -> Self {
        let compiled_regex = if let Ok(re) = Regex::new(&pattern) {
            Some(re)
        } else {
            None
        };

        Self {
            pattern,
            paths,
            mode: SearchMode::Regex,
            options: SearchOptions::default(),
            context: CommandContext::default(),
            compiled_regex,
        }
    }

    pub fn with_mode(mut self, mode: SearchMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_options(mut self, options: SearchOptions) -> Self {
        self.options = options;
        self
    }

    fn format_result(&self, result: &SearchResult) -> String {
        let mut output = String::new();

        if self.options.show_scores {
            let score_text = format!("[{:.3}] ", result.score);
            output.push_str(&self.colorize_score(score_text, result.score));
        }

        if !self.options.no_filename {
            let file_text = if self.options.line_numbers {
                format!("{}:{}: ", result.file, result.line_start)
            } else {
                format!("{}: ", result.file)
            };
            output.push_str(&style(file_text).cyan().to_string());
        } else if self.options.line_numbers {
            output.push_str(&format!("{}: ", result.line_start));
        }

        output.push_str(&self.highlight_match(&result.preview, &self.pattern));
        output
    }

    fn colorize_score(&self, text: String, score: f32) -> String {
        let normalized = (score.clamp(0.0, 1.0) * 255.0) as u8;
        let red = 255 - normalized;
        let green = normalized;
        format!("{}", text.truecolor(red, green, 0))
    }

    fn highlight_match(&self, text: &str, _pattern: &str) -> String {
        if self.mode == SearchMode::Regex {
            if let Some(ref re) = self.compiled_regex {
                let mut result = String::new();
                let mut last_end = 0;

                for mat in re.find_iter(text) {
                    result.push_str(&text[last_end..mat.start()]);
                    result.push_str(&style(&text[mat.start()..mat.end()]).yellow().bold().to_string());
                    last_end = mat.end();
                }
                result.push_str(&text[last_end..]);
                return result;
            }
        }
        text.to_string()
    }

    fn print_summary(&self, matches: usize) {
        if !self.context.quiet && self.context.verbose {
            eprintln!("Found {} matches", matches);
        }
    }
}

#[async_trait::async_trait]
impl Command for SearchCommand {
    fn name(&self) -> &'static str {
        "search"
    }

    async fn execute(&self) -> Result<()> {
        let search_paths = if self.paths.is_empty() {
            vec![PathBuf::from(".")]
        } else {
            self.paths.clone()
        };

        if self.mode == SearchMode::Semantic && !self.context.quiet {
            let topk = self.options.topk.unwrap_or(10);
            let threshold = self.options.threshold.unwrap_or(0.6);

            eprintln!(
                "ℹ️  Semantic search: top {} results, threshold ≥{:.1}",
                topk, threshold
            );
        }

        let mut all_results = Vec::new();
        let mut total_matches = 0;

        for path in &search_paths {
            let results = ck_engine::search(&ck_core::SearchOptions {
                pattern: Some(self.pattern.clone()),
                paths: vec![path.clone()],
                mode: self.mode,
                recursive: self.options.recursive,
                line_numbers: self.options.line_numbers,
                topk: self.options.topk,
                threshold: self.options.threshold,
                ..Default::default()
            }).await?;

            for result in results {
                if self.options.files_with_matches {
                    println!("{}", result.file);
                } else if !self.options.files_without_matches {
                    println!("{}", self.format_result(&result));
                }
                total_matches += 1;
                all_results.push(result);
            }
        }

        if total_matches == 0 {
            if !self.context.quiet {
                eprintln!("No matches found");
            }
            anyhow::bail!("No matches found");
        }
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        if self.pattern.is_empty() {
            anyhow::bail!("Search pattern cannot be empty");
        }

        if self.options.files_with_matches && self.options.files_without_matches {
            anyhow::bail!("Cannot use both --files-with-matches and --files-without-matches");
        }

        Ok(())
    }
}