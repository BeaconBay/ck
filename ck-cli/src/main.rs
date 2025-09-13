use anyhow::Result;
use ck_core::{SearchMode, SearchOptions};
use clap::Parser;
use console::style;
use owo_colors::{OwoColorize, Rgb};
use regex::RegexBuilder;
use std::path::{Path, PathBuf};

mod progress;
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
    ck --index .                       # First, create search index
    ck --sem "error handling" src/     # Find error handling patterns (top 10, threshold ≥0.6)
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
    ck --add file.rs                   # Add single file to index

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

SEARCH MODES:
  --regex   : Classic grep behavior (default, no index needed)
  --lex     : BM25 lexical search (requires index)  
  --sem     : Semantic/embedding search (requires index, defaults: top 10, threshold ≥0.6)
  --hybrid  : Combines regex and semantic (requires index)

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

    #[arg(long = "add", help = "Add a single file to the index")]
    add: bool,

    #[arg(long = "status", help = "Show index status and statistics")]
    status: bool,

    #[arg(long = "status-verbose", help = "Show detailed index statistics")]
    status_verbose: bool,

    #[arg(
        long = "inspect",
        help = "Show detailed metadata for a specific file (chunks, embeddings, tree-sitter parsing info)"
    )]
    inspect: bool,

    // Model selection (index-time only)
    #[arg(
        long = "model",
        value_name = "MODEL",
        help = "Embedding model to use for indexing (bge-small, nomic-v1.5, jina-code) [default: bge-small]. Only used with --index."
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
        help = "Reranking model to use (jina, bge) [default: jina]"
    )]
    rerank_model: Option<String>,
}

fn expand_glob_patterns(paths: &[PathBuf], exclude_patterns: &[String]) -> Result<Vec<PathBuf>> {
    let mut expanded = Vec::new();

    for path in paths {
        let path_str = path.to_string_lossy();

        // Check if this looks like a glob pattern
        if path_str.contains('*') || path_str.contains('?') || path_str.contains('[') {
            // Use glob to expand the pattern
            match glob::glob(&path_str) {
                Ok(glob_paths) => {
                    let mut found_matches = false;
                    for glob_result in glob_paths {
                        match glob_result {
                            Ok(matched_path) => {
                                // Apply exclusion patterns to glob results
                                if !should_exclude_path(&matched_path, exclude_patterns) {
                                    expanded.push(matched_path);
                                }
                                found_matches = true;
                            }
                            Err(e) => {
                                eprintln!("Warning: glob error for pattern '{}': {}", path_str, e);
                            }
                        }
                    }

                    // If no matches found, treat as literal path (grep behavior)
                    if !found_matches {
                        expanded.push(path.clone());
                    }
                }
                Err(e) => {
                    eprintln!("Warning: invalid glob pattern '{}': {}", path_str, e);
                    // Treat as literal path if glob pattern is invalid
                    expanded.push(path.clone());
                }
            }
        } else {
            // Not a glob pattern, use as-is
            expanded.push(path.clone());
        }
    }

    Ok(expanded)
}

fn should_exclude_path(path: &Path, exclude_patterns: &[String]) -> bool {
    // Check if any component in the path matches an exclusion pattern
    for component in path.components() {
        if let std::path::Component::Normal(name) = component {
            let name_str = name.to_string_lossy();
            for pattern in exclude_patterns {
                if name_str == pattern.as_str() {
                    return true;
                }
            }
        }
    }
    false
}

