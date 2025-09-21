use anyhow::Result;
use ck_core::{CkError, SearchMode, SearchOptions, SearchResult, Span};
use globset::{Glob, GlobSet, GlobSetBuilder};
use rayon::prelude::*;
use regex::{Regex, RegexBuilder};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf as StdPathBuf;
use std::path::{Path, PathBuf};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{STORED, Schema, TEXT, Value};
use tantivy::{Index, ReloadPolicy, TantivyDocument, doc};
use walkdir::WalkDir;

mod semantic_v3;
pub use semantic_v3::{semantic_search_v3, semantic_search_v3_with_progress};

pub type SearchProgressCallback = Box<dyn Fn(&str) + Send + Sync>;
pub type IndexingProgressCallback = Box<dyn Fn(&str) + Send + Sync>;
pub type DetailedIndexingProgressCallback = Box<dyn Fn(ck_index::EmbeddingProgress) + Send + Sync>;

/// Resolve the actual file path to read content from
/// For PDFs: returns cache path and validates it exists
/// For regular files: returns original path
fn resolve_content_path(file_path: &Path, repo_root: &Path) -> Result<PathBuf> {
    if ck_core::pdf::is_pdf_file(file_path) {
        // PDFs: Read from cached extracted text
        let cache_path = ck_core::pdf::get_content_cache_path(repo_root, file_path);
        if !cache_path.exists() {
            return Err(anyhow::anyhow!(
                "PDF not preprocessed. Run 'ck --index' first."
            ));
        }
        Ok(cache_path)
    } else {
        // Regular files: Read from original source
        Ok(file_path.to_path_buf())
    }
}

/// Read content from file for search result extraction
/// Regular files: read directly from source
/// PDFs: read from preprocessed cache
fn read_file_content(file_path: &Path, repo_root: &Path) -> Result<String> {
    let content_path = resolve_content_path(file_path, repo_root)?;
    Ok(fs::read_to_string(content_path)?)
}

/// Extract content from a file using a span (streaming version)
async fn extract_content_from_span(file_path: &Path, span: &ck_core::Span) -> Result<String> {
    // Find repo root to locate cache
    let repo_root = find_nearest_index_root(file_path)
        .unwrap_or_else(|| file_path.parent().unwrap_or(file_path).to_path_buf());

    // Use centralized path resolution
    let content_path = resolve_content_path(file_path, &repo_root)?;

    // Stream only the needed lines
    extract_lines_from_file(&content_path, span.line_start, span.line_end)
}

/// Stream-read specific lines from a file without loading the entire content
fn extract_lines_from_file(file_path: &Path, line_start: usize, line_end: usize) -> Result<String> {
    use std::io::{BufRead, BufReader};

    if line_start == 0 {
        return Ok(String::new());
    }

    let file = fs::File::open(file_path)?;
    let reader = BufReader::new(file);
    let mut result = Vec::new();

    // Convert to 0-based indexing
    let start_idx = line_start.saturating_sub(1);
    let end_idx = line_end.saturating_sub(1);

    for (current_line, line_result) in reader.lines().enumerate() {
        if current_line > end_idx {
            break; // Stop reading once we've passed the needed lines
        }

        let line = line_result?;

        if current_line >= start_idx {
            result.push(line);
        }
    }

    // Handle case where requested lines exceed file length
    if result.is_empty() && line_start > 0 {
        return Ok(String::new());
    }

    Ok(result.join("\n"))
}

/// Split content into lines while preserving the exact number of trailing newline bytes per line.
/// Handles Unix (\n), Windows (\r\n) and old Mac (\r) line endings.
fn split_lines_with_endings(content: &str) -> (Vec<String>, Vec<usize>) {
    let mut lines = Vec::new();
    let mut endings = Vec::new();

    let bytes = content.as_bytes();
    let mut start = 0usize;
    let mut i = 0usize;

    while i < bytes.len() {
        match bytes[i] {
            b'\n' => {
                lines.push(content[start..i].to_string());
                endings.push(1);
                i += 1;
                start = i;
            }
            b'\r' => {
                lines.push(content[start..i].to_string());
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    endings.push(2);
                    i += 2;
                } else {
                    endings.push(1);
                    i += 1;
                }
                start = i;
            }
            _ => {
                i += 1;
            }
        }
    }

    if start < bytes.len() {
        lines.push(content[start..].to_string());
        endings.push(0);
    }

    (lines, endings)
}

