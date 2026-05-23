use anyhow::Result;
use ck_core::{
    IncludePattern, SearchMode, SearchOptions, get_default_ckignore_content,
    heatmap::{self, HeatmapBucket},
};
use clap::Parser;
use console::style;
use owo_colors::{OwoColorize, Rgb};
use regex::RegexBuilder;
use std::path::{Path, PathBuf};

mod mcp;
mod mcp_server;
mod path_utils;
mod progress;
// TUI is now in its own crate: ck-tui

use path_utils::{build_include_patterns, expand_glob_patterns};
use progress::StatusReporter;

#[derive(Parser)]
#[command(name = "ck")]
#[command(about = "Semantic grep by embedding - seek code, semantically")]
#[command(long_about = r#"
ck (seek) - A drop-in replacement for grep with semantic search capabilities

QUICK START EXAMPLES:

  Basic grep-style search (no indexing required):
    ck "error" src/                    # Find text matches
    ck -i "TODO" .                     # Case-insensitive search  
    ck -r "fn main" .                  # Recursive search
    ck -n "import" lib.py              # Show line numbers

  Semantic search (finds conceptually similar code):
    ck --sem "error handling" src/     # Builds/updates the index automatically (top 10, threshold ≥0.6)
    ck --sem "database connection"     # Find DB-related code  
    ck --sem --limit 5 "authentication"    # Limit to top 5 results
    ck --sem --threshold 0.8 "auth"   # Higher precision filtering

  Lexical search (BM25 full-text search):
    ck --lex "user authentication"    # Full-text search with ranking
    ck --lex "http client request"    # Better than regex for phrases

  Hybrid search (combines regex + semantic):  
    ck --hybrid "async function"      # Best of both worlds
    ck --hybrid "error" --limit 10    # Top 10 most relevant results (--limit is alias for --topk)
    ck --hybrid "bug" --threshold 0.02 # Only results with RRF score >= 0.02
    ck --sem "auth" --scores           # Show similarity scores in output

  Index management:
    ck --status .                     # Check index status
    ck --status-verbose .              # Detailed index statistics
    ck --clean-orphans .               # Clean up orphaned files
    ck --clean .                       # Remove entire index
    ck --switch-model nomic-v1.5       # Clean + rebuild with a different embedding model
    ck --add file.rs                   # Add single file to index
    ck --index .                       # Optional: pre-build before CI runs

  JSON output for tools/scripts:
    ck --json --sem "bug fix" src/    # Traditional JSON (single array)
    ck --json --limit 5 "TODO"       # Limit results (--limit alias for --topk)
    
  JSONL output for AI agents (recommended):
    ck --jsonl "auth" --no-snippet    # Streaming, memory-efficient format
    ck --jsonl --sem "error" src/     # Perfect for LLM/agent consumption
    ck --jsonl --topk 5 --threshold 0.8 "func"  # High-confidence agent results
    # Why JSONL? Streaming, error-resilient, standard in AI pipelines

  Advanced grep features:
    ck -C 2 "error" src/              # Show 2 lines of context  
    ck -A 3 -B 1 "TODO"              # 3 lines after, 1 before
    ck -w "test" .                    # Match whole words only
    ck -F "log.Error()" .             # Fixed string (no regex)

  Model and embedding options:
    ck --index --model nomic-v1.5      # Index with higher-quality model (8k context)
    ck --index --model jina-code       # Index with code-specialized model
    ck --sem "auth" --rerank           # Enable reranking for better relevance
    ck --sem "login" --rerank-model bge # Use specific reranking model

  AI agent integration (MCP):
    ck --serve                         # Start MCP server for Claude/Cursor integration
    # Provides tools: semantic_search, regex_search, hybrid_search, index_status, reindex, health_check
    # Connect with Claude Desktop, Cursor, or any MCP-compatible client

  SEARCH MODES:
  --regex   : Classic grep behavior (default, no index needed)
  --lex     : BM25 lexical search (auto-indexed before it runs)  
  --sem     : Semantic/embedding search (auto-indexed, defaults: top 10, threshold ≥0.6)
  --hybrid  : Combines regex and semantic (shares the auto-indexing path)

RESULT FILTERING:
  --topk, --limit N : Limit to top N results (default: 10 for semantic search)
  --threshold SCORE : Filter by minimum score (default: 0.6 for semantic search)
                      (0.0-1.0 semantic/lexical, 0.01-0.05 hybrid RRF)
  --scores          : Show scores in output [0.950] file:line:match

The semantic search understands meaning - searching for "error handling" 
will find try/catch blocks, error returns, exception handling, etc.
"#)]
#[command(version)]
struct Cli {
    pattern: Option<String>,

    #[arg(help = "Files or directories to search")]
    files: Vec<PathBuf>,

    #[arg(short = 'n', long = "line-number", help = "Show line numbers")]
    line_numbers: bool,

    #[arg(long = "no-filename", help = "Suppress filenames in output")]
    no_filenames: bool,

    #[arg(short = 'H', help = "Always print filenames")]
    with_filenames: bool,

