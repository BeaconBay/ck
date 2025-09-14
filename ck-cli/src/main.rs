use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ck")]
#[command(about = "Semantic grep by embedding - seek code, semantically")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create or update semantic index
    Index {
        /// Path to index
        path: Option<PathBuf>,

        #[arg(long, help = "Embedding model to use")]
        model: Option<String>,

        #[arg(long, help = "Force rebuild from scratch")]
        force: bool,

        #[arg(long, help = "Exclude patterns")]
        exclude: Vec<String>,
    },

    /// Search for pattern
    Search {
        /// Search pattern
        pattern: String,

        /// Files or directories to search
        paths: Vec<PathBuf>,

        #[arg(long, help = "Use semantic search")]
        semantic: bool,

        #[arg(long, help = "Use lexical search")]
        lexical: bool,

        #[arg(long, help = "Use hybrid search")]
        hybrid: bool,

        #[arg(short = 'n', long, help = "Show line numbers")]
        line_numbers: bool,

        #[arg(short = 'i', long, help = "Case insensitive")]
        ignore_case: bool,

        #[arg(long, help = "JSON output")]
        json: bool,

        #[arg(long, help = "JSONL output")]
        jsonl: bool,

        #[arg(long, help = "Top K results")]
        topk: Option<usize>,

        #[arg(long, help = "Threshold")]
        threshold: Option<f32>,
    },

    /// Check index status
    Status {
        /// Path to check
        path: Option<PathBuf>,

        #[arg(short = 'v', long, help = "Verbose output")]
        verbose: bool,
    },

    /// Clean index
    Clean {
        /// Path to clean
        path: Option<PathBuf>,

        #[arg(long, help = "Clean orphans only")]
        orphans: bool,
    },

    /// Inspect file
    Inspect {
        /// File to inspect
        file: PathBuf,

        #[arg(long, help = "Model to use for analysis")]
        model: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Error: {}", e);
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

    match cli.command {
        Commands::Index {
            path,
            model,
            force,
            exclude,
        } => {
            let index_path = path.unwrap_or_else(|| PathBuf::from("."));
            println!("🔄 Indexing: {}", index_path.display());
            println!("📦 Model: {}", model.as_deref().unwrap_or("default"));
            println!("🔨 Force rebuild: {}", force);
            println!("🚫 Exclude patterns: {:?}", exclude);

            // Call existing indexing function with proper error handling
            let result = ck_index::smart_update_index(
                &index_path,
                true, // compute_embeddings
                true, // respect_gitignore
                &exclude,
            )
            .await;

            match result {
                Ok(stats) => {
                    println!("✅ Indexed {} files", stats.files_indexed);
                    if stats.files_up_to_date > 0 {
                        println!("ℹ️  Skipped {} unchanged files", stats.files_up_to_date);
                    }
                }
                Err(e) => {
                    eprintln!("❌ Indexing failed: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Search {
            pattern,
            paths,
            semantic,
            lexical,
            hybrid,
            line_numbers,
            ignore_case,
            json,
            jsonl,
            topk,
            threshold,
        } => {
            let search_paths = if paths.is_empty() {
                vec![PathBuf::from(".")]
            } else {
                paths
            };

            let mode = if semantic {
                ck_core::SearchMode::Semantic
            } else if lexical {
                ck_core::SearchMode::Lexical
            } else if hybrid {
                ck_core::SearchMode::Hybrid
            } else {
                ck_core::SearchMode::Regex
            };

            println!("🔍 Searching for: '{}'", pattern);
            println!("📂 Paths: {:?}", search_paths);
            println!("🔧 Mode: {:?}", mode);

            for path in search_paths {
                let options = ck_core::SearchOptions {
                    mode: mode.clone(),
                    query: pattern.clone(),
                    path: path.clone(),
                    top_k: topk,
                    threshold,
                    case_insensitive: ignore_case,
                    line_numbers,
                    json_output: json,
                    jsonl_output: jsonl,
                    recursive: true,
                    ..Default::default()
                };

                match ck_engine::search(&options).await {
                    Ok(results) => {
                        if results.is_empty() {
                            println!("No matches found in {}", path.display());
                        } else {
                            for result in results {
                                if json || jsonl {
                                    println!("{}", serde_json::to_string(&result)?);
                                } else {
                                    let line_prefix = if line_numbers {
                                        format!("{}:", result.span.line_start)
                                    } else {
                                        String::new()
                                    };

                                    println!(
                                        "{}:{}{}",
                                        result.file.display(),
                                        line_prefix,
                                        result.preview
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("❌ Search failed for {}: {}", path.display(), e);
                    }
                }
            }
        }

        Commands::Status { path, verbose } => {
            let status_path = path.unwrap_or_else(|| PathBuf::from("."));
            println!("📊 Index status for: {}", status_path.display());

            match ck_index::get_index_stats(&status_path) {
                Ok(stats) => {
                    if stats.total_files == 0 {
                        println!("❌ No index found. Run 'ck index' to create one.");
                    } else {
                        println!(
                            "✅ Index contains {} files ({} chunks)",
                            stats.total_files, stats.total_chunks
                        );

                        if verbose {
                            println!(
                                "📏 Index size: {:.2} MB",
                                stats.index_size_bytes as f64 / 1_048_576.0
                            );
                            println!("🕒 Created: {}", stats.index_created);
                            println!("🔄 Updated: {}", stats.index_updated);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("❌ Failed to get index stats: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Clean { path, orphans } => {
            let clean_path = path.unwrap_or_else(|| PathBuf::from("."));

            if orphans {
                println!("🧹 Cleaning orphaned files in: {}", clean_path.display());
                // TODO: Implement orphan cleanup
                println!("ℹ️  Orphan cleanup not yet implemented");
            } else {
                println!("🗑️  Cleaning entire index in: {}", clean_path.display());
                match ck_index::clean_index(&clean_path) {
                    Ok(_) => println!("✅ Index cleaned successfully"),
                    Err(e) => {
                        eprintln!("❌ Failed to clean index: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }

        Commands::Inspect { file, model } => {
            println!("🔍 Inspecting file: {}", file.display());
            println!("📦 Model: {}", model.as_deref().unwrap_or("default"));

            if !file.exists() {
                eprintln!("❌ File not found: {}", file.display());
                std::process::exit(1);
            }

            let content = std::fs::read_to_string(&file)?;
            let line_count = content.lines().count();
            let byte_count = content.len();
            let word_count = content.split_whitespace().count();

            println!(
                "📄 File: {} ({:.1} KB, {} lines, {} words)",
                file.display(),
                byte_count as f64 / 1024.0,
                line_count,
                word_count
            );

            println!("📝 Language: {:?}", ck_core::Language::from_path(&file));
            println!("✅ File inspection complete");
        }
    }

    Ok(())
}
