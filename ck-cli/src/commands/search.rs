use super::{Command, CommandContext};
use anyhow::Result;
use ck_core::{SearchMode, SearchOptions};
use ck_search::{SearchResult, SearchSummary};
use console::style;
use owo_colors::{OwoColorize, Rgb};
use std::path::PathBuf;

pub struct SearchCommand {
    pub pattern: String,
    pub paths: Vec<PathBuf>,
    pub mode: SearchMode,
    pub options: SearchOptions,
    pub context: CommandContext,
}

impl SearchCommand {
    pub fn new(pattern: String, paths: Vec<PathBuf>) -> Self {
        Self {
            pattern,
            paths,
            mode: SearchMode::Regex,
            options: SearchOptions::default(),
            context: CommandContext::default(),
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

    fn highlight_match(&self, text: &str, pattern: &str) -> String {
        if self.mode == SearchMode::Regex {
            if let Ok(re) = regex::Regex::new(pattern) {
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

    fn print_summary(&self, summary: &SearchSummary) {
        if !self.context.quiet {
            if summary.total_matches == 0 {
                eprintln!("No matches found");

                if let Some(ref closest) = summary.closest_below_threshold {
                    eprintln!();
                    eprintln!("{}", style("(nearest match beneath the threshold)").dim());
                    eprintln!("{}", self.format_result(closest));
                }
            } else if self.context.verbose {
                eprintln!(
                    "Found {} matches in {} files (searched {} files in {:.2}s)",
                    summary.total_matches,
                    summary.files_with_matches,
                    summary.files_searched,
                    summary.search_duration.as_secs_f64()
                );
            }
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
        let mut total_summary = SearchSummary::default();

        for path in &search_paths {
            let results = ck_search::search(
                &self.pattern,
                path,
                self.mode,
                self.options.clone(),
            ).await?;

            for result in results.results {
                if self.options.files_with_matches {
                    println!("{}", result.file);
                } else if !self.options.files_without_matches {
                    println!("{}", self.format_result(&result));
                }
                all_results.push(result);
            }

            total_summary.merge(&results.summary);
        }

        self.print_summary(&total_summary);

        let exit_code = if total_summary.total_matches > 0 { 0 } else { 1 };
        std::process::exit(exit_code);
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