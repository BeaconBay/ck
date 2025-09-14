use crate::commands::{
    index::IndexCommand,
    search::SearchCommand,
    status::StatusCommand,
    clean::CleanCommand,
    inspect::InspectCommand,
    Command,
    CommandContext,
};
use anyhow::Result;
use clap::Parser;
use ck_core::{SearchMode, SearchOptions};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ck")]
#[command(about = "Semantic grep by embedding - seek code, semantically")]
#[command(version)]
pub struct Cli {
    pub pattern: Option<String>,

    #[arg(help = "Files or directories to search")]
    pub files: Vec<PathBuf>,

    #[arg(long, help = "Create or update semantic index")]
    pub index: bool,

    #[arg(long, help = "Force rebuild index from scratch")]
    pub reindex: bool,

    #[arg(long, help = "Check index status")]
    pub status: bool,

    #[arg(long, help = "Show detailed index statistics")]
    pub status_verbose: bool,

    #[arg(long, help = "Clean entire index")]
    pub clean: bool,

    #[arg(long, help = "Clean orphaned sidecar files only")]
    pub clean_orphans: bool,

    #[arg(long, help = "Add single file to existing index")]
    pub add: Option<PathBuf>,

    #[arg(long, help = "Inspect file chunking")]
    pub inspect: bool,

    #[arg(long, help = "Download model for offline use")]
    pub download_model: Option<String>,

    #[arg(long, help = "Use offline mode (no downloads)")]
    pub offline: bool,

    #[arg(long, help = "Retry failed downloads")]
    pub retry_downloads: bool,

    #[arg(short = 'n', long = "line-number", help = "Show line numbers")]
    pub line_numbers: bool,

    #[arg(long = "no-filename", help = "Suppress filenames in output")]
    pub no_filenames: bool,

    #[arg(short = 'H', help = "Always print filenames")]
    pub with_filenames: bool,

    #[arg(short = 'C', long = "context", help = "Show N lines of context")]
    pub context: Option<usize>,

    #[arg(short = 'A', long = "after-context", help = "Show N lines after match")]
    pub after_context: Option<usize>,

    #[arg(short = 'B', long = "before-context", help = "Show N lines before match")]
    pub before_context: Option<usize>,

    #[arg(short = 'r', short_alias = 'R', long, help = "Search recursively")]
    pub recursive: bool,

    #[arg(short = 'i', long = "ignore-case", help = "Case-insensitive search")]
    pub ignore_case: bool,

    #[arg(short = 'F', long = "fixed-strings", help = "Treat pattern as literal")]
    pub fixed_strings: bool,

    #[arg(short = 'w', long = "word-regexp", help = "Match whole words only")]
    pub word_regexp: bool,

    #[arg(short = 'l', long = "files-with-matches", help = "List matching files only")]
    pub files_with_matches: bool,

    #[arg(short = 'L', long = "files-without-matches", help = "List non-matching files")]
    pub files_without_matches: bool,

    #[arg(long = "regex", help = "Use regex search (default)")]
    pub regex: bool,

    #[arg(long = "lex", help = "Use lexical (BM25) search")]
    pub lex: bool,

    #[arg(long = "sem", help = "Use semantic search")]
    pub sem: bool,

    #[arg(long = "hybrid", help = "Use hybrid search")]
    pub hybrid: bool,

    #[arg(long = "json", help = "Output as JSON")]
    pub json: bool,

    #[arg(long = "jsonl", help = "Output as JSONL")]
    pub jsonl: bool,

    #[arg(long = "topk", help = "Return top K results")]
    pub topk: Option<usize>,

    #[arg(long = "limit", help = "Alias for --topk")]
    pub limit: Option<usize>,

    #[arg(long = "threshold", help = "Minimum similarity score")]
    pub threshold: Option<f32>,

    #[arg(long = "scores", help = "Show similarity scores")]
    pub scores: bool,

    #[arg(long = "full-section", help = "Return complete code sections")]
    pub full_section: bool,

    #[arg(long = "no-snippet", help = "Don't include snippets in JSON")]
    pub no_snippet: bool,

    #[arg(long = "exclude", help = "Exclude patterns")]
    pub exclude: Vec<String>,

    #[arg(long = "no-default-excludes", help = "Don't use default exclusions")]
    pub no_default_excludes: bool,

    #[arg(long = "no-ignore", help = "Don't respect .gitignore")]
    pub no_ignore: bool,

    #[arg(long = "model", help = "Embedding model to use")]
    pub model: Option<String>,

    #[arg(long = "rerank", help = "Enable result reranking")]
    pub rerank: bool,

    #[arg(long = "rerank-model", help = "Reranking model to use")]
    pub rerank_model: Option<String>,

    #[arg(short = 'v', long = "verbose", help = "Verbose output")]
    pub verbose: bool,

    #[arg(short = 'q', long = "quiet", help = "Quiet mode")]
    pub quiet: bool,

    #[arg(long = "no-progress", help = "Disable progress bars")]
    pub no_progress: bool,
}

pub struct CommandDispatcher {
    cli: Cli,
}

impl CommandDispatcher {
    pub fn new(cli: Cli) -> Self {
        Self { cli }
    }