async fn inspect_file_metadata(file_path: &PathBuf, status: &StatusReporter) -> Result<()> {
    use std::fs;
    use std::path::Path;

    let path = Path::new(file_path);

    // Basic file info
    status.info(&format!("📁 File: {}", path.display()));

    if !path.exists() {
        status.error("File does not exist");
        return Ok(());
    }

    let metadata = fs::metadata(path)?;
    status.info(&format!("📏 Size: {} bytes", metadata.len()));

    let detected_lang = ck_core::Language::from_path(path);

    status.info(&format!(
        "🔍 Language: {}",
        detected_lang
            .map(|l| l.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    ));

    // Read file content
    let content = fs::read_to_string(path)?;
    status.info(&format!("📄 Lines: {}", content.lines().count()));

    // Try chunking with detected language
    status.info("🧩 Chunking Analysis:");
    let chunks = ck_chunk::chunk_text(&content, detected_lang)?;
    status.info(&format!("  • Total chunks: {}", chunks.len()));

    for (i, chunk) in chunks.iter().take(15).enumerate() {
        status.info(&format!(
            "  • Chunk {}: {:?} ({}:{}-{}:{})",
            i + 1,
            chunk.chunk_type,
            chunk.span.line_start,
            chunk.span.line_end,
            chunk.span.byte_start,
            chunk.span.byte_end
        ));

        // Show preview of chunk content (first 100 chars)
        let preview = if chunk.text.chars().count() > 100 {
            let truncated: String = chunk.text.chars().take(100).collect();
            format!("{}...", truncated)
        } else {
            chunk.text.clone()
        };
        status.info(&format!("    Preview: {}", preview.replace('\n', "\\n")));
    }

    if chunks.len() > 15 {
        status.info(&format!("    ... and {} more chunks", chunks.len() - 15));
    }

    // Check if file is indexed
    let parent_dir = path.parent().unwrap_or(Path::new("."));
    if let Ok(stats) = ck_index::get_index_stats(parent_dir) {
        if stats.total_files > 0 {
            status.info("📚 Index Status: File's directory is indexed");
            status.info(&format!("  • Total indexed files: {}", stats.total_files));
            status.info(&format!(
                "  • Total chunks in index: {}",
                stats.total_chunks
            ));
        } else {
            status.warn("📚 Index Status: File's directory is not indexed");
            status.info("  Run 'ck --index .' to create an index for semantic search");
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
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .init();

    let cli = Cli::parse();
    let status = StatusReporter::new(cli.quiet);

    // Handle command flags first (these take precedence over search)
    if cli.index {
        // Handle --index flag
        let path = cli
            .files
            .first()
            .cloned()
            .unwrap_or_else(|| PathBuf::from("."));

        status.section_header("Indexing Repository");
        status.info(&format!("Scanning files in {}", path.display()));

        // Build exclusion patterns
        let mut exclude_patterns = Vec::new();
        if !cli.no_default_excludes {
            exclude_patterns.extend(ck_core::get_default_exclude_patterns());
        }
        exclude_patterns.extend(cli.exclude.clone());

        let indexing_progress = status.create_spinner("Building index...");

        // Create progress callback to show current file being processed
        let progress_callback = if !cli.quiet {
            let pb_clone = indexing_progress.clone();
            Some(Box::new(move |file_name: &str| {
                if let Some(pb) = &pb_clone {
                    pb.set_message(format!("Processing {}", file_name));
                }
            }) as ck_index::ProgressCallback)
        } else {
            None
        };

        let stats = ck_index::smart_update_index_with_progress(
            &path,
            false,
            progress_callback,
            true,
            !cli.no_ignore,
            &exclude_patterns,
            cli.model.as_deref(), // Pass the model selection from CLI
        )
        .await?;
        status.finish_progress(indexing_progress, "Index built successfully");

        status.success(&format!("Indexed {} files", stats.files_indexed));
        if stats.files_added > 0 {
            status.info(&format!("  {} new files added", stats.files_added));
        }
        if stats.files_modified > 0 {
            status.info(&format!("  {} files updated", stats.files_modified));
        }
        if stats.files_up_to_date > 0 {
            status.info(&format!(
                "  {} files already current",
                stats.files_up_to_date
            ));
        }
        if stats.orphaned_files_removed > 0 {
            status.info(&format!(
                "  {} orphaned entries cleaned",
                stats.orphaned_files_removed
            ));
        }
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

            // Build exclusion patterns
            let mut exclude_patterns = Vec::new();
            if !cli.no_default_excludes {
                exclude_patterns.extend(ck_core::get_default_exclude_patterns());
            }
            exclude_patterns.extend(cli.exclude.clone());

            let cleanup_spinner = status.create_spinner("Removing orphaned entries...");
            let cleanup_stats =
                ck_index::cleanup_index(&clean_path, !cli.no_ignore, &exclude_patterns)?;
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

    if cli.status || cli.status_verbose {
        // Handle --status and --status-verbose flags
        let status_path = cli
            .files
            .first()
            .cloned()
            .unwrap_or_else(|| PathBuf::from("."));
        let verbose = cli.status_verbose;

        status.section_header("Index Status");
        let check_spinner = status.create_spinner("Reading index...");
        let stats = ck_index::get_index_stats(&status_path)?;
        status.finish_progress(check_spinner, "Status retrieved");

        if stats.total_files == 0 {
            status.warn(&format!("No index found at {}", status_path.display()));
            status.info("Run 'ck --index .' to create an index");
        } else {
            status.info(&format!("Index location: {}", status_path.display()));
            status.success(&format!("Files indexed: {}", stats.total_files));
            status.info(&format!("  Total chunks: {}", stats.total_chunks));
            status.info(&format!("  Embedded chunks: {}", stats.embedded_chunks));

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

    // Validate conflicting flags
    if cli.files_with_matches && cli.files_without_matches {
        eprintln!("Error: Cannot use -l and -L together");
        std::process::exit(1);
    }

    // Default behavior: search with pattern
    if let Some(ref pattern) = cli.pattern {
        let reindex = cli.reindex;

        // Build options to get exclusion patterns
        let temp_options = build_options(&cli, reindex);

        let files = if cli.files.is_empty() {
            vec![PathBuf::from(".")]
        } else {
            expand_glob_patterns(&cli.files, &temp_options.exclude_patterns)?
        };

        // Handle multiple files like grep; allow -h/-H overrides
        let mut show_filenames = files.len() > 1 || files.iter().any(|p| p.is_dir());
        if cli.no_filenames {
            show_filenames = false;
        }
        if cli.with_filenames {
            show_filenames = true;
        }
        let mut any_matches = false;
        for file_path in files {
            let mut options = build_options(&cli, reindex);
            options.show_filenames = show_filenames;
            let had_matches = run_search(pattern.clone(), file_path, options, &status).await?;
            if had_matches {
                any_matches = true;
            }
        }

        // grep-like exit codes: 0 if matches found, 1 if none
        if !any_matches {
            eprintln!("No matches found");
            std::process::exit(1);
        }
    } else {
        eprintln!("Error: No pattern specified");
        std::process::exit(1);
    }

    Ok(())
}

fn build_options(cli: &Cli, reindex: bool) -> SearchOptions {
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

    // Build exclusion patterns (as provided; glob semantics applied in ck-search)
    let mut exclude_patterns = Vec::new();

    // Add default exclusions unless disabled
    if !cli.no_default_excludes {
        exclude_patterns.extend(ck_core::get_default_exclude_patterns());
    }

    // Add user-specified exclusions
    exclude_patterns.extend(cli.exclude.clone());

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
        respect_gitignore: !cli.no_ignore,
        full_section: cli.full_section,
        // Enhanced embedding options (search-time only)
        rerank: cli.rerank,
        rerank_model: cli.rerank_model.clone(),
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
        Err(_) => {
            // If regex is invalid, return original text
            text.to_string()
        }
    }
}

fn highlight_semantic_chunks(text: &str, pattern: &str, _options: &SearchOptions) -> String {
    // Split text into tokens for more granular heatmap highlighting
    let tokens = split_into_tokens(text);

    // Calculate similarity scores for each token/phrase
    let highlighted_tokens: Vec<String> = tokens
        .into_iter()
        .map(|token| {
            let similarity_score = calculate_token_similarity(&token, pattern);
            apply_heatmap_color(&token, similarity_score)
        })
        .collect();

    highlighted_tokens.join("")
}

fn split_into_tokens(text: &str) -> Vec<String> {
    // Split text into meaningful tokens for heatmap highlighting
    // This preserves spaces and punctuation as separate tokens
    let mut tokens = Vec::new();
    let mut current_token = String::new();

    for ch in text.chars() {
        match ch {
            ' ' | '\t' | '\n' => {
                if !current_token.is_empty() {
                    tokens.push(current_token.clone());
                    current_token.clear();
                }
                tokens.push(ch.to_string());
            }
            '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';' | ':' | '.' | '!' | '?' => {
                if !current_token.is_empty() {
                    tokens.push(current_token.clone());
                    current_token.clear();
                }
                tokens.push(ch.to_string());
            }
            _ => {
                current_token.push(ch);
            }
        }
    }

    if !current_token.is_empty() {
        tokens.push(current_token);
    }

    tokens
}

fn calculate_token_similarity(token: &str, pattern: &str) -> f32 {
    // Skip whitespace and punctuation
    if token.trim().is_empty() || token.chars().all(|c| !c.is_alphanumeric()) {
        return 0.0;
    }

    let token_lower = token.to_lowercase();
    let pattern_lower = pattern.to_lowercase();

    // Exact match gets highest score
    if token_lower == pattern_lower {
        return 1.0;
    }

    // Check if token contains any pattern words or vice versa
    let pattern_words: Vec<&str> = pattern_lower.split_whitespace().collect();
    let mut max_score: f32 = 0.0;

    for pattern_word in &pattern_words {
        if pattern_word.len() < 3 {
            continue; // Skip short words
        }

        // Exact word match
        if token_lower == *pattern_word {
            max_score = max_score.max(0.9);
        }
        // Substring match
        else if token_lower.contains(pattern_word) {
            let ratio = pattern_word.len() as f32 / token_lower.len() as f32;
            max_score = max_score.max(0.6 * ratio);
        }
        // Pattern word contains token
        else if pattern_word.contains(&token_lower) && token_lower.len() >= 3 {
            let ratio = token_lower.len() as f32 / pattern_word.len() as f32;
            max_score = max_score.max(0.5 * ratio);
        }
        // Fuzzy similarity for related terms
        else {
            let similarity = calculate_fuzzy_similarity(&token_lower, pattern_word);
            max_score = max_score.max(similarity * 0.4);
        }
    }

    max_score
}

fn calculate_fuzzy_similarity(s1: &str, s2: &str) -> f32 {
    // Simple edit distance-based similarity
    if s1.is_empty() || s2.is_empty() || s1.len() < 3 || s2.len() < 3 {
        return 0.0;
    }

    let len1 = s1.len();
    let len2 = s2.len();
    let max_len = len1.max(len2);

    // Count common characters
    let s1_chars: std::collections::HashSet<char> = s1.chars().collect();
    let s2_chars: std::collections::HashSet<char> = s2.chars().collect();
    let common_chars = s1_chars.intersection(&s2_chars).count();

    // Similarity based on common characters
    common_chars as f32 / max_len as f32
}

fn apply_heatmap_color(token: &str, score: f32) -> String {
    // Skip coloring whitespace and punctuation
    if token.trim().is_empty() || token.chars().all(|c| !c.is_alphanumeric()) {
        return token.to_string();
    }

    // 8-step linear gradient: grey to green with bright final step
    match score {
        s if s >= 0.875 => token.color(Rgb(0, 255, 100)).bold().to_string(), // Step 8: Extra bright green (bold)
        s if s >= 0.75 => token.color(Rgb(0, 180, 80)).to_string(),          // Step 7: Bright green
        s if s >= 0.625 => token.color(Rgb(0, 160, 70)).to_string(), // Step 6: Medium-bright green
        s if s >= 0.5 => token.color(Rgb(0, 140, 60)).to_string(),   // Step 5: Medium green
        s if s >= 0.375 => token.color(Rgb(50, 120, 80)).to_string(), // Step 4: Green-grey
        s if s >= 0.25 => token.color(Rgb(100, 130, 100)).to_string(), // Step 3: Light green-grey
        s if s >= 0.125 => token.color(Rgb(140, 140, 140)).to_string(), // Step 2: Medium grey
        s if s > 0.0 => token.color(Rgb(180, 180, 180)).to_string(), // Step 1: Light grey
        _ => token.to_string(),                                      // No relevance: no color
    }
}

async fn run_search(
    pattern: String,
    path: PathBuf,
    mut options: SearchOptions,
    status: &StatusReporter,
) -> Result<bool> {
    options.query = pattern;
    options.path = path;

    if options.reindex {
        let reindex_spinner = status.create_spinner("Updating index...");
        ck_index::update_index(
            &options.path,
            true,
            options.respect_gitignore,
            &options.exclude_patterns,
        )
        .await?;
        status.finish_progress(reindex_spinner, "Index updated");
    }

    // Show search progress for non-regex searches or when explicitly enabled
    let search_spinner = if !matches!(options.mode, ck_core::SearchMode::Regex) {
        let mode_name = match options.mode {
            ck_core::SearchMode::Semantic => "semantic",
            ck_core::SearchMode::Lexical => "lexical",
            ck_core::SearchMode::Hybrid => "hybrid",
            _ => "regex",
        };

        // Show search parameters for semantic mode
        if matches!(options.mode, ck_core::SearchMode::Semantic) {
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
        }

        status.create_spinner(&format!("Searching with {} mode...", mode_name))
    } else {
        None
    };

    // Create progress callback for search operations
    let search_progress_callback = if !status.quiet && search_spinner.is_some() {
        let spinner_clone = search_spinner.clone();
        Some(Box::new(move |msg: &str| {
            if let Some(ref pb) = spinner_clone {
                pb.set_message(msg.to_string());
            }
        }) as ck_engine::SearchProgressCallback)
    } else {
        None
    };

    let results = ck_engine::search_with_progress(&options, search_progress_callback).await?;

    if let Some(spinner) = search_spinner {
        status.finish_progress(Some(spinner), &format!("Found {} results", results.len()));
    }

    let mut has_matches = false;
    if options.jsonl_output {
        for result in results {
            has_matches = true;
            let jsonl_result =
                ck_core::JsonlSearchResult::from_search_result(&result, !options.no_snippet);
            println!("{}", serde_json::to_string(&jsonl_result)?);
        }
    } else if options.json_output {
        for result in results {
            has_matches = true;
            let json_result = ck_core::JsonSearchResult {
                file: result.file.display().to_string(),
                span: result.span,
                lang: result.lang,
                symbol: result.symbol,
                score: result.score,
                signals: ck_core::SearchSignals {
                    lex_rank: None,
                    vec_rank: None,
                    rrf_score: result.score,
                },
                preview: result.preview,
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

            let file_text = if options.show_filenames {
                format!("{}:", style(result.file.display()).cyan().bold())
            } else {
                String::new()
            };

            let highlighted_preview = highlight_matches(&result.preview, &options.query, &options);

            if options.line_numbers && options.show_filenames {
                println!(
                    "{}{}{}:{}",
                    score_text,
                    file_text,
                    style(result.span.line_start).yellow(),
                    highlighted_preview
                );
            } else if options.line_numbers {
                println!(
                    "{}{}:{}",
                    score_text,
                    style(result.span.line_start).yellow(),
                    highlighted_preview
                );
            } else {
                println!("{}{}{}", score_text, file_text, highlighted_preview);
            }
        }
    }

    // For -L flag: if this file had no matches, print the filename
    if options.files_without_matches && !has_matches {
        println!("{}", options.path.display());
    }

    Ok(has_matches)
}