    #[arg(
        short = 'l',
        long = "files-with-matches",
        help = "Print only names of files with matches"
    )]
    files_with_matches: bool,

    #[arg(
        short = 'L',
        long = "files-without-matches",
        help = "Print only names of files without matches"
    )]
    files_without_matches: bool,

    #[arg(short = 'i', long = "ignore-case", help = "Case insensitive search")]
    ignore_case: bool,

    #[arg(short = 'w', long = "word-regexp", help = "Match whole words only")]
    word_regexp: bool,

    #[arg(
        short = 'F',
        long = "fixed-strings",
        help = "Interpret pattern as fixed string"
    )]
    fixed_strings: bool,

    #[arg(
        short = 'R',
        short_alias = 'r',
        long = "recursive",
        help = "Recursively search directories"
    )]
    recursive: bool,

    #[arg(
        short = 'C',
        long = "context",
        value_name = "NUM",
        help = "Show NUM lines of context before and after"
    )]
    context: Option<usize>,

    #[arg(
        short = 'A',
        long = "after-context",
        value_name = "NUM",
        help = "Show NUM lines after match"
    )]
    after_context: Option<usize>,

    #[arg(
        short = 'B',
        long = "before-context",
        value_name = "NUM",
        help = "Show NUM lines before match"
    )]
    before_context: Option<usize>,

    #[arg(
        long = "sem",
        help = "Semantic search - finds conceptually similar code (defaults: top 10, threshold ≥0.6)"
    )]
    semantic: bool,

    #[arg(
        long = "lex",
        help = "Lexical search - BM25 full-text search with ranking"
    )]
    lexical: bool,

    #[arg(
        long = "hybrid",
        help = "Hybrid search - combines regex and semantic results"
    )]
    hybrid: bool,

    #[arg(long = "regex", help = "Regex search mode (default, grep-compatible)")]
    regex: bool,

    #[arg(
        long = "topk",
        alias = "limit",
        value_name = "N",
        help = "Limit results to top N matches (alias: --limit) [default: 10 for semantic search]"
    )]
    top_k: Option<usize>,

    #[arg(
        long = "threshold",
        value_name = "SCORE",
        help = "Minimum score threshold (0.0-1.0 for semantic/lexical, 0.01-0.05 for hybrid RRF) [default: 0.6 for semantic search]"
    )]
    threshold: Option<f32>,

    #[arg(long = "scores", help = "Show similarity scores in output")]
    show_scores: bool,

    #[arg(long = "json", help = "Output results as JSON for tools/scripts")]
    json: bool,

    #[arg(long = "json-v1", help = "Output results as JSON v1 schema")]
    json_v1: bool,

    #[arg(long = "jsonl", help = "Output results as JSONL for agent workflows")]
    jsonl: bool,

    #[arg(long = "no-snippet", help = "Exclude code snippets from JSONL output")]
    no_snippet: bool,

    #[arg(long = "reindex", help = "Force index update before searching")]
    reindex: bool,

    #[arg(
        long = "exclude",
        value_name = "PATTERN",
        help = "Exclude directories matching pattern (can be used multiple times)"
    )]
    exclude: Vec<String>,

    #[arg(
        long = "no-default-excludes",
        help = "Disable default directory exclusions (like .git, node_modules, etc.)"
    )]
    no_default_excludes: bool,

    #[arg(long = "no-ignore", help = "Don't respect .gitignore files")]
    no_ignore: bool,

    #[arg(long = "no-ckignore", help = "Don't respect .ckignore file")]
    no_ckignore: bool,

    #[arg(
        long = "print-default-ckignore",
        help = "Print the default .ckignore content that ck generates and exit"
    )]
    print_default_ckignore: bool,

    #[arg(
        long = "full-section",
        help = "Return complete code sections (functions/classes) instead of just matching lines. Uses tree-sitter to identify semantic boundaries. Supported: Python, JavaScript, TypeScript, Haskell, Rust, Ruby"
    )]
    full_section: bool,

    #[arg(
        short = 'q',
        long = "quiet",
        help = "Suppress status messages and progress indicators"
    )]
    quiet: bool,

    // Command flags (replacing subcommands)
    #[arg(
        long = "index",
        help = "Create or update search index for the specified path"
    )]
    index: bool,

    #[arg(long = "clean", help = "Clean up search index")]
    clean: bool,

    #[arg(long = "clean-orphans", help = "Clean only orphaned index files")]
    clean_orphans: bool,

    #[arg(
        long = "switch-model",
        value_name = "NAME",
        help = "Clean the existing index and rebuild it using the specified embedding model",
        conflicts_with_all = [
            "index",
            "clean",
            "clean_orphans",
            "status",
            "status_verbose",
            "add",
            "inspect"
        ],
        conflicts_with = "model"
    )]
    switch_model: Option<String>,

    #[arg(
        long = "force",
        help = "Force rebuilding when used with --switch-model",
        requires = "switch_model"
    )]
    force: bool,

    #[arg(long = "add", help = "Add a single file to the index")]
    add: bool,

    #[arg(long = "status", help = "Show index status and statistics")]
    status: bool,

    #[arg(long = "status-verbose", help = "Show detailed index statistics")]
    status_verbose: bool,

    #[arg(long = "status-json", help = "Output index status as JSON")]
    status_json: bool,

    #[arg(
        long = "inspect",
        help = "Show detailed metadata for a specific file (chunks, embeddings, tree-sitter parsing info)"
    )]
    inspect: bool,

    #[arg(
        long = "dump-chunks",
        help = "Visualize chunk boundaries for a file using the same rendering as TUI chunk mode"
    )]
    dump_chunks: bool,

    // Model selection (index-time only)
    #[arg(
        long = "model",
        value_name = "MODEL",
        help = "Embedding model to use for indexing (bge-small, nomic-v1.5, jina-code, mxbai-xsmall) [default: bge-small]. Only used with --index."
    )]
    model: Option<String>,

    // Search-time enhancement options
    #[arg(
        long = "rerank",
        help = "Enable reranking with cross-encoder model for improved relevance"
    )]
    rerank: bool,

    #[arg(
        long = "rerank-model",
        value_name = "MODEL",
        help = "Reranking model to use (jina, bge, mxbai) [default: jina]"
    )]
    rerank_model: Option<String>,

    // MCP Server mode
    #[arg(
        long = "serve",
        help = "Start MCP server mode for AI agent integration",
        conflicts_with_all = [
            "pattern", "files", "line_numbers", "no_filenames", "with_filenames",
            "files_with_matches", "files_without_matches", "ignore_case", "word_regexp",
            "fixed_strings", "recursive", "context", "after_context", "before_context",
            "semantic", "lexical", "hybrid", "regex", "top_k", "threshold", "show_scores",
            "json", "json_v1", "jsonl", "no_snippet", "reindex", "exclude", "no_default_excludes",
            "no_ignore", "full_section", "index", "clean", "clean_orphans", "switch_model",
            "force", "add", "status", "status_verbose", "inspect", "dump_chunks", "model", "rerank", "rerank_model", "tui"
        ]
    )]
    serve: bool,

    // TUI mode
    #[arg(
        long = "tui",
        help = "Interactive TUI mode - like fzf but semantic. Live search with arrow keys, Tab to switch modes, Enter to open in $EDITOR",
        conflicts_with_all = [
            "line_numbers", "no_filenames", "with_filenames",
            "files_with_matches", "files_without_matches", "ignore_case", "word_regexp",
            "fixed_strings", "recursive", "context", "after_context", "before_context",
            "semantic", "lexical", "hybrid", "regex", "top_k", "threshold", "show_scores",
            "json", "json_v1", "jsonl", "no_snippet", "reindex", "exclude", "no_default_excludes",
            "no_ignore", "full_section", "index", "clean", "clean_orphans", "switch_model",
            "force", "add", "status", "status_verbose", "inspect", "dump_chunks", "model", "rerank", "rerank_model", "serve"
        ]
    )]
    tui: bool,
}

fn canonicalize_for_comparison(path: &Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }

    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

fn find_search_root(include_patterns: &[IncludePattern]) -> PathBuf {
    if include_patterns.is_empty() {
        return PathBuf::from(".");
    }

    let mut root = if include_patterns[0].is_dir {
        include_patterns[0].path.clone()
    } else {
        include_patterns[0]
            .path
            .parent()
            .unwrap_or(&include_patterns[0].path)
            .to_path_buf()
    };

    for pattern in include_patterns.iter().skip(1) {
        let mut candidate = if pattern.is_dir {
            pattern.path.clone()
        } else {
            pattern.path.parent().unwrap_or(&pattern.path).to_path_buf()
        };

        if candidate.starts_with(&root) {
            continue;
        }

        while !root.starts_with(&candidate) && !candidate.starts_with(&root) {
            if let Some(parent) = root.parent() {
                root = parent.to_path_buf();
            } else {
                break;
            }
        }

        if !candidate.starts_with(&root) {
            while let Some(parent) = candidate.parent() {
                if parent.starts_with(&root) {
                    candidate = parent.to_path_buf();
                    break;
                }
                candidate = parent.to_path_buf();
            }
        }

        if root.starts_with(&candidate) {
            root = candidate;
        }
    }

    if root.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        root
    }
}

fn build_exclude_patterns(cli: &Cli) -> Vec<String> {
    // Use the centralized pattern builder from ck-core
    // Note: .ckignore handling is now done by WalkBuilder via the use_ckignore parameter
    ck_core::build_exclude_patterns(&cli.exclude, !cli.no_default_excludes)
}