fn find_nearest_index_root(path: &Path) -> Option<StdPathBuf> {
    let mut current = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    loop {
        if current.join(".ck").exists() {
            return Some(current.to_path_buf());
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => return None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedModel {
    pub canonical_name: String,
    pub alias: String,
    pub dimensions: usize,
}

fn find_model_entry<'a>(
    registry: &'a ck_models::ModelRegistry,
    key: &str,
) -> Option<(String, &'a ck_models::ModelConfig)> {
    if let Some(config) = registry.get_model(key) {
        return Some((key.to_string(), config));
    }

    registry
        .models
        .iter()
        .find(|(_, config)| config.name == key)
        .map(|(alias, config)| (alias.clone(), config))
}

pub(crate) fn resolve_model_from_root(
    index_root: &Path,
    cli_model: Option<&str>,
) -> Result<ResolvedModel> {
    use ck_models::ModelRegistry;

    let registry = ModelRegistry::default();
    let index_dir = index_root.join(".ck");
    let manifest_path = index_dir.join("manifest.json");

    if manifest_path.exists() {
        let data = std::fs::read(&manifest_path)?;
        let manifest: ck_index::IndexManifest = serde_json::from_slice(&data)?;

        if let Some(existing_model) = manifest.embedding_model {
            let (alias, config_opt) = find_model_entry(&registry, &existing_model)
                .map(|(alias, config)| (alias, Some(config)))
                .unwrap_or_else(|| (existing_model.clone(), None));

            let dims = manifest
                .embedding_dimensions
                .or_else(|| config_opt.map(|c| c.dimensions))
                .unwrap_or(384);

            if let Some(requested) = cli_model {
                let (_, requested_config) =
                    find_model_entry(&registry, requested).ok_or_else(|| {
                        CkError::Embedding(format!(
                            "Unknown model '{}'. Available models: {}",
                            requested,
                            registry
                                .models
                                .keys()
                                .cloned()
                                .collect::<Vec<_>>()
                                .join(", ")
                        ))
                    })?;

                if requested_config.name != existing_model {
                    let suggested_alias = alias.clone();
                    return Err(CkError::Embedding(format!(
                        "Index was built with embedding model '{}' (alias '{}'), but '--model {}' was requested. To switch models run `ck --clean .` then `ck --index --model {}`. To keep using this index rerun your command with '--model {}'.",
                        existing_model,
                        suggested_alias,
                        requested,
                        requested,
                        suggested_alias
                    ))
                    .into());
                }
            }

            return Ok(ResolvedModel {
                canonical_name: existing_model,
                alias,
                dimensions: dims,
            });
        }
    }

    let (alias, config) = if let Some(requested) = cli_model {
        find_model_entry(&registry, requested).ok_or_else(|| {
            CkError::Embedding(format!(
                "Unknown model '{}'. Available models: {}",
                requested,
                registry
                    .models
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?
    } else {
        let alias = registry.default_model.clone();
        let config = registry.get_default_model().ok_or_else(|| {
            CkError::Embedding("No default embedding model configured".to_string())
        })?;
        (alias, config)
    };

    Ok(ResolvedModel {
        canonical_name: config.name.clone(),
        alias,
        dimensions: config.dimensions,
    })
}

pub fn resolve_model_for_path(path: &Path, cli_model: Option<&str>) -> Result<ResolvedModel> {
    let index_root = find_nearest_index_root(path).unwrap_or_else(|| {
        if path.is_file() {
            path.parent().unwrap_or(path).to_path_buf()
        } else {
            path.to_path_buf()
        }
    });
    resolve_model_from_root(&index_root, cli_model)
}

pub async fn search(options: &SearchOptions) -> Result<Vec<SearchResult>> {
    let results = search_enhanced(options).await?;
    Ok(results.matches)
}

pub async fn search_with_progress(
    options: &SearchOptions,
    progress_callback: Option<SearchProgressCallback>,
) -> Result<Vec<SearchResult>> {
    let results = search_enhanced_with_progress(options, progress_callback).await?;
    Ok(results.matches)
}

/// Enhanced search that includes near-miss information for threshold queries
pub async fn search_enhanced(options: &SearchOptions) -> Result<ck_core::SearchResults> {
    search_enhanced_with_progress(options, None).await
}

/// Enhanced search with progress callback that includes near-miss information
pub async fn search_enhanced_with_progress(
    options: &SearchOptions,
    progress_callback: Option<SearchProgressCallback>,
) -> Result<ck_core::SearchResults> {
    search_enhanced_with_indexing_progress(options, progress_callback, None, None).await
}

/// Enhanced search with both search and indexing progress callbacks
pub async fn search_enhanced_with_indexing_progress(
    options: &SearchOptions,
    progress_callback: Option<SearchProgressCallback>,
    indexing_progress_callback: Option<IndexingProgressCallback>,
    detailed_indexing_progress_callback: Option<DetailedIndexingProgressCallback>,
) -> Result<ck_core::SearchResults> {
    // Validate that the search path exists
    if !options.path.exists() {
        return Err(ck_core::CkError::Search(format!(
            "Path does not exist: {}",
            options.path.display()
        ))
        .into());
    }

    // Auto-update index if needed (unless it's regex-only mode)
    if !matches!(options.mode, SearchMode::Regex) {
        let need_embeddings = matches!(options.mode, SearchMode::Semantic | SearchMode::Hybrid);
        ensure_index_updated_with_progress(
            &options.path,
            options.reindex,
            need_embeddings,
            indexing_progress_callback,
            detailed_indexing_progress_callback,
            options.respect_gitignore,
            &options.exclude_patterns,
            options.embedding_model.as_deref(),
        )
        .await?;
    }

    let search_results = match options.mode {
        SearchMode::Regex => {
            let matches = regex_search(options)?;
            ck_core::SearchResults {
                matches,
                closest_below_threshold: None,
            }
        }
        SearchMode::Lexical => {
            let matches = lexical_search(options).await?;
            ck_core::SearchResults {
                matches,
                closest_below_threshold: None,
            }
        }
        SearchMode::Semantic => {
            // Use v3 semantic search (reads pre-computed embeddings from sidecars using spans)
            semantic_search_v3_with_progress(options, progress_callback).await?
        }
        SearchMode::Hybrid => {
            let matches = hybrid_search_with_progress(options, progress_callback).await?;
            ck_core::SearchResults {
                matches,
                closest_below_threshold: None,
            }
        }
    };

    Ok(search_results)
}

fn regex_search(options: &SearchOptions) -> Result<Vec<SearchResult>> {
    let pattern = if options.fixed_string {
        regex::escape(&options.query)
    } else if options.whole_word {
        format!(r"\b{}\b", regex::escape(&options.query))
    } else {
        options.query.clone()
    };

    let regex = RegexBuilder::new(&pattern)
        .case_insensitive(options.case_insensitive)
        .build()
        .map_err(CkError::Regex)?;

    // Default to recursive for directories (like grep) to maintain compatibility
    let should_recurse = options.path.is_dir() || options.recursive;
    let files = if should_recurse {
        // Use ck_index's collect_files which respects gitignore
        ck_index::collect_files(
            &options.path,
            options.respect_gitignore,
            &options.exclude_patterns,
        )?
    } else {
        // For non-recursive, use the local collect_files
        collect_files(&options.path, should_recurse, &options.exclude_patterns)?
    };

    let results: Vec<Vec<SearchResult>> = files
        .par_iter()
        .filter_map(|file_path| match search_file(&regex, file_path, options) {
            Ok(matches) => {
                if matches.is_empty() {
                    None
                } else {
                    Some(matches)
                }
            }
            Err(e) => {
                tracing::debug!("Error searching {:?}: {}", file_path, e);
                None
            }
        })
        .collect();

    let mut all_results: Vec<SearchResult> = results.into_iter().flatten().collect();
    // Deterministic ordering: file path, then line number
    all_results.sort_by(|a, b| {
        let path_cmp = a.file.cmp(&b.file);
        if path_cmp != std::cmp::Ordering::Equal {
            return path_cmp;
        }
        a.span.line_start.cmp(&b.span.line_start)
    });

    if let Some(top_k) = options.top_k {
        all_results.truncate(top_k);
    }

    Ok(all_results)
}

fn search_file(
    regex: &Regex,
    file_path: &Path,
    options: &SearchOptions,
) -> Result<Vec<SearchResult>> {
    // Find repo root to locate cache
    let repo_root = find_nearest_index_root(file_path)
        .unwrap_or_else(|| file_path.parent().unwrap_or(file_path).to_path_buf());

    // For full_section mode, we need the entire content for parsing
    // For context previews, we need all lines for surrounding context
    // So we'll load content when needed, but optimize for the common case
    if options.full_section || options.context_lines > 0 {
        // Load full content when we need section parsing or context
        let content = read_file_content(file_path, &repo_root)?;
        let (lines, line_ending_lengths) = split_lines_with_endings(&content);

        // If full_section is enabled, try to parse the file and find code sections
        let code_sections = if options.full_section {
            extract_code_sections(file_path, &content)
        } else {
            None
        };

        search_file_in_memory(regex, file_path, options, &lines, &code_sections, &line_ending_lengths)
    } else {
        // Streaming search (simple case)
        search_file_streaming(regex, file_path, &repo_root, options)
    }
}

/// In-memory search for cases requiring context or code sections
fn search_file_in_memory(
    regex: &Regex,
    file_path: &Path,
    options: &SearchOptions,
    lines: &[String],
    code_sections: &Option<Vec<(usize, usize, String)>>,
    line_ending_lengths: &[usize],
) -> Result<Vec<SearchResult>> {
    let mut results = Vec::new();
    let mut byte_offset = 0;

    for (line_idx, line) in lines.iter().enumerate() {
        let line_number = line_idx + 1;

        // Special handling for empty pattern - match the entire line once
        // An empty regex pattern will match at every position, so we need to handle it specially
        if regex.as_str().is_empty() {
            // Empty pattern matches the whole line once (grep compatibility)
            let preview = if options.full_section {
                // Try to find the containing code section
                if let Some(sections) = code_sections {
                    if let Some(section) = find_containing_section(sections, line_idx) {
                        section.clone()
                    } else {
                        // Fall back to context lines if no section found
                        get_context_preview(lines, line_idx, options)
                    }
                } else {
                    get_context_preview(lines, line_idx, options)
                }
            } else {
                get_context_preview(lines, line_idx, options)
            };

            results.push(SearchResult {
                file: file_path.to_path_buf(),
                span: Span {
                    byte_start: byte_offset,
                    byte_end: byte_offset + line.len(),
                    line_start: line_number,
                    line_end: line_number,
                },
                score: 1.0,
                preview,
                lang: ck_core::Language::from_path(file_path),
                symbol: None,
                chunk_hash: None,
                index_epoch: None,
            });
        } else {
            // Find all matches in the line with their positions
            for mat in regex.find_iter(line) {
                let preview = if options.full_section {
                    // Try to find the containing code section
                    if let Some(sections) = code_sections {
                        if let Some(section) = find_containing_section(sections, line_idx) {
                            section.clone()
                        } else {
                            // Fall back to context lines if no section found
                            get_context_preview(lines, line_idx, options)
                        }
                    } else {
                        get_context_preview(lines, line_idx, options)
                    }
                } else {
                    get_context_preview(lines, line_idx, options)
                };

                results.push(SearchResult {
                    file: file_path.to_path_buf(),
                    span: Span {
                        byte_start: byte_offset + mat.start(),
                        byte_end: byte_offset + mat.end(),
                        line_start: line_number,
                        line_end: line_number,
                    },
                    score: 1.0,
                    preview,
                    lang: ck_core::Language::from_path(file_path),
                    symbol: None,
                    chunk_hash: None,
                    index_epoch: None,
                });
            }
        }

        // Update byte offset for next line (add line length + actual line ending length)
        byte_offset += line.len();
        byte_offset += line_ending_lengths.get(line_idx).copied().unwrap_or(0);
    }

    Ok(results)
}

/// Streaming search for simple cases without context or code sections
fn search_file_streaming(
    regex: &Regex,
    file_path: &Path,
    repo_root: &Path,
    _options: &SearchOptions,
) -> Result<Vec<SearchResult>> {
    use std::io::{BufRead, BufReader};

    let content_path = resolve_content_path(file_path, repo_root)?;
    let file = std::fs::File::open(&content_path)?;
    let mut reader = BufReader::new(file);

    let mut results = Vec::new();
    let mut line = String::new();
    let mut byte_offset = 0usize;
    let mut line_number = 1usize;

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            break;
        }

        // Determine the length of the trailing line ending (if any) and
        // normalise the line buffer so it no longer contains newline bytes.
        let mut newline_len = 0usize;
        if line.ends_with("\r\n") {
            line.pop(); // remove \n
            line.pop(); // remove \r
            newline_len = 2;
        } else if line.ends_with(['\n', '\r']) {
            line.pop();
            newline_len = 1;
        }

        // Old Mac-style files may use bare carriage returns as separators.
        // When the trimmed line still contains '\r' characters, treat them as
        // record separators so the byte offsets remain accurate.
        let treat_cr_as_newline = line.contains('\r');

        if treat_cr_as_newline {
            let bytes = line.as_bytes();
            let mut segment_start = 0usize;
            while segment_start <= bytes.len() {
                match bytes[segment_start..].iter().position(|&b| b == b'\r') {
                    Some(rel_idx) => {
                        let idx = segment_start + rel_idx;
                        let segment_bytes = &bytes[segment_start..idx];
                        let segment_str = std::str::from_utf8(segment_bytes)?;
                        process_streaming_line(
                            regex,
                            file_path,
                            segment_str,
                            line_number,
                            byte_offset,
                            &mut results,
                        );
                        byte_offset += segment_bytes.len() + 1; // account for \r
                        line_number += 1;
                        segment_start = idx + 1;
                    }
                    None => {
                        let segment_bytes = &bytes[segment_start..];
                        let segment_str = std::str::from_utf8(segment_bytes)?;
                        process_streaming_line(
                            regex,
                            file_path,
                            segment_str,
                            line_number,
                            byte_offset,
                            &mut results,
                        );
                        byte_offset += segment_bytes.len();
                        line_number += 1;
                        break;
                    }
                }
            }
            byte_offset += newline_len;
        } else {
            let line_str = line.as_str();
            process_streaming_line(
                regex,
                file_path,
                line_str,
                line_number,
                byte_offset,
                &mut results,
            );
            byte_offset += line_str.len() + newline_len;
            line_number += 1;
        }
    }

    Ok(results)
}

fn process_streaming_line(
    regex: &Regex,
    file_path: &Path,
    line: &str,
    line_number: usize,
    byte_offset: usize,
    results: &mut Vec<SearchResult>,
) {
    if regex.as_str().is_empty() {
        results.push(SearchResult {
            file: file_path.to_path_buf(),
            span: Span {
                byte_start: byte_offset,
                byte_end: byte_offset + line.len(),
                line_start: line_number,
                line_end: line_number,
            },
            score: 1.0,
            preview: line.to_string(),
            lang: ck_core::Language::from_path(file_path),
            symbol: None,
            chunk_hash: None,
            index_epoch: None,
        });
    } else {
        for mat in regex.find_iter(line) {
            results.push(SearchResult {
                file: file_path.to_path_buf(),
                span: Span {
                    byte_start: byte_offset + mat.start(),
                    byte_end: byte_offset + mat.end(),
                    line_start: line_number,
                    line_end: line_number,
                },
                score: 1.0,
                preview: line.to_string(),
                lang: ck_core::Language::from_path(file_path),
                symbol: None,
                chunk_hash: None,
                index_epoch: None,
            });
        }
    }
}

async fn lexical_search(options: &SearchOptions) -> Result<Vec<SearchResult>> {
    // Handle both files and directories and reuse nearest existing .ck index up the tree
    let index_root = find_nearest_index_root(&options.path).unwrap_or_else(|| {
        if options.path.is_file() {
            options.path.parent().unwrap_or(&options.path).to_path_buf()
        } else {
            options.path.clone()
        }
    });

    let index_dir = index_root.join(".ck");
    if !index_dir.exists() {
        return Err(CkError::Index("No index found. Run 'ck index' first.".to_string()).into());
    }

    let tantivy_index_path = index_dir.join("tantivy_index");

    if !tantivy_index_path.exists() {
        return build_tantivy_index(options).await;
    }

    let mut schema_builder = Schema::builder();
    let content_field = schema_builder.add_text_field("content", TEXT | STORED);
    let path_field = schema_builder.add_text_field("path", TEXT | STORED);
    let _schema = schema_builder.build();

    let index = Index::open_in_dir(&tantivy_index_path)
        .map_err(|e| CkError::Index(format!("Failed to open tantivy index: {}", e)))?;

    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommitWithDelay)
        .try_into()
        .map_err(|e| CkError::Index(format!("Failed to create index reader: {}", e)))?;

    let searcher = reader.searcher();
    let query_parser = QueryParser::for_index(&index, vec![content_field]);

    let query = query_parser
        .parse_query(&options.query)
        .map_err(|e| CkError::Search(format!("Failed to parse query: {}", e)))?;

    let top_docs = if let Some(top_k) = options.top_k {
        searcher.search(&query, &TopDocs::with_limit(top_k))?
    } else {
        searcher.search(&query, &TopDocs::with_limit(100))?
    };

    // First, collect all results with raw scores
    let mut raw_results = Vec::new();
    for (_score, doc_address) in top_docs {
        let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
        let path_text = retrieved_doc
            .get_first(path_field)
            .map(|field_value| field_value.as_str().unwrap_or(""))
            .unwrap_or("");
        let content_text = retrieved_doc
            .get_first(content_field)
            .map(|field_value| field_value.as_str().unwrap_or(""))
            .unwrap_or("");

        let file_path = PathBuf::from(path_text);
        let preview = if options.full_section {
            content_text.to_string()
        } else {
            content_text.lines().take(3).collect::<Vec<_>>().join("\n")
        };

        raw_results.push((
            _score,
            SearchResult {
                file: file_path,
                span: Span {
                    byte_start: 0,
                    byte_end: content_text.len(),
                    line_start: 1,
                    line_end: content_text.lines().count(),
                },
                score: _score,
                preview,
                lang: ck_core::Language::from_path(&PathBuf::from(path_text)),
                symbol: None,
                chunk_hash: None,
                index_epoch: None,
            },
        ));
    }

    // Normalize scores to 0-1 range and apply threshold
    let mut results = Vec::new();
    if !raw_results.is_empty() {
        let max_score = raw_results
            .iter()
            .map(|(score, _)| *score)
            .fold(0.0f32, f32::max);
        if max_score > 0.0 {
            for (raw_score, mut result) in raw_results {
                let normalized_score = raw_score / max_score;

                // Apply threshold filtering with normalized score
                if let Some(threshold) = options.threshold
                    && normalized_score < threshold
                {
                    continue;
                }

                result.score = normalized_score;
                results.push(result);
            }
        }
    }

    Ok(results)
}

async fn build_tantivy_index(options: &SearchOptions) -> Result<Vec<SearchResult>> {
    // Handle both files and directories by finding the appropriate directory for indexing
    let index_root = if options.path.is_file() {
        options.path.parent().unwrap_or(&options.path)
    } else {
        &options.path
    };

    let index_dir = index_root.join(".ck");
    let tantivy_index_path = index_dir.join("tantivy_index");

    fs::create_dir_all(&tantivy_index_path)?;

    let mut schema_builder = Schema::builder();
    let content_field = schema_builder.add_text_field("content", TEXT | STORED);
    let path_field = schema_builder.add_text_field("path", TEXT | STORED);
    let schema = schema_builder.build();

    let index = Index::create_in_dir(&tantivy_index_path, schema.clone())
        .map_err(|e| CkError::Index(format!("Failed to create tantivy index: {}", e)))?;

    let mut index_writer = index
        .writer(50_000_000)
        .map_err(|e| CkError::Index(format!("Failed to create index writer: {}", e)))?;

    let files = collect_files(index_root, true, &options.exclude_patterns)?;

    for file_path in &files {
        if let Ok(content) = fs::read_to_string(file_path) {
            let doc = doc!(
                content_field => content,
                path_field => file_path.display().to_string()
            );
            index_writer.add_document(doc)?;
        }
    }

    index_writer
        .commit()
        .map_err(|e| CkError::Index(format!("Failed to commit index: {}", e)))?;

    // After building, search again with the same options
    let tantivy_index_path = index_root.join(".ck").join("tantivy_index");
    let mut schema_builder = Schema::builder();
    let content_field = schema_builder.add_text_field("content", TEXT | STORED);
    let path_field = schema_builder.add_text_field("path", TEXT | STORED);
    let _schema = schema_builder.build();

    let index = Index::open_in_dir(&tantivy_index_path)
        .map_err(|e| CkError::Index(format!("Failed to open tantivy index: {}", e)))?;

    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommitWithDelay)
        .try_into()
        .map_err(|e| CkError::Index(format!("Failed to create index reader: {}", e)))?;

    let searcher = reader.searcher();
    let query_parser = QueryParser::for_index(&index, vec![content_field]);

    let query = query_parser
        .parse_query(&options.query)
        .map_err(|e| CkError::Search(format!("Failed to parse query: {}", e)))?;

    let top_docs = if let Some(top_k) = options.top_k {
        searcher.search(&query, &TopDocs::with_limit(top_k))?
    } else {
        searcher.search(&query, &TopDocs::with_limit(100))?
    };

    // First, collect all results with raw scores
    let mut raw_results = Vec::new();
    for (_score, doc_address) in top_docs {
        let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
        let path_text = retrieved_doc
            .get_first(path_field)
            .map(|field_value| field_value.as_str().unwrap_or(""))
            .unwrap_or("");
        let content_text = retrieved_doc
            .get_first(content_field)
            .map(|field_value| field_value.as_str().unwrap_or(""))
            .unwrap_or("");

        let file_path = PathBuf::from(path_text);
        let preview = if options.full_section {
            content_text.to_string()
        } else {
            content_text.lines().take(3).collect::<Vec<_>>().join("\n")
        };

        raw_results.push((
            _score,
            SearchResult {
                file: file_path,
                span: Span {
                    byte_start: 0,
                    byte_end: content_text.len(),
                    line_start: 1,
                    line_end: content_text.lines().count(),
                },
                score: _score,
                preview,
                lang: ck_core::Language::from_path(&PathBuf::from(path_text)),
                symbol: None,
                chunk_hash: None,
                index_epoch: None,
            },
        ));
    }

    // Normalize scores to 0-1 range and apply threshold
    let mut results = Vec::new();
    if !raw_results.is_empty() {
        let max_score = raw_results
            .iter()
            .map(|(score, _)| *score)
            .fold(0.0f32, f32::max);
        if max_score > 0.0 {
            for (raw_score, mut result) in raw_results {
                let normalized_score = raw_score / max_score;

                // Apply threshold filtering with normalized score
                if let Some(threshold) = options.threshold
                    && normalized_score < threshold
                {
                    continue;
                }

                result.score = normalized_score;
                results.push(result);
            }
        }
    }

    Ok(results)
}

#[allow(dead_code)]
async fn hybrid_search(options: &SearchOptions) -> Result<Vec<SearchResult>> {
    hybrid_search_with_progress(options, None).await
}

async fn hybrid_search_with_progress(
    options: &SearchOptions,
    progress_callback: Option<SearchProgressCallback>,
) -> Result<Vec<SearchResult>> {
    if let Some(ref callback) = progress_callback {
        callback("Running regex search...");
    }
    let regex_results = regex_search(options)?;

    if let Some(ref callback) = progress_callback {
        callback("Running semantic search...");
    }
    let semantic_results = semantic_search_v3_with_progress(options, progress_callback).await?;

    let mut combined = HashMap::new();

    for (rank, result) in regex_results.iter().enumerate() {
        let key = format!("{}:{}", result.file.display(), result.span.line_start);
        combined
            .entry(key)
            .or_insert(Vec::new())
            .push((rank + 1, result.clone()));
    }

    for (rank, result) in semantic_results.matches.iter().enumerate() {
        let key = format!("{}:{}", result.file.display(), result.span.line_start);
        combined
            .entry(key)
            .or_insert(Vec::new())
            .push((rank + 1, result.clone()));
    }

    // Calculate RRF scores according to original paper: RRFscore(d) = Σ(r∈R) 1/(k + r(d))
    let mut rrf_results: Vec<SearchResult> = combined
        .into_values()
        .map(|ranks| {
            let mut result = ranks[0].1.clone();
            let rrf_score = ranks
                .iter()
                .map(|(rank, _)| 1.0 / (60.0 + *rank as f32))
                .sum();
            result.score = rrf_score;
            result
        })
        .filter(|result| {
            // Apply threshold filtering to raw RRF scores
            if let Some(threshold) = options.threshold {
                result.score >= threshold
            } else {
                true
            }
        })
        .collect();

    // Sort by RRF score (highest first)
    rrf_results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if let Some(top_k) = options.top_k {
        rrf_results.truncate(top_k);
    }

    Ok(rrf_results)
}

fn build_globset(patterns: &[String]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pat in patterns {
        // Treat patterns as filename or directory globs
        if let Ok(glob) = Glob::new(pat) {
            builder.add(glob);
        }
    }
    builder.build().unwrap_or_else(|_| GlobSet::empty())
}

fn should_exclude_path(path: &Path, exclude_patterns: &[String]) -> bool {
    let globset = build_globset(exclude_patterns);
    // Match against each path component and the full path
    if globset.is_match(path) {
        return true;
    }
    for component in path.components() {
        if let std::path::Component::Normal(name) = component
            && globset.is_match(name)
        {
            return true;
        }
    }
    false
}

fn collect_files(
    path: &Path,
    recursive: bool,
    exclude_patterns: &[String],
) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let globset = build_globset(exclude_patterns);

    if path.is_file() {
        // Always add single files, even if they're excluded (user explicitly requested)
        files.push(path.to_path_buf());
    } else if recursive {
        for entry in WalkDir::new(path).into_iter().filter_entry(|e| {
            // Skip excluded directories entirely for efficiency
            let name = e.file_name();
            !globset.is_match(e.path()) && !globset.is_match(name)
        }) {
            match entry {
                Ok(entry) => {
                    if entry.file_type().is_file()
                        && !should_exclude_path(entry.path(), exclude_patterns)
                    {
                        files.push(entry.path().to_path_buf());
                    }
                }
                Err(e) => {
                    // Log directory traversal errors but continue processing
                    tracing::debug!("Skipping path due to error: {}", e);
                    continue;
                }
            }
        }
    } else {
        match fs::read_dir(path) {
            Ok(read_dir) => {
                for entry in read_dir {
                    match entry {
                        Ok(entry) => {
                            let path = entry.path();
                            if path.is_file() && !should_exclude_path(&path, exclude_patterns) {
                                files.push(path);
                            }
                        }
                        Err(e) => {
                            tracing::debug!("Skipping directory entry due to error: {}", e);
                            continue;
                        }
                    }
                }
            }
            Err(e) => {
                tracing::debug!("Cannot read directory {:?}: {}", path, e);
                return Err(e.into());
            }
        }
    }

    Ok(files)
}

#[allow(clippy::too_many_arguments)]
async fn ensure_index_updated_with_progress(
    path: &Path,
    force_reindex: bool,
    need_embeddings: bool,
    progress_callback: Option<ck_index::ProgressCallback>,
    detailed_progress_callback: Option<ck_index::DetailedProgressCallback>,
    respect_gitignore: bool,
    exclude_patterns: &[String],
    model_override: Option<&str>,
) -> Result<()> {
    // Handle both files and directories and reuse nearest existing .ck index up the tree
    let index_root_buf = find_nearest_index_root(path).unwrap_or_else(|| {
        if path.is_file() {
            path.parent().unwrap_or(path).to_path_buf()
        } else {
            path.to_path_buf()
        }
    });
    let index_root = &index_root_buf;

    // If force reindex is requested, always update
    if force_reindex {
        let stats = ck_index::smart_update_index_with_detailed_progress(
            index_root,
            false,
            progress_callback,
            detailed_progress_callback,
            need_embeddings,
            respect_gitignore,
            exclude_patterns, // Use search-specific exclude patterns
            model_override,
        )
        .await?;
        if stats.files_indexed > 0 || stats.orphaned_files_removed > 0 {
            tracing::info!(
                "Index updated: {} files indexed, {} orphaned files removed",
                stats.files_indexed,
                stats.orphaned_files_removed
            );
        }
        return Ok(());
    }

    // Always use smart_update_index for incremental updates (handles both new and existing indexes)
    let stats = ck_index::smart_update_index_with_detailed_progress(
        index_root,
        false,
        progress_callback,
        detailed_progress_callback,
        need_embeddings,
        respect_gitignore,
        exclude_patterns,
        model_override,
    )
    .await?;
    if stats.files_indexed > 0 || stats.orphaned_files_removed > 0 {
        tracing::info!(
            "Index updated: {} files indexed, {} orphaned files removed",
            stats.files_indexed,
            stats.orphaned_files_removed
        );
    }

    Ok(())
}

fn get_context_preview(lines: &[String], line_idx: usize, options: &SearchOptions) -> String {
    let before = options.before_context_lines.max(options.context_lines);
    let after = options.after_context_lines.max(options.context_lines);

    if before > 0 || after > 0 {
        let start_idx = line_idx.saturating_sub(before);
        let end_idx = (line_idx + after + 1).min(lines.len());
        lines[start_idx..end_idx].join("\n")
    } else {
        lines[line_idx].to_string()
    }
}

fn extract_code_sections(file_path: &Path, content: &str) -> Option<Vec<(usize, usize, String)>> {
    let lang = ck_core::Language::from_path(file_path)?;

    // Parse the file with tree-sitter and extract function/class sections
    if let Ok(chunks) = ck_chunk::chunk_text(content, Some(lang)) {
        let sections: Vec<(usize, usize, String)> = chunks
            .into_iter()
            .filter(|chunk| {
                matches!(
                    chunk.chunk_type,
                    ck_chunk::ChunkType::Function
                        | ck_chunk::ChunkType::Class
                        | ck_chunk::ChunkType::Method
                )
            })
            .map(|chunk| {
                (
                    chunk.span.line_start - 1, // Convert to 0-based index
                    chunk.span.line_end - 1,
                    chunk.text,
                )
            })
            .collect();

        if sections.is_empty() {
            None
        } else {
            Some(sections)
        }
    } else {
        None
    }
}

fn find_containing_section(
    sections: &[(usize, usize, String)],
    line_idx: usize,
) -> Option<&String> {
    for (start, end, text) in sections {
        if line_idx >= *start && line_idx <= *end {
            return Some(text);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_files(dir: &std::path::Path) -> Vec<PathBuf> {
        let files = vec![
            ("test1.txt", "hello world rust programming"),
            ("test2.rs", "fn main() { println!(\"Hello Rust\"); }"),
            ("test3.py", "print('Hello Python')"),
            ("test4.txt", "machine learning artificial intelligence"),
        ];

        let mut paths = Vec::new();
        for (name, content) in files {
            let path = dir.join(name);
            fs::write(&path, content).unwrap();
            paths.push(path);
        }
        paths
    }

    #[test]
    fn test_extract_lines_from_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_lines.txt");

        // Create a multi-line test file
        let content =
            "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7\nLine 8\nLine 9\nLine 10";
        fs::write(&test_file, content).unwrap();

        // Test extracting lines 3-5 (1-based indexing)
        let result = extract_lines_from_file(&test_file, 3, 5).unwrap();
        assert_eq!(result, "Line 3\nLine 4\nLine 5");

        // Test extracting a single line
        let result = extract_lines_from_file(&test_file, 7, 7).unwrap();
        assert_eq!(result, "Line 7");

        // Test extracting from line 8 to end
        let result = extract_lines_from_file(&test_file, 8, 100).unwrap();
        assert_eq!(result, "Line 8\nLine 9\nLine 10");

        // Test line_start == 0 (should return empty)
        let result = extract_lines_from_file(&test_file, 0, 5).unwrap();
        assert_eq!(result, "");

        // Test line_start > file length (should return empty)
        let result = extract_lines_from_file(&test_file, 20, 25).unwrap();
        assert_eq!(result, "");
    }

    #[tokio::test]
    async fn test_extract_content_from_span() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("code.rs");

        // Create a multi-line code file
        let content = "fn first() {\n    println!(\"First\");\n}\n\nfn second() {\n    println!(\"Second\");\n}\n\nfn third() {\n    println!(\"Third\");\n}";
        fs::write(&test_file, content).unwrap();

        // Test extracting the second function (lines 5-7)
        let span = ck_core::Span {
            byte_start: 0, // Not used in line extraction
            byte_end: 0,   // Not used in line extraction
            line_start: 5,
            line_end: 7,
        };

        let result = extract_content_from_span(&test_file, &span).await.unwrap();
        assert_eq!(result, "fn second() {\n    println!(\"Second\");\n}");

        // Test extracting a single line
        let span = ck_core::Span {
            byte_start: 0,
            byte_end: 0,
            line_start: 2,
            line_end: 2,
        };

        let result = extract_content_from_span(&test_file, &span).await.unwrap();
        assert_eq!(result, "    println!(\"First\");");
    }

    #[test]
    fn test_collect_files() {
        let temp_dir = TempDir::new().unwrap();
        let test_files = create_test_files(temp_dir.path());

        // Test non-recursive
        let files = collect_files(temp_dir.path(), false, &[]).unwrap();
        assert_eq!(files.len(), 4);

        // Test recursive
        let files = collect_files(temp_dir.path(), true, &[]).unwrap();
        assert_eq!(files.len(), 4);

        // Test single file
        let files = collect_files(&test_files[0], false, &[]).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], test_files[0]);
    }

    #[test]
    fn test_regex_search() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path());

        let options = SearchOptions {
            mode: SearchMode::Regex,
            query: "rust".to_string(),
            path: temp_dir.path().to_path_buf(),
            recursive: true,
            ..Default::default()
        };

        let results = regex_search(&options).unwrap();
        assert!(!results.is_empty());

        // Should find matches in files containing "rust"
        let rust_matches: Vec<_> = results
            .iter()
            .filter(|r| r.preview.to_lowercase().contains("rust"))
            .collect();
        assert!(!rust_matches.is_empty());
    }

    #[test]
    fn test_regex_search_case_insensitive() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path());

        let options = SearchOptions {
            mode: SearchMode::Regex,
            query: "HELLO".to_string(),
            path: temp_dir.path().to_path_buf(),
            recursive: true,
            case_insensitive: true,
            ..Default::default()
        };

        let results = regex_search(&options).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_regex_search_fixed_string() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path());

        let options = SearchOptions {
            mode: SearchMode::Regex,
            query: "fn main()".to_string(),
            path: temp_dir.path().to_path_buf(),
            recursive: true,
            fixed_string: true,
            ..Default::default()
        };

        let results = regex_search(&options).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_regex_search_whole_word() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join("word_test.txt"),
            "rust rusty rustacean",
        )
        .unwrap();

        let options = SearchOptions {
            mode: SearchMode::Regex,
            query: "rust".to_string(),
            path: temp_dir.path().to_path_buf(),
            recursive: true,
            whole_word: true,
            ..Default::default()
        };

        let results = regex_search(&options).unwrap();
        assert!(!results.is_empty());
        // Should only match "rust" as a whole word, not "rusty" or "rustacean"
    }

    #[test]
    fn test_regex_search_top_k() {
        let temp_dir = TempDir::new().unwrap();

        // Create multiple files with matches
        for i in 0..10 {
            fs::write(
                temp_dir.path().join(format!("file{}.txt", i)),
                "test content",
            )
            .unwrap();
        }

        let options = SearchOptions {
            mode: SearchMode::Regex,
            query: "test".to_string(),
            path: temp_dir.path().to_path_buf(),
            recursive: true,
            top_k: Some(5),
            ..Default::default()
        };

        let results = regex_search(&options).unwrap();
        assert!(results.len() <= 5);
    }

    #[test]
    fn test_regex_search_span_offsets() {
        // Test that span offsets are correctly calculated for multiple matches on a line
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("spans.txt");
        fs::write(&test_file, "test test test\nline two test\ntest end").unwrap();

        let options = SearchOptions {
            mode: SearchMode::Regex,
            query: "test".to_string(),
            path: test_file.clone(),
            recursive: false,
            ..Default::default()
        };

        let results = regex_search(&options).unwrap();

        // Should find 5 matches total
        assert_eq!(results.len(), 5);

        // Check first line has 3 matches with correct byte offsets
        let line1_matches: Vec<_> = results.iter().filter(|r| r.span.line_start == 1).collect();
        assert_eq!(line1_matches.len(), 3);
        assert_eq!(line1_matches[0].span.byte_start, 0);
        assert_eq!(line1_matches[1].span.byte_start, 5);
        assert_eq!(line1_matches[2].span.byte_start, 10);

        // Check second line match
        let line2_matches: Vec<_> = results.iter().filter(|r| r.span.line_start == 2).collect();
        assert_eq!(line2_matches.len(), 1);
        assert_eq!(line2_matches[0].span.byte_start, 24); // "test test test\n" = 15 bytes, "line two " = 9 bytes

        // Each match should have different byte offsets
        let mut byte_starts: Vec<_> = results.iter().map(|r| r.span.byte_start).collect();
        byte_starts.sort();
        byte_starts.dedup();
        assert_eq!(byte_starts.len(), 5); // All byte_starts should be unique
    }

    #[test]
    fn test_search_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(
            &file_path,
            "line 1: hello\nline 2: world\nline 3: rust programming",
        )
        .unwrap();

        let regex = regex::Regex::new("rust").unwrap();
        let options = SearchOptions::default();

        let results = search_file(&regex, &file_path, &options).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].span.line_start, 3);
        assert!(results[0].preview.contains("rust"));
    }

    #[test]
    fn test_search_file_with_context() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "line 1\nline 2\ntarget line\nline 4\nline 5").unwrap();

        let regex = regex::Regex::new("target").unwrap();
        let options = SearchOptions {
            context_lines: 1,
            ..Default::default()
        };

        let results = search_file(&regex, &file_path, &options).unwrap();
        assert_eq!(results.len(), 1);

        println!("Preview: '{}'", results[0].preview);

        // The target line is line 3, with 1 context line before and after
        // So we should get lines 2, 3, 4
        assert!(results[0].preview.contains("line 2"));
        assert!(results[0].preview.contains("target line"));
        assert!(results[0].preview.contains("line 4"));
    }

    #[tokio::test]
    async fn test_search_main_function() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path());

        let options = SearchOptions {
            mode: SearchMode::Regex,
            query: "hello".to_string(),
            path: temp_dir.path().to_path_buf(),
            recursive: true,
            case_insensitive: true,
            ..Default::default()
        };

        let results = search(&options).await.unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_regex_search_mixed_line_endings() {
        // Regression test for byte offset issues with different line endings
        let temp_dir = TempDir::new().unwrap();

        // Create test file with mixed line endings (Windows \r\n and Unix \n)
        let test_file = temp_dir.path().join("mixed_endings.txt");
        let content = "line1\r\nline2\nline3\r\npattern here\nline5\r\n";
        std::fs::write(&test_file, content).unwrap();

        let options = SearchOptions {
            mode: SearchMode::Regex,
            query: "pattern".to_string(),
            path: test_file.clone(),
            recursive: false,
            ..Default::default()
        };

        let results = search(&options).await.unwrap();
        assert_eq!(results.len(), 1);

        let result = &results[0];
        // Verify byte offsets are correct - should point to start of "pattern"
        let original_content = std::fs::read_to_string(&test_file).unwrap();
        let pattern_start = original_content.find("pattern").unwrap();

        assert_eq!(result.span.byte_start, pattern_start);
        assert_eq!(result.span.line_start, 4); // Fourth line
    }

    #[tokio::test]
    async fn test_regex_search_windows_line_endings() {
        // Regression test specifically for Windows \r\n line endings
        let temp_dir = TempDir::new().unwrap();

        let test_file = temp_dir.path().join("windows_endings.txt");
        let content = "first line\r\nsecond line\r\nmatch this\r\nfourth line\r\n";
        std::fs::write(&test_file, content).unwrap();

        let options = SearchOptions {
            mode: SearchMode::Regex,
            query: "match".to_string(),
            path: test_file.clone(),
            recursive: false,
            ..Default::default()
        };

        let results = search(&options).await.unwrap();
        assert_eq!(results.len(), 1);

        let result = &results[0];

        // Verify the match is on line 3
        assert_eq!(result.span.line_start, 3);

        // Verify byte offset accounts for \r\n endings
        // first line\r\n = 12 bytes, second line\r\n = 13 bytes, total = 25 bytes before "match"
        let expected_byte_start = 25; // Position of "match" in the content
        assert_eq!(result.span.byte_start, expected_byte_start);
    }

    #[test]
    fn test_split_lines_with_endings_helper() {
        // Unix line endings
        let unix_content = "line1\nline2\nline3\n";
        let (unix_lines, unix_endings) = split_lines_with_endings(unix_content);
        assert_eq!(unix_lines, vec!["line1", "line2", "line3"]);
        assert_eq!(unix_endings, vec![1, 1, 1]);

        // Windows line endings
        let windows_content = "line1\r\nline2\r\nline3\r\n";
        let (windows_lines, windows_endings) = split_lines_with_endings(windows_content);
        assert_eq!(windows_lines, vec!["line1", "line2", "line3"]);
        assert_eq!(windows_endings, vec![2, 2, 2]);

        // Old Mac line endings
        let mac_content = "line1\rline2\rline3\r";
        let (mac_lines, mac_endings) = split_lines_with_endings(mac_content);
        assert_eq!(mac_lines, vec!["line1", "line2", "line3"]);
        assert_eq!(mac_endings, vec![1, 1, 1]);

        // Mixed endings
        let mixed_content = "line1\nline2\r\nline3\r";
        let (mixed_lines, mixed_endings) = split_lines_with_endings(mixed_content);
        assert_eq!(mixed_lines, vec!["line1", "line2", "line3"]);
        assert_eq!(mixed_endings, vec![1, 2, 1]);

        // No line endings
        let no_endings = "single line";
        let (no_lines, no_endings_vec) = split_lines_with_endings(no_endings);
        assert_eq!(no_lines, vec!["single line"]);
        assert_eq!(no_endings_vec, vec![0]);
    }
}