    pub async fn dispatch(&self) -> Result<()> {
        let context = CommandContext {
            verbose: self.cli.verbose,
            quiet: self.cli.quiet,
            no_progress: self.cli.no_progress,
            working_dir: std::env::current_dir()?,
        };

        if let Some(model) = &self.cli.download_model {
            return self.download_model_command(model, context).await;
        }

        if self.cli.index || self.cli.reindex {
            return self.index_command(context).await;
        }

        if self.cli.status || self.cli.status_verbose {
            return self.status_command(context).await;
        }

        if self.cli.clean || self.cli.clean_orphans {
            return self.clean_command(context).await;
        }

        if let Some(ref file) = self.cli.add {
            return self.add_file_command(file, context).await;
        }

        if self.cli.inspect {
            return self.inspect_command(context).await;
        }

        if let Some(ref pattern) = self.cli.pattern {
            return self.search_command(pattern, context).await;
        }

        anyhow::bail!("No command specified. Use --help for usage information.");
    }

    async fn index_command(&self, context: CommandContext) -> Result<()> {
        let path = self.cli.files.first().cloned().unwrap_or_else(|| PathBuf::from("."));

        let mut cmd = IndexCommand::new(path);
        cmd.context = context;
        cmd.model = self.cli.model.clone();
        cmd.exclude_patterns = self.cli.exclude.clone();
        cmd.force_rebuild = self.cli.reindex;

        if self.cli.offline {
            cmd.max_retries = 0;
        }

        cmd.validate()?;
        cmd.execute().await
    }

    async fn search_command(&self, pattern: &str, context: CommandContext) -> Result<()> {
        let paths = if self.cli.files.is_empty() {
            vec![PathBuf::from(".")]
        } else {
            self.cli.files.clone()
        };

        let mode = if self.cli.sem {
            SearchMode::Semantic
        } else if self.cli.lex {
            SearchMode::Lexical
        } else if self.cli.hybrid {
            SearchMode::Hybrid
        } else {
            SearchMode::Regex
        };

        let topk = self.cli.topk.or(self.cli.limit);

        let options = SearchOptions {
            line_numbers: self.cli.line_numbers,
            no_filename: self.cli.no_filenames,
            with_filename: self.cli.with_filenames,
            context: self.cli.context,
            before_context: self.cli.before_context,
            after_context: self.cli.after_context,
            recursive: self.cli.recursive,
            ignore_case: self.cli.ignore_case,
            fixed_strings: self.cli.fixed_strings,
            word_regexp: self.cli.word_regexp,
            files_with_matches: self.cli.files_with_matches,
            files_without_matches: self.cli.files_without_matches,
            json: self.cli.json,
            jsonl: self.cli.jsonl,
            topk,
            threshold: self.cli.threshold,
            show_scores: self.cli.scores,
            full_section: self.cli.full_section,
            no_snippet: self.cli.no_snippet,
            exclude: self.cli.exclude.clone(),
            no_default_excludes: self.cli.no_default_excludes,
            no_ignore: self.cli.no_ignore,
            rerank: self.cli.rerank,
            rerank_model: self.cli.rerank_model.clone(),
        };

        let mut cmd = SearchCommand::new(pattern.to_string(), paths);
        cmd.mode = mode;
        cmd.options = options;
        cmd.context = context;

        cmd.validate()?;
        cmd.execute().await
    }

    async fn status_command(&self, context: CommandContext) -> Result<()> {
        let path = self.cli.files.first().cloned().unwrap_or_else(|| PathBuf::from("."));

        let mut cmd = StatusCommand::new(path);
        cmd.verbose = self.cli.status_verbose;
        cmd.context = context;

        cmd.execute().await
    }

    async fn clean_command(&self, context: CommandContext) -> Result<()> {
        let path = self.cli.files.first().cloned().unwrap_or_else(|| PathBuf::from("."));

        let mut cmd = CleanCommand::new(path);
        if self.cli.clean_orphans {
            cmd = cmd.orphans_only();
        }
        cmd.context = context;

        cmd.execute().await
    }

    async fn inspect_command(&self, context: CommandContext) -> Result<()> {
        let file_path = self.cli.files.first().cloned().ok_or_else(|| {
            anyhow::anyhow!("--inspect requires a file path")
        })?;

        let mut cmd = InspectCommand::new(file_path);
        if let Some(ref model) = self.cli.model {
            cmd = cmd.with_model(model.clone());
        }
        cmd.context = context;

        cmd.validate()?;
        cmd.execute().await
    }

    async fn add_file_command(&self, file: &PathBuf, context: CommandContext) -> Result<()> {
        let mut cmd = IndexCommand::new(file.clone());
        cmd.context = context;
        cmd.model = self.cli.model.clone();

        cmd.validate()?;
        cmd.execute().await
    }

    async fn download_model_command(&self, model: &str, context: CommandContext) -> Result<()> {
        use ck_embed::{ModelDownloader, ModelDownloadConfig};

        let config = ModelDownloadConfig {
            offline_mode: false,
            verbose: !context.quiet,
            ..Default::default()
        };

        let downloader = ModelDownloader::new(config);

        let progress_callback = if !context.no_progress {
            Some(Box::new(|msg: &str| {
                eprintln!("{}", msg);
            }) as Box<dyn Fn(&str) + Send + Sync>)
        } else {
            None
        };

        match downloader.download_with_retry(model, progress_callback).await {
            Ok(path) => {
                eprintln!("✅ Model downloaded to: {}", path.display());
                Ok(())
            }
            Err(e) => {
                eprintln!("❌ Failed to download model: {}", e);
                Err(e)
            }
        }
    }
}