async fn run_index_workflow(
    status: &StatusReporter,
    path: &Path,
    cli: &Cli,
    model_alias: &str,
    model_config: &ck_models::ModelConfig,
    heading: &str,
    clean_first: bool,
) -> Result<()> {
    status.section_header(heading);
    status.info(&format!("Scanning files in {}", path.display()));

    if model_alias == model_config.name {
        status.info(&format!(
            "🤖 Model: {} ({} dims)",
            model_config.name, model_config.dimensions
        ));
    } else {
        status.info(&format!(
            "🤖 Model: {} (alias '{}', {} dims)",
            model_config.name, model_alias, model_config.dimensions
        ));
    }

    let max_tokens = ck_chunk::TokenEstimator::get_model_limit(model_config.name.as_str());
    let (chunk_tokens, overlap_tokens) =
        ck_chunk::get_model_chunk_config(Some(model_config.name.as_str()));

    status.info(&format!("📏 FastEmbed Config: {} token limit", max_tokens));
    status.info(&format!(
        "📄 Chunk Config: {} tokens target, {} token overlap (~20%)",
        chunk_tokens, overlap_tokens
    ));

    // Create .ckignore file if it doesn't exist
    if !cli.no_ckignore
        && let Ok(created) = ck_core::create_ckignore_if_missing(path)
        && created
    {
        status.info("📄 Created .ckignore with default patterns");
    }

    let exclude_patterns = build_exclude_patterns(cli);

    if clean_first {
        let index_dir = path.join(".ck");
        if index_dir.exists() {
            let spinner = status.create_spinner("Removing existing index...");
            ck_index::clean_index(path)?;
            status.finish_progress(spinner, "Old index removed");
        } else {
            status.info("No existing index detected; creating a fresh one");
        }
    }

    let start_time = std::time::Instant::now();

    let (
        mut file_progress_bar,
        mut overall_progress_bar,
        progress_callback,
        detailed_progress_callback,
    ) = if !cli.quiet {
        use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

        let multi_progress = MultiProgress::new();

        let overall_pb = multi_progress.add(ProgressBar::new(0));
        overall_pb
            .set_style(
                ProgressStyle::default_bar()
                    .template(
                        "📂 Embedding Files: [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {msg}"
                    )
                    .unwrap()
                    .progress_chars("━━╸ "),
            );

        let file_pb = multi_progress.add(ProgressBar::new(0));
        file_pb
            .set_style(
                ProgressStyle::default_bar()
                    .template(
                        "📄 Embedding Chunks: [{elapsed_precise}] [{bar:40.green/yellow}] {pos}/{len} ({percent}%) {msg}"
                    )
                    .unwrap()
                    .progress_chars("━━╸ "),
            );

        let overall_pb_clone = overall_pb.clone();
        let overall_pb_clone2 = overall_pb.clone();
        let file_pb_clone2 = file_pb.clone();

        let progress_callback = Some(Box::new(move |file_name: &str| {
            let short_name = file_name.split('/').next_back().unwrap_or(file_name);
            overall_pb_clone.set_message(format!("Processing {}", short_name));
            overall_pb_clone.inc(1);
        }) as ck_index::ProgressCallback);

        let detailed_progress_callback =
            Some(Box::new(move |progress: ck_index::EmbeddingProgress| {
                if overall_pb_clone2.length().unwrap_or(0) != progress.total_files as u64 {
                    overall_pb_clone2.set_length(progress.total_files as u64);
                }
                overall_pb_clone2.set_position(progress.file_index as u64);

                if file_pb_clone2.length().unwrap_or(0) != progress.total_chunks as u64 {
                    file_pb_clone2.set_length(progress.total_chunks as u64);
                    file_pb_clone2.reset();
                }
                file_pb_clone2.set_position(progress.chunk_index as u64);

                let short_name = progress
                    .file_name
                    .split('/')
                    .next_back()
                    .unwrap_or(&progress.file_name);
                file_pb_clone2.set_message(format!(
                    "{} (chunk {}/{}, {}B)",
                    short_name,
                    progress.chunk_index + 1,
                    progress.total_chunks,
                    progress.chunk_size
                ));
            }) as ck_index::DetailedProgressCallback);

        (
            Some(file_pb),
            Some(overall_pb),
            progress_callback,
            detailed_progress_callback,
        )
    } else {
        (None, None, None, None)
    };

    let file_options = ck_core::FileCollectionOptions {
        respect_gitignore: !cli.no_ignore,
        use_ckignore: !cli.no_ckignore,
        exclude_patterns: exclude_patterns.clone(),
    };
    let index_future = ck_index::smart_update_index_with_detailed_progress(
        path,
        false,
        progress_callback,
        detailed_progress_callback,
        true,
        &file_options,
        Some(model_alias),
    );
    tokio::pin!(index_future);

    let stats = match tokio::select! {
        res = &mut index_future => res,
        _ = tokio::signal::ctrl_c() => {
            ck_index::request_interrupt();
            if let Some(pb) = file_progress_bar.take() {
                pb.finish_and_clear();
            }
            if let Some(pb) = overall_progress_bar.take() {
                pb.finish_with_message("⏹ Indexing interrupted");
            }
            status.warn("Indexing interrupted by user");
            match (&mut index_future).await {
                Ok(_) => return Ok(()),
                Err(err) => {
                    if err.to_string() == ck_index::INDEX_INTERRUPTED_MSG {
                        return Ok(());
                    }
                    return Err(err);
                }
            }
        }
    } {
        Ok(stats) => stats,
        Err(err) => {
            if let Some(pb) = file_progress_bar.take() {
                pb.finish_and_clear();
            }
            if let Some(pb) = overall_progress_bar.take() {
                pb.finish_and_clear();
            }
            return Err(err);
        }
    };

    let elapsed = start_time.elapsed();
    let files_per_sec = if elapsed.as_secs_f64() > 0.0 {
        stats.files_indexed as f64 / elapsed.as_secs_f64()
    } else {
        stats.files_indexed as f64
    };

    if let Some(file_pb) = file_progress_bar.take() {
        file_pb.finish_with_message("✅ All chunks processed");
    }
    if let Some(overall_pb) = overall_progress_bar.take() {
        overall_pb.finish_with_message(format!(
            "✅ Index built in {:.2}s ({:.1} files/sec)",
            elapsed.as_secs_f64(),
            files_per_sec
        ));
    }

    status.success(&format!("🚀 Indexed {} files", stats.files_indexed));
    if stats.files_added > 0 {
        status.info(&format!("  ➕ {} new files added", stats.files_added));
    }
    if stats.files_modified > 0 {
        status.info(&format!("  🔄 {} files updated", stats.files_modified));
    }
    if stats.files_up_to_date > 0 {
        status.info(&format!(
            "  ✅ {} files already current",
            stats.files_up_to_date
        ));
    }
    if stats.orphaned_files_removed > 0 {
        status.info(&format!(
            "  🧹 {} orphaned entries cleaned",
            stats.orphaned_files_removed
        ));
    }

    if clean_first {
        status.info(&format!(
            "  🔁 Active embedding model: {} (alias '{}', {} dims)",
            model_config.name, model_alias, model_config.dimensions
        ));
    }

    Ok(())
}

async fn dump_file_chunks(file_path: &PathBuf) -> Result<()> {
    use std::path::Path;

    let path = Path::new(file_path);

    // Use the shared live chunking function
    let (lines, chunk_metas) = ck_tui::chunk_file_live(path).map_err(|err| {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    })?;

    // Display chunks for entire file
    let display_lines = ck_tui::chunks::collect_chunk_display_lines(
        &lines,
        0,            // context_start
        lines.len(),  // context_end
        1,            // match_line (not relevant for dump)
        None,         // chunk_meta (None = show all chunks)
        &chunk_metas, // all_chunks
        true,         // full_file_mode
    );

    // Print header
    println!("File: {}", file_path.display());
    if let Some(lang) = ck_core::Language::from_path(path) {
        println!("Language: {}", lang);
    }
    println!("Chunks: {}", chunk_metas.len());

    // Debug: Show chunk type breakdown
    let text_chunk_count = chunk_metas
        .iter()
        .filter(|c| c.chunk_type.as_deref() == Some("text"))
        .count();
    println!("  - Text chunks: {}", text_chunk_count);
    println!(
        "  - Structural chunks: {}",
        chunk_metas.len() - text_chunk_count
    );
    println!();

    // Convert display lines to strings and print
    for line in display_lines {
        println!("{}", ck_tui::chunk_display_line_to_string(&line));
    }

    Ok(())
}

async fn inspect_file_metadata(file_path: &PathBuf, status: &StatusReporter) -> Result<()> {
    use ck_embed::TokenEstimator;
    use console::style;
    use std::fs;
    use std::path::Path;

    let path = Path::new(file_path);

    if !path.exists() {
        status.error("File does not exist");
        return Ok(());
    }

    let metadata = fs::metadata(path)?;
    let detected_lang = ck_core::Language::from_path(path);
    let content = fs::read_to_string(path)?;
    let total_tokens = TokenEstimator::estimate_tokens(&content);

    // Basic file info
    println!(
        "File: {} ({:.1} KB, {} lines, {} tokens)",
        style(path.display()).cyan().bold(),
        metadata.len() as f64 / 1024.0,
        content.lines().count(),
        style(total_tokens).yellow()
    );

    if let Some(lang) = detected_lang {
        println!("Language: {}", style(lang.to_string()).green());
    }

    // Use model-aware chunking
    let default_model = "nomic-embed-text-v1.5";
    let chunks = ck_chunk::chunk_text_with_model(&content, detected_lang, Some(default_model))?;

    if chunks.is_empty() {
        println!("No chunks generated");
        return Ok(());
    }

    // Token analysis
    let token_counts: Vec<usize> = chunks
        .iter()
        .map(|chunk| TokenEstimator::estimate_tokens(&chunk.text))
        .collect();

    let min_tokens = *token_counts.iter().min().unwrap();
    let max_tokens = *token_counts.iter().max().unwrap();
    let avg_tokens = token_counts.iter().sum::<usize>() as f64 / token_counts.len() as f64;

    println!(
        "\nChunks: {} (tokens: min={}, max={}, avg={:.0})",
        style(chunks.len()).green().bold(),
        style(min_tokens).cyan(),
        style(max_tokens).cyan(),
        style(avg_tokens as usize).cyan()
    );

    // Show chunk details (limit to 10)
    let display_limit = 10;
    for (i, chunk) in chunks.iter().take(display_limit).enumerate() {
        let chunk_tokens = token_counts[i];

        let type_display = match chunk.chunk_type {
            ck_chunk::ChunkType::Function => "func",
            ck_chunk::ChunkType::Class => "class",
            ck_chunk::ChunkType::Method => "method",
            ck_chunk::ChunkType::Module => "mod",
            ck_chunk::ChunkType::Text => "text",
        };

        let stride_display = chunk
            .stride_info
            .as_ref()
            .map(|stride| {
                format!(
                    " [stride {}/{}]",
                    stride.stride_index + 1,
                    stride.total_strides
                )
            })
            .unwrap_or_default();

        // Simple preview - first 80 chars
        let preview = chunk
            .text
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("")
            .chars()
            .take(80)
            .collect::<String>()
            .trim()
            .to_string();

        println!(
            "  {} {}{}: {} tokens | L{}-{} | {}{}",
            style(format!("{:2}.", i + 1)).dim(),
            style(type_display).blue(),
            stride_display,
            style(chunk_tokens).yellow(),
            chunk.span.line_start,
            chunk.span.line_end,
            preview,
            if chunk.text.len() > 80 { "..." } else { "" }
        );
    }

    if chunks.len() > display_limit {
        println!("  ... and {} more chunks", chunks.len() - display_limit);
    }

    // Index status
    let parent_dir = path.parent().unwrap_or(Path::new("."));
    if let Ok(stats) = ck_index::get_index_stats(parent_dir) {
        if stats.total_files > 0 {
            println!(
                "\nIndexed: {} files, {} chunks in directory",
                style(stats.total_files).green(),
                style(stats.total_chunks).green()
            );
        } else {
            println!("\nNot indexed. Run 'ck --index .' to enable semantic search");
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run_main().await {
        eprintln!("DETAILED ERROR: {:#}", e);
        eprintln!("DEBUG: Error occurred in main");

        // Print the error chain for better debugging
        let mut source = e.source();
        while let Some(err) = source {
            eprintln!("CAUSED BY: {}", err);
            source = err.source();
        }

        std::process::exit(1);
    }
}

async fn run_main() -> Result<()> {
    let cli = Cli::parse();

    if cli.print_default_ckignore {
        print!("{}", get_default_ckignore_content());
        return Ok(());
    }

    // Handle MCP server mode first
    if cli.serve {
        return run_mcp_server().await;
    }

    // Handle TUI mode
    if cli.tui {
        let search_path = cli
            .files
            .first()
            .cloned()
            .unwrap_or_else(|| PathBuf::from("."));
        let initial_query = cli.pattern.clone();
        return ck_tui::run_tui(search_path, initial_query).await;
    }

    // Regular CLI mode
    run_cli_mode(cli).await
}

async fn run_mcp_server() -> Result<()> {
    // Configure service-safe logging for MCP mode (no stdout pollution)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let cwd = std::env::current_dir()?;
    let server = mcp_server::CkMcpServer::new(cwd)?;
    server.run().await
}

async fn run_cli_mode(cli: Cli) -> Result<()> {
    // Regular CLI mode logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .init();

    let status = StatusReporter::new(cli.quiet);

    // Handle command flags first (these take precedence over search)
    if let Some(model_name) = cli.switch_model.as_deref() {
        let path = cli
            .files
            .first()
            .cloned()
            .unwrap_or_else(|| PathBuf::from("."));

        let registry = ck_models::ModelRegistry::default();
        let (model_alias, model_config) = registry
            .resolve(Some(model_name))
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        if !cli.force {
            let manifest_path = path.join(".ck").join("manifest.json");
            if manifest_path.exists()
                && let Ok(data) = std::fs::read(&manifest_path)
                && let Ok(manifest) = serde_json::from_slice::<ck_index::IndexManifest>(&data)
                && let Some(existing_model) = manifest.embedding_model.clone()
                && let Ok((existing_alias, existing_config)) =
                    registry.resolve(Some(existing_model.as_str()))
                && existing_config.name == model_config.name
            {
                status.section_header("Switching Embedding Model");
                let dims = manifest
                    .embedding_dimensions
                    .unwrap_or(existing_config.dimensions);

                if existing_alias == existing_config.name {
                    status.info(&format!(
                        "Index already uses {} ({} dims)",
                        existing_config.name, dims
                    ));
                } else {
                    status.info(&format!(
                        "Index already uses {} (alias '{}', {} dims)",
                        existing_config.name, existing_alias, dims
                    ));
                }

                status.success("No rebuild required; index already on requested model");
                status.info(&format!(
                    "Use '--switch-model {} --force' to rebuild anyway",
                    model_name
                ));
                return Ok(());
            }
        }

        run_index_workflow(
            &status,
            &path,
            &cli,
            model_alias.as_str(),
            &model_config,
            "Switching Embedding Model",
            true,
        )
        .await?;
        return Ok(());
    }

    if cli.index {
        let path = cli
            .files
            .first()
            .cloned()
            .unwrap_or_else(|| PathBuf::from("."));

        let registry = ck_models::ModelRegistry::default();
        let (model_alias, model_config) = registry
            .resolve(cli.model.as_deref())
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        run_index_workflow(
            &status,
            &path,
            &cli,
            model_alias.as_str(),
            &model_config,
            "Indexing Repository",
            false,
        )
        .await?;
        return Ok(());
    }

    if cli.clean || cli.clean_orphans {
        // Handle --clean and --clean-orphans flags
        let clean_path = cli
            .files
            .first()
            .cloned()
            .unwrap_or_else(|| PathBuf::from("."));
        let orphans_only = cli.clean_orphans;

        if orphans_only {
            status.section_header("Cleaning Orphaned Files");
            status.info(&format!("Scanning for orphans in {}", clean_path.display()));

            // Build exclusion patterns using unified builder
            let exclude_patterns = build_exclude_patterns(&cli);

            let cleanup_spinner = status.create_spinner("Removing orphaned entries...");
            let file_options = ck_core::FileCollectionOptions {
                respect_gitignore: !cli.no_ignore,
                use_ckignore: !cli.no_ckignore,
                exclude_patterns: exclude_patterns.clone(),
            };
            let cleanup_stats = ck_index::cleanup_index(&clean_path, &file_options)?;
            status.finish_progress(cleanup_spinner, "Cleanup complete");

            if cleanup_stats.orphaned_entries_removed > 0
                || cleanup_stats.orphaned_sidecars_removed > 0
            {
                status.success(&format!(
                    "Removed {} orphaned entries and {} orphaned sidecars",
                    cleanup_stats.orphaned_entries_removed, cleanup_stats.orphaned_sidecars_removed
                ));
            } else {
                status.info("No orphaned files found");
            }
        } else {
            status.section_header("Cleaning Index");
            status.warn(&format!(
                "Removing entire index for {}",
                clean_path.display()
            ));

            let clean_spinner = status.create_spinner("Removing index files...");
            ck_index::clean_index(&clean_path)?;
            status.finish_progress(clean_spinner, "Index removed");

            status.success("Index cleaned successfully");
        }
        return Ok(());
    }

    if cli.add {
        // Handle --add flag
        // When using --add, the file path might be in pattern or files
        let file = if let Some(ref pattern) = cli.pattern {
            // If pattern is provided and no files, use pattern as the file path
            if cli.files.is_empty() {
                PathBuf::from(pattern)
            } else {
                // Otherwise use the first file
                cli.files
                    .first()
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("No file specified. Usage: ck --add <file>"))?
            }
        } else {
            // No pattern, must be in files
            cli.files
                .first()
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("No file specified. Usage: ck --add <file>"))?
        };
        status.section_header("Adding File to Index");
        status.info(&format!("Processing {}", file.display()));

        let add_spinner = status.create_spinner("Updating index...");
        ck_index::index_file(&file, true).await?;
        status.finish_progress(add_spinner, "File indexed");

        status.success(&format!("Added {} to index", file.display()));
        return Ok(());
    }

    if cli.status || cli.status_verbose || cli.status_json {
        // Handle --status, --status-verbose, and --status-json flags
        let status_path = cli
            .files
            .first()
            .cloned()
            .unwrap_or_else(|| PathBuf::from("."));
        let verbose = cli.status_verbose;

        let stats = if cli.status_json {
            // For JSON output, skip spinner and human-readable messages
            ck_index::get_index_stats(&status_path)?
        } else {
            status.section_header("Index Status");
            let check_spinner = status.create_spinner("Reading index...");
            let stats = ck_index::get_index_stats(&status_path)?;
            status.finish_progress(check_spinner, "Status retrieved");
            stats
        };

        if cli.status_json {
            // Output JSON format
            let mut json_output = serde_json::json!({
                "path": status_path.to_string_lossy(),
                "index_exists": stats.total_files > 0,
                "total_files": stats.total_files,
                "total_chunks": stats.total_chunks,
                "embedded_chunks": stats.embedded_chunks,
                "total_size_bytes": stats.total_size_bytes,
                "index_size_bytes": stats.index_size_bytes,
                "index_created": stats.index_created,
                "index_updated": stats.index_updated,
            });

            // Add model information if available
            let manifest_path = status_path.join(".ck").join("manifest.json");
            if let Ok(data) = std::fs::read(&manifest_path)
                && let Ok(manifest) = serde_json::from_slice::<ck_index::IndexManifest>(&data)
                && let Some(model_name) = manifest.embedding_model
            {
                let registry = ck_models::ModelRegistry::default();
                let alias = registry
                    .models
                    .iter()
                    .find(|(_, config)| config.name == model_name)
                    .map(|(alias, _)| alias.clone())
                    .unwrap_or_else(|| model_name.clone());
                let dims = manifest
                    .embedding_dimensions
                    .or_else(|| {
                        registry
                            .models
                            .iter()
                            .find(|(_, config)| config.name == model_name)
                            .map(|(_, config)| config.dimensions)
                    })
                    .unwrap_or(0);

                json_output["model"] = serde_json::json!({
                    "name": model_name,
                    "alias": alias,
                    "dimensions": dims,
                });
            }

            println!("{}", serde_json::to_string_pretty(&json_output)?);
        } else if stats.total_files == 0 {
            status.warn(&format!("No index found at {}", status_path.display()));
            status.info("Run 'ck --index .' to create an index");
        } else {
            status.info(&format!("Index location: {}", status_path.display()));
            status.success(&format!("Files indexed: {}", stats.total_files));
            status.info(&format!("  Total chunks: {}", stats.total_chunks));
            status.info(&format!("  Embedded chunks: {}", stats.embedded_chunks));

            let manifest_path = status_path.join(".ck").join("manifest.json");
            if let Ok(data) = std::fs::read(&manifest_path)
                && let Ok(manifest) = serde_json::from_slice::<ck_index::IndexManifest>(&data)
                && let Some(model_name) = manifest.embedding_model
            {
                let registry = ck_models::ModelRegistry::default();
                let alias = registry
                    .models
                    .iter()
                    .find(|(_, config)| config.name == model_name)
                    .map(|(alias, _)| alias.clone())
                    .unwrap_or_else(|| model_name.clone());
                let dims = manifest
                    .embedding_dimensions
                    .or_else(|| {
                        registry
                            .models
                            .iter()
                            .find(|(_, config)| config.name == model_name)
                            .map(|(_, config)| config.dimensions)
                    })
                    .unwrap_or(0);

                if alias == model_name {
                    status.info(&format!("  Model: {} ({} dims)", model_name, dims));
                } else {
                    status.info(&format!(
                        "  Model: {} (alias '{}', {} dims)",
                        model_name, alias, dims
                    ));
                }
            }

            if verbose {
                let size_mb = stats.total_size_bytes as f64 / (1024.0 * 1024.0);
                let index_size_mb = stats.index_size_bytes as f64 / (1024.0 * 1024.0);
                status.info(&format!("  Source size: {:.1} MB", size_mb));
                status.info(&format!("  Index size: {:.1} MB", index_size_mb));

                use std::time::UNIX_EPOCH;
                if stats.index_created > 0
                    && let Some(created) =
                        UNIX_EPOCH.checked_add(std::time::Duration::from_secs(stats.index_created))
                    && let Ok(datetime) = created.elapsed()
                {
                    status.info(&format!(
                        "  Created: {:.1} hours ago",
                        datetime.as_secs() as f64 / 3600.0
                    ));
                }
                if stats.index_updated > 0
                    && let Some(updated) =
                        UNIX_EPOCH.checked_add(std::time::Duration::from_secs(stats.index_updated))
                    && let Ok(datetime) = updated.elapsed()
                {
                    status.info(&format!(
                        "  Updated: {:.1} hours ago",
                        datetime.as_secs() as f64 / 3600.0
                    ));
                }

                // Show compression ratio
                if stats.total_size_bytes > 0 {
                    let compression_ratio =
                        stats.index_size_bytes as f64 / stats.total_size_bytes as f64;
                    status.info(&format!(
                        "  Compression: {:.1}x ({:.1}%)",
                        1.0 / compression_ratio,
                        compression_ratio * 100.0
                    ));
                }
            }
        }
        return Ok(());
    }

    if cli.inspect {
        // Handle --inspect flag
        // For inspect, the file path could be in pattern or files
        let file_path = if let Some(pattern) = &cli.pattern {
            PathBuf::from(pattern)
        } else if !cli.files.is_empty() {
            cli.files[0].clone()
        } else {
            eprintln!("Error: --inspect requires a file path");
            std::process::exit(1);
        };

        status.section_header("File Inspection");

        // Inspect the file metadata
        inspect_file_metadata(&file_path, &status).await?;
        return Ok(());
    }

    if cli.dump_chunks {
        // Handle --dump-chunks flag
        let file_path = if let Some(pattern) = &cli.pattern {
            PathBuf::from(pattern)
        } else if !cli.files.is_empty() {
            cli.files[0].clone()
        } else {
            eprintln!("Error: --dump-chunks requires a file path");
            std::process::exit(1);
        };

        dump_file_chunks(&file_path).await?;
        return Ok(());
    }

    // Validate conflicting flags
    if cli.files_with_matches && cli.files_without_matches {
        eprintln!("Error: Cannot use -l and -L together");
        std::process::exit(1);
    }

    // Default behavior: search with pattern
    if let Some(ref pattern) = cli.pattern {
        let reindex = cli.reindex;

        // Determine repo root for .ckignore loading
        let repo_root_path = cli
            .files
            .first()
            .map(|p| {
                if p.is_dir() {
                    p.clone()
                } else {
                    p.parent().unwrap_or(p).to_path_buf()
                }
            })
            .unwrap_or_else(|| PathBuf::from("."));

        let repo_root = Some(repo_root_path.as_path());

        // Build options to get exclusion patterns
        let temp_options = build_options(&cli, reindex, repo_root);

        let expanded_targets = if cli.files.is_empty() {
            vec![PathBuf::from(".")]
        } else {
            expand_glob_patterns(&cli.files, &temp_options.exclude_patterns)?
        };

        let include_patterns = if cli.files.is_empty() {
            Vec::new()
        } else {
            build_include_patterns(&expanded_targets)
        };

        let mut search_root = if include_patterns.is_empty() {
            PathBuf::from(".")
        } else {
            find_search_root(&include_patterns)
        };

        if expanded_targets.len() == 1 && !expanded_targets[0].exists() {
            search_root = expanded_targets[0].clone();
        }

        let include_patterns = if include_patterns.len() > 1 {
            include_patterns
                .into_iter()
                .filter(|pattern| !(pattern.is_dir && pattern.path == search_root))
                .collect()
        } else {
            include_patterns
        };

        // Handle multiple files like grep; allow -h/-H overrides
        let mut show_filenames = if include_patterns.is_empty() {
            expanded_targets.len() > 1 || expanded_targets.iter().any(|p| p.is_dir())
        } else {
            include_patterns.len() > 1 || include_patterns.iter().any(|p| p.is_dir)
        };
        if cli.no_filenames {
            show_filenames = false;
        }
        if cli.with_filenames {
            show_filenames = true;
        }
        let mut options = build_options(&cli, reindex, repo_root);
        options.show_filenames = show_filenames;
        options.include_patterns = include_patterns.clone();
        options.path = search_root.clone();

        let summary = run_search(pattern.clone(), search_root, options, &status).await?;

        if cli.files_without_matches {
            let matched_canon: Vec<PathBuf> = summary
                .matched_paths
                .iter()
                .map(|p| canonicalize_for_comparison(p))
                .collect();

            for target in &expanded_targets {
                let canonical_target = canonicalize_for_comparison(target);
                let target_is_dir = target.is_dir();
                let has_match = matched_canon.iter().any(|matched| {
                    if target_is_dir {
                        matched.starts_with(&canonical_target)
                    } else {
                        matched == &canonical_target
                    }
                });

                if !has_match {
                    println!("{}", target.display());
                }
            }
        }

        // grep-like exit codes: 0 if matches found, 1 if none
        if !summary.had_matches {
            eprintln!("No matches found");

            // Show the closest match below threshold if available
            if let Some(closest) = summary.closest_below_threshold {
                // Format like a regular result but in red
                let score_text = format!("[{:.3}] ", closest.score);
                let file_text = format!("{}:", closest.file.display());

                // Get the pattern as a string
                let options = build_options(&cli, false, repo_root);
                let highlighted_preview = highlight_matches(&closest.preview, pattern, &options);

                // Print in red with same format as regular results, with header
                eprintln!();
                eprintln!("{}", style("(nearest match beneath the threshold)").dim());
                eprintln!(
                    "{}{}{}:{}",
                    style(score_text).red(),
                    style(file_text).red(),
                    style(closest.span.line_start).red(),
                    style(highlighted_preview).red()
                );
            }

            std::process::exit(1);
        }
    } else {
        eprintln!("Error: No pattern specified");
        std::process::exit(1);
    }

    Ok(())
}

fn build_options(cli: &Cli, reindex: bool, _repo_root: Option<&Path>) -> SearchOptions {
    let mode = if cli.semantic {
        SearchMode::Semantic
    } else if cli.lexical {
        SearchMode::Lexical
    } else if cli.hybrid {
        SearchMode::Hybrid
    } else {
        SearchMode::Regex
    };

    let context = cli.context.unwrap_or(0);
    let before_context = cli.before_context.unwrap_or(context);
    let after_context = cli.after_context.unwrap_or(context);

    // Use the unified pattern builder
    let exclude_patterns = build_exclude_patterns(cli);

    // Set intelligent defaults for semantic search
    let default_topk = match mode {
        SearchMode::Semantic => Some(10),
        _ => None,
    };
    let default_threshold = match mode {
        SearchMode::Semantic => Some(0.6),
        _ => None,
    };

    SearchOptions {
        mode,
        query: String::new(),
        path: PathBuf::from("."),
        top_k: cli.top_k.or(default_topk),
        threshold: cli.threshold.or(default_threshold),
        case_insensitive: cli.ignore_case,
        whole_word: cli.word_regexp,
        fixed_string: cli.fixed_strings,
        line_numbers: cli.line_numbers,
        context_lines: context,
        before_context_lines: before_context,
        after_context_lines: after_context,
        recursive: cli.recursive,
        json_output: cli.json || cli.json_v1,
        jsonl_output: cli.jsonl,
        no_snippet: cli.no_snippet,
        reindex,
        show_scores: cli.show_scores,
        show_filenames: false, // Will be set by caller
        files_with_matches: cli.files_with_matches,
        files_without_matches: cli.files_without_matches,
        exclude_patterns,
        include_patterns: Vec::new(),
        respect_gitignore: !cli.no_ignore,
        use_ckignore: !cli.no_ckignore,
        full_section: cli.full_section,
        // Enhanced embedding options (search-time only)
        rerank: cli.rerank,
        rerank_model: cli.rerank_model.clone(),
        embedding_model: cli.model.clone(),
    }
}

fn highlight_matches(text: &str, pattern: &str, options: &SearchOptions) -> String {
    // Don't highlight if this is JSON/JSONL output
    if options.json_output || options.jsonl_output {
        return text.to_string();
    }

    match options.mode {
        SearchMode::Regex => highlight_regex_matches(text, pattern, options),
        SearchMode::Semantic | SearchMode::Hybrid => {
            // For semantic/hybrid search, use subchunk similarity highlighting
            highlight_semantic_chunks(text, pattern, options)
        }
        _ => text.to_string(),
    }
}

fn highlight_regex_matches(text: &str, pattern: &str, options: &SearchOptions) -> String {
    // Build regex from pattern with EXACT same logic as regex_search in ck-engine
    let regex_pattern = if options.fixed_string {
        regex::escape(pattern)
    } else if options.whole_word {
        // Must escape the pattern for whole_word, matching the search engine behavior
        format!(r"\b{}\b", regex::escape(pattern))
    } else {
        pattern.to_string()
    };

    let regex_result = RegexBuilder::new(&regex_pattern)
        .case_insensitive(options.case_insensitive)
        .build();

    match regex_result {
        Ok(re) => {
            // Replace matches with highlighted versions
            re.replace_all(text, |caps: &regex::Captures| {
                style(&caps[0]).red().bold().to_string()
            })
            .to_string()
        }
        Err(e) => {
            // Surface regex compilation error to user
            eprintln!("Warning: Invalid regex pattern '{}': {}", pattern, e);
            // Return original text without highlighting
            text.to_string()
        }
    }
}

fn highlight_semantic_chunks(text: &str, pattern: &str, _options: &SearchOptions) -> String {
    let tokens = heatmap::split_into_tokens(text);

    let highlighted_tokens: Vec<String> = tokens
        .into_iter()
        .map(|token| {
            let similarity_score = heatmap::calculate_token_similarity(&token, pattern);
            apply_heatmap_color(&token, similarity_score)
        })
        .collect();

    highlighted_tokens.join("")
}

fn apply_heatmap_color(token: &str, score: f32) -> String {
    if token.trim().is_empty() || token.chars().all(|c| !c.is_alphanumeric()) {
        return token.to_string();
    }

    let bucket = HeatmapBucket::from_score(score);

    match bucket.rgb() {
        Some((r, g, b)) => {
            let coloured = token.color(Rgb(r, g, b));
            if bucket.is_bold() {
                coloured.bold().to_string()
            } else {
                coloured.to_string()
            }
        }
        None => token.to_string(),
    }
}

struct SearchSummary {
    had_matches: bool,
    closest_below_threshold: Option<ck_core::SearchResult>,
    matched_paths: Vec<PathBuf>,
}

async fn run_search(
    pattern: String,
    path: PathBuf,
    mut options: SearchOptions,
    status: &StatusReporter,
) -> Result<SearchSummary> {
    options.query = pattern;
    options.path = path;

    if options.reindex {
        let reindex_spinner = status.create_spinner("Updating index...");
        let file_options = ck_core::FileCollectionOptions::from(&options);
        ck_index::update_index(&options.path, true, &file_options).await?;
        status.finish_progress(reindex_spinner, "Index updated");
    }

    // Show search parameters for semantic mode
    if matches!(
        options.mode,
        ck_core::SearchMode::Semantic | ck_core::SearchMode::Hybrid
    ) {
        let topk_info = options
            .top_k
            .map_or("unlimited".to_string(), |k| k.to_string());
        let threshold_info = options
            .threshold
            .map_or("none".to_string(), |t| format!("{:.1}", t));
        eprintln!(
            "ℹ Semantic search: top {} results, threshold ≥{}",
            topk_info, threshold_info
        );

        let resolved_model =
            ck_engine::resolve_model_for_path(&options.path, options.embedding_model.as_deref())?;

        if resolved_model.alias == resolved_model.canonical_name() {
            eprintln!(
                "🤖 Model: {} ({} dims)",
                resolved_model.canonical_name(),
                resolved_model.dimensions()
            );
        } else {
            eprintln!(
                "🤖 Model: {} (alias '{}', {} dims)",
                resolved_model.canonical_name(),
                resolved_model.alias,
                resolved_model.dimensions()
            );
        }

        let max_tokens = ck_chunk::TokenEstimator::get_model_limit(resolved_model.canonical_name());
        let (chunk_tokens, overlap_tokens) =
            ck_chunk::get_model_chunk_config(Some(resolved_model.canonical_name()));

        eprintln!("📏 FastEmbed Config: {} token limit", max_tokens);
        eprintln!(
            "📄 Chunk Config: {} tokens target, {} token overlap (~20%)",
            chunk_tokens, overlap_tokens
        );
    }

    // Create search spinner for showing live search progress
    let search_spinner = status.create_spinner("Searching...");

    // Create progress callback for search operations
    let search_progress_callback = search_spinner.as_ref().map(|spinner| {
        let spinner_clone = spinner.clone();
        Box::new(move |msg: &str| {
            spinner_clone.set_message(msg.to_string());
        }) as ck_engine::SearchProgressCallback
    });

    // Create indexing progress callbacks for automatic indexing during semantic search
    let (indexing_progress_callback, detailed_indexing_progress_callback) = if !status.quiet
        && matches!(
            options.mode,
            ck_core::SearchMode::Semantic | ck_core::SearchMode::Hybrid
        ) {
        // Create the same enhanced progress system for automatic indexing during semantic search
        use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

        let multi_progress = MultiProgress::new();

        // Overall progress bar (files)
        let overall_pb = multi_progress.add(ProgressBar::new(0));
        overall_pb.set_style(ProgressStyle::default_bar()
            .template("📂 Embedding Files: [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {msg}")
            .unwrap()
            .progress_chars("━━╸ "));

        // Current file progress bar (chunks)
        let file_pb = multi_progress.add(ProgressBar::new(0));
        file_pb.set_style(ProgressStyle::default_bar()
            .template("📄 Embedding Chunks: [{elapsed_precise}] [{bar:40.green/yellow}] {pos}/{len} ({percent}%) {msg}")
            .unwrap()
            .progress_chars("━━╸ "));

        let overall_pb_clone = overall_pb.clone();
        let _file_pb_clone = file_pb.clone();
        let overall_pb_clone2 = overall_pb.clone();
        let file_pb_clone2 = file_pb.clone();

        // Basic progress callback for file-level updates
        let indexing_progress_callback = Some(Box::new(move |file_name: &str| {
            let short_name = file_name.split('/').next_back().unwrap_or(file_name);
            overall_pb_clone.set_message(format!("Processing {}", short_name));
            overall_pb_clone.inc(1);
        }) as ck_engine::IndexingProgressCallback);

        // Detailed progress callback for chunk-level updates
        let detailed_indexing_progress_callback =
            Some(Box::new(move |progress: ck_index::EmbeddingProgress| {
                // Update overall progress bar
                if overall_pb_clone2.length().unwrap_or(0) != progress.total_files as u64 {
                    overall_pb_clone2.set_length(progress.total_files as u64);
                }
                overall_pb_clone2.set_position(progress.file_index as u64);

                // Update file progress bar
                if file_pb_clone2.length().unwrap_or(0) != progress.total_chunks as u64 {
                    file_pb_clone2.set_length(progress.total_chunks as u64);
                    file_pb_clone2.reset();
                }
                file_pb_clone2.set_position(progress.chunk_index as u64);

                let short_name = progress
                    .file_name
                    .split('/')
                    .next_back()
                    .unwrap_or(&progress.file_name);
                file_pb_clone2.set_message(format!(
                    "{} (chunk {}/{}, {}B)",
                    short_name,
                    progress.chunk_index + 1,
                    progress.total_chunks,
                    progress.chunk_size
                ));
            })
                as ck_engine::DetailedIndexingProgressCallback);

        // Store progress bars for cleanup
        let _file_pb_ref = file_pb;
        let _overall_pb_ref = overall_pb;

        (
            indexing_progress_callback,
            detailed_indexing_progress_callback,
        )
    } else {
        (None, None)
    };

    let search_results = ck_engine::search_enhanced_with_indexing_progress(
        &options,
        search_progress_callback,
        indexing_progress_callback,
        detailed_indexing_progress_callback,
    )
    .await?;
    let results = &search_results.matches;
    let matched_paths: Vec<PathBuf> = results.iter().map(|result| result.file.clone()).collect();

    status.finish_progress(search_spinner, &format!("Found {} results", results.len()));

    let mut has_matches = false;
    if options.jsonl_output {
        for result in results {
            has_matches = true;
            let jsonl_result =
                ck_core::JsonlSearchResult::from_search_result(result, !options.no_snippet);
            println!("{}", serde_json::to_string(&jsonl_result)?);
        }
    } else if options.json_output {
        for result in results {
            has_matches = true;
            let json_result = ck_core::JsonSearchResult {
                file: result.file.display().to_string(),
                span: result.span.clone(),
                lang: result.lang,
                symbol: result.symbol.clone(),
                score: result.score,
                signals: ck_core::SearchSignals {
                    lex_rank: None,
                    vec_rank: None,
                    rrf_score: result.score,
                },
                preview: result.preview.clone(),
                model: "none".to_string(),
            };
            println!("{}", serde_json::to_string(&json_result)?);
        }
    } else if options.files_with_matches {
        // For -l flag: print only unique filenames that have matches
        let mut printed_files = std::collections::HashSet::new();
        for result in results {
            has_matches = true;
            let file_path = &result.file;
            if printed_files.insert(file_path.clone()) {
                println!("{}", file_path.display());
            }
        }
    } else if options.files_without_matches {
        // For -L flag: just set has_matches, printing is done later
        has_matches = !results.is_empty();
    } else {
        // Normal output
        for result in results {
            has_matches = true;
            let score_text = if options.show_scores {
                format!("[{:.3}] ", result.score)
            } else {
                String::new()
            };

            let highlighted_preview = highlight_matches(&result.preview, &options.query, &options);

            // Format output based on options
            if options.line_numbers && options.show_filenames {
                // grep format: filename:line_number:content (all on one line)
                println!(
                    "{}{}:{}:{}",
                    score_text,
                    style(result.file.display()).cyan().bold(),
                    style(result.span.line_start).yellow(),
                    highlighted_preview
                );
            } else if options.line_numbers {
                // Just line number when no filename
                println!(
                    "{}{}:{}",
                    score_text,
                    style(result.span.line_start).yellow(),
                    highlighted_preview
                );
            } else if options.show_filenames {
                // Filename on separate line when no line numbers (more readable for semantic search)
                println!(
                    "{}{}:\n{}",
                    score_text,
                    style(result.file.display()).cyan().bold(),
                    highlighted_preview
                );
            } else {
                // No filename or line number
                println!("{}{}", score_text, highlighted_preview);
            }
        }
    }

    Ok(SearchSummary {
        had_matches: has_matches,
        closest_below_threshold: search_results.closest_below_threshold,
        matched_paths,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::path_utils::{self, expand_glob_patterns_with_base};
    use tempfile::tempdir;

    #[test]
    fn test_expand_glob_patterns_supports_semicolon_lists() {
        let temp_dir = tempdir().unwrap();
        let base = temp_dir.path();

        let rust_file = base.join("example.rs");
        let html_file = base.join("page.html");
        let docs_dir = base.join("docs");
        let nested_dir = docs_dir.join("nested");
        let nested_rust_file = nested_dir.join("lib.rs");

        fs::write(&rust_file, "fn main() {}\n").unwrap();
        fs::write(&html_file, "<html></html>").unwrap();
        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(&nested_rust_file, "pub fn lib() {}\n").unwrap();

        let expanded =
            expand_glob_patterns_with_base(base, &[PathBuf::from("*.rs;*.html;docs/")], &[])
                .expect("pattern expansion");

        let has_example = expanded.iter().any(|p| p.ends_with("example.rs"));
        let has_page = expanded.iter().any(|p| p.ends_with("page.html"));
        let has_docs = expanded.iter().any(|p| p.ends_with("docs"));
        let has_nested = expanded.iter().any(|p| p.ends_with("docs/nested/lib.rs"));

        assert!(has_example);
        assert!(has_page);
        assert!(has_docs);
        assert!(has_nested);
    }

    #[test]
    fn test_split_path_patterns_trims_whitespace_and_empties() {
        let patterns = path_utils::split_path_patterns(Path::new(" foo.rs ; ; *.html ;docs/ "));
        assert_eq!(
            patterns,
            vec![
                "foo.rs".to_string(),
                "*.html".to_string(),
                "docs/".to_string()
            ]
        );
    }

    #[test]
    fn test_highlight_regex_matches_with_valid_pattern() {
        let options = SearchOptions {
            mode: SearchMode::Regex,
            case_insensitive: false,
            fixed_string: false,
            whole_word: false,
            use_ckignore: true,
            ..Default::default()
        };

        let text = "hello world test";
        let pattern = "world";
        let result = highlight_regex_matches(text, pattern, &options);

        // Should contain the text (exact highlighting might differ based on styling)
        assert!(result.contains("world"));
    }

    #[test]
    fn test_highlight_regex_matches_with_invalid_pattern() {
        let options = SearchOptions {
            mode: SearchMode::Regex,
            case_insensitive: false,
            fixed_string: false,
            whole_word: false,
            use_ckignore: true,
            ..Default::default()
        };

        let text = "hello world test";
        let pattern = "[invalid"; // Invalid regex pattern

        // Capture stderr to check for warning
        let original_text = highlight_regex_matches(text, pattern, &options);

        // Should return original text when regex is invalid
        assert_eq!(original_text, text);

        // Note: We can't easily capture stderr in unit tests without more complex setup,
        // but the integration test covers the stderr warning behavior
    }

    #[test]
    fn test_highlight_regex_matches_with_fixed_string() {
        let options = SearchOptions {
            mode: SearchMode::Regex,
            case_insensitive: false,
            fixed_string: true, // This should escape the pattern
            whole_word: false,
            ..Default::default()
        };

        let text = "hello [world] test";
        let pattern = "[world]"; // Special chars that would be invalid regex
        let result = highlight_regex_matches(text, pattern, &options);

        // Should work fine because fixed_string escapes the pattern
        assert!(result.contains("[world]"));
    }

    #[test]
    fn test_highlight_regex_matches_with_whole_word() {
        let options = SearchOptions {
            mode: SearchMode::Regex,
            case_insensitive: false,
            fixed_string: false,
            whole_word: true, // This should escape the pattern and add word boundaries
            ..Default::default()
        };

        let text = "hello [world] test";
        let pattern = "[world]"; // Special chars that would be invalid regex
        let result = highlight_regex_matches(text, pattern, &options);

        // Should work fine because whole_word escapes the pattern
        assert!(result.contains("[world]"));
    }
}
