use anyhow::Result;
use ck_core::{CkError, IncludePattern, SearchMode, SearchOptions, SearchResult, Span};
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

fn canonicalize_for_matching(path: &Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }

    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

fn path_matches_include(path: &Path, include_patterns: &[IncludePattern]) -> bool {
    if include_patterns.is_empty() {
        return true;
    }

    let candidate = canonicalize_for_matching(path);
    include_patterns.iter().any(|pattern| {
        if pattern.is_dir {
            candidate.starts_with(&pattern.path)
        } else {
            candidate == pattern.path
        }
    })
}

fn filter_files_by_include(
    files: Vec<PathBuf>,
    include_patterns: &[IncludePattern],
) -> Vec<PathBuf> {
    if include_patterns.is_empty() {
        return files;
    }

    files
        .into_iter()
        .filter(|path| path_matches_include(path, include_patterns))
        .collect()
}

fn find_nearest_index_root(path: &Path) -> Option<StdPathBuf> {
    let mut current = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    loop {
        if ck_core::index_exists(current) {
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
    pub alias: String,
    pub config: ck_models::ModelConfig,
}

impl ResolvedModel {
    pub fn canonical_name(&self) -> &str {
        self.config.name.as_str()
    }

    pub fn dimensions(&self) -> usize {
        self.config.dimensions
    }
}

fn legacy_model_config(name: &str, dimensions: usize) -> ck_models::ModelConfig {
    ck_models::ModelConfig {
        name: name.to_string(),
        provider: "fastembed".to_string(),
        dimensions,
        max_tokens: 8192,
        description: "Legacy ck embedding model preserved for backwards compatibility".to_string(),
    }
}

pub(crate) fn resolve_model_from_root(
    index_root: &Path,
    cli_model: Option<&str>,
) -> Result<ResolvedModel> {
    use ck_models::ModelRegistry;

    let registry = ModelRegistry::default();
    let index_dir = ck_core::index_dir(index_root);
    let manifest_path = index_dir.join("manifest.json");

    if manifest_path.exists() {
        let data = std::fs::read(&manifest_path)?;
        let manifest: ck_index::IndexManifest = serde_json::from_slice(&data)?;

        if let Some(existing_model) = manifest.embedding_model {
            let dims_hint = manifest.embedding_dimensions.unwrap_or(384);
            let resolved_existing = match registry.resolve(Some(existing_model.as_str())) {
                Ok((alias, config)) => ResolvedModel { alias, config },
                Err(_) => ResolvedModel {
                    alias: existing_model.clone(),
                    config: legacy_model_config(&existing_model, dims_hint),
                },
            };

            if let Some(requested) = cli_model {
                let (requested_alias, requested_config) = registry
                    .resolve(Some(requested))
                    .map_err(|e| CkError::Embedding(e.to_string()))?;

                if requested_config.name != resolved_existing.config.name {
                    let suggested_alias = resolved_existing.alias.clone();
                    return Err(CkError::Embedding(format!(
                        "Index was built with embedding model '{}' (alias '{}'), but '--model {}' was requested. To switch models run `ck --clean .` then `ck --index --model {}`. To keep using this index rerun your command with '--model {}'.",
                        resolved_existing.config.name,
                        suggested_alias,
                        requested,
                        requested,
                        suggested_alias
                    ))
                    .into());
                }

                return Ok(ResolvedModel {
                    alias: requested_alias,
                    config: requested_config,
                });
            }

            return Ok(resolved_existing);
        }
    }

    let (alias, config) = registry
        .resolve(cli_model)
        .map_err(|e| CkError::Embedding(e.to_string()))?;

    Ok(ResolvedModel { alias, config })
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
/// Summary of index maintenance performed automatically before a search.
#[derive(Debug, Clone, Default)]
pub struct IndexUpdate {
    pub files_indexed: usize,
    pub orphaned_files_removed: usize,
    /// Wall-clock time spent checking and updating the index, in milliseconds.
    /// This covers the staleness check even when no files needed work.
    pub duration_ms: u64,
}

impl IndexUpdate {
    /// True when the index actually changed (files re-indexed or removed),
    /// as opposed to a no-op staleness check.
    pub fn did_work(&self) -> bool {
        self.files_indexed > 0 || self.orphaned_files_removed > 0
    }
}

/// Search results plus what auto-indexing did to produce them, so callers can
/// report search latency separately from index-build latency.
#[derive(Debug)]
pub struct SearchOutcome {
    pub results: ck_core::SearchResults,
    /// `None` for regex mode (which never touches the index).
    pub index_update: Option<IndexUpdate>,
}

pub async fn search_enhanced_with_indexing_progress(
    options: &SearchOptions,
    progress_callback: Option<SearchProgressCallback>,
    indexing_progress_callback: Option<IndexingProgressCallback>,
    detailed_indexing_progress_callback: Option<DetailedIndexingProgressCallback>,
) -> Result<ck_core::SearchResults> {
    let outcome = search_enhanced_with_outcome(
        options,
        progress_callback,
        indexing_progress_callback,
        detailed_indexing_progress_callback,
    )
    .await?;
    Ok(outcome.results)
}

pub async fn search_enhanced_with_outcome(
    options: &SearchOptions,
    progress_callback: Option<SearchProgressCallback>,
    indexing_progress_callback: Option<IndexingProgressCallback>,
    detailed_indexing_progress_callback: Option<DetailedIndexingProgressCallback>,
) -> Result<SearchOutcome> {
    // Validate that the search path exists
    if !options.path.exists() {
        return Err(ck_core::CkError::Search(format!(
            "Path does not exist: {}",
            options.path.display()
        ))
        .into());
    }

    // Auto-update index if needed (unless it's regex-only mode)
    let mut index_update = None;
    if !matches!(options.mode, SearchMode::Regex) {
        let need_embeddings = matches!(options.mode, SearchMode::Semantic | SearchMode::Hybrid);
        let file_options = ck_core::FileCollectionOptions::from(options);
        let started = std::time::Instant::now();
        let update_stats = ensure_index_updated_with_progress(
            &options.path,
            options.reindex,
            need_embeddings,
            indexing_progress_callback,
            detailed_indexing_progress_callback,
            &file_options,
            options.embedding_model.as_deref(),
        )
        .await?;
        index_update = Some(IndexUpdate {
            files_indexed: update_stats
                .as_ref()
                .map(|s| s.files_indexed)
                .unwrap_or_default(),
            orphaned_files_removed: update_stats
                .as_ref()
                .map(|s| s.orphaned_files_removed)
                .unwrap_or_default(),
            duration_ms: started.elapsed().as_millis() as u64,
        });
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

    Ok(SearchOutcome {
        results: search_results,
        index_update,
    })
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
        let file_options = ck_core::FileCollectionOptions {
            respect_gitignore: options.respect_gitignore,
            use_ckignore: options.use_ckignore,
            exclude_patterns: options.exclude_patterns.clone(),
        };
        let collected = ck_index::collect_files(&options.path, &file_options)?;
        filter_files_by_include(collected, &options.include_patterns)
    } else {
        // For non-recursive, use the local collect_files
        let collected = collect_files(&options.path, should_recurse, &options.exclude_patterns)?;
        filter_files_by_include(collected, &options.include_patterns)
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

        search_file_in_memory(
            regex,
            file_path,
            options,
            &lines,
            &code_sections,
            &line_ending_lengths,
        )
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

/// Name of the metadata file (inside `.ck`) recording the corpus fingerprint
/// the tantivy index was built from, so staleness is detectable.
const TANTIVY_META_FILE: &str = "tantivy_index.meta";

/// Fingerprint of the file set a tantivy index covers: path, mtime and size
/// of every corpus file. Any added, removed, or modified file changes the
/// fingerprint, as does a different exclude-pattern set (it changes the
/// collected file list).
fn lexical_corpus_fingerprint(files: &[PathBuf]) -> String {
    let mut entries: Vec<String> = files
        .iter()
        .map(|f| {
            let (mtime, size) = fs::metadata(f)
                .map(|m| {
                    let mtime = m
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_nanos())
                        .unwrap_or(0);
                    (mtime, m.len())
                })
                .unwrap_or((0, 0));
            format!("{}\x00{}\x00{}", f.display(), mtime, size)
        })
        .collect();
    entries.sort_unstable();

    let mut hasher = blake3::Hasher::new();
    for entry in &entries {
        hasher.update(entry.as_bytes());
        hasher.update(b"\n");
    }
    hasher.finalize().to_hex().to_string()
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

    let index_dir = ck_core::index_dir(&index_root);
    if !index_dir.exists() {
        return Err(CkError::Index("No index found. Run 'ck index' first.".to_string()).into());
    }
    // Refuse to serve results from an index dir that a different root claimed
    // via a CK_INDEX_DIR basename-hash collision. No-op in-tree.
    ck_core::check_index_root_marker(&index_root)?;

    let tantivy_index_path = index_dir.join("tantivy_index");

    // The tantivy index always covers the whole index root (include patterns
    // are applied per result at search time below), so corpus membership only
    // depends on the root and the exclusion rules.
    //
    // Collection goes through ck_index::collect_files — the same walker the
    // regex and semantic paths use — so gitignore/.ckignore semantics match
    // and exclude patterns apply relative to the walk root. The engine-local
    // collect_files matched exclude globs against every *absolute* path
    // component, so a corpus under e.g. /tmp on Linux matched the default
    // "tmp" exclude and silently produced an empty lexical index.
    let file_options = ck_core::FileCollectionOptions {
        respect_gitignore: options.respect_gitignore,
        use_ckignore: options.use_ckignore,
        exclude_patterns: options.exclude_patterns.clone(),
    };
    let corpus = ck_index::collect_files(&index_root, &file_options)?;
    let fingerprint = lexical_corpus_fingerprint(&corpus);
    let meta_path = index_dir.join(TANTIVY_META_FILE);
    let is_fresh = tantivy_index_path.exists()
        && fs::read_to_string(&meta_path)
            .map(|stored| stored.trim() == fingerprint)
            .unwrap_or(false);

    if !is_fresh {
        // Serialize with index mutations (and concurrent lexical rebuilds);
        // re-check freshness after acquiring in case another process just
        // rebuilt the same corpus.
        let _lock = ck_index::acquire_index_write_lock(&index_dir)?;
        let still_stale = !tantivy_index_path.exists()
            || fs::read_to_string(&meta_path)
                .map(|stored| stored.trim() != fingerprint)
                .unwrap_or(true);
        if still_stale {
            tracing::info!(
                "Lexical index stale or missing for {}; rebuilding from {} files",
                index_root.display(),
                corpus.len()
            );
            build_tantivy_index(&tantivy_index_path, &corpus)?;
            fs::write(&meta_path, &fingerprint)?;
        }
    }

    let mut schema_builder = Schema::builder();
    let content_field = schema_builder.add_text_field("content", TEXT | STORED);
    let path_field = schema_builder.add_text_field("path", TEXT | STORED);
    let _schema = schema_builder.build();

    let index = Index::open_in_dir(&tantivy_index_path)
        .map_err(|e| CkError::Index(format!("Failed to open tantivy index: {e}")))?;

    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommitWithDelay)
        .try_into()
        .map_err(|e| CkError::Index(format!("Failed to create index reader: {e}")))?;

    let searcher = reader.searcher();
    let query_parser = QueryParser::for_index(&index, vec![content_field]);

    let query = query_parser
        .parse_query(&options.query)
        .map_err(|e| CkError::Search(format!("Failed to parse query: {e}")))?;

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
        if !path_matches_include(&file_path, &options.include_patterns) {
            continue;
        }
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

/// (Re)build the tantivy index at `tantivy_index_path` over `files`.
/// Callers must hold the index write lock. Any existing index is replaced —
/// tantivy has no cheap way to diff segments against a changed corpus, and a
/// full text-only rebuild is fast relative to embedding work.
///
/// Searching the result happens in [`lexical_search`]; this function builds
/// only (its previous incarnation duplicated the entire search/read path,
/// which had already drifted — the rebuilt-path copy lost include filtering).
fn build_tantivy_index(tantivy_index_path: &Path, files: &[PathBuf]) -> Result<()> {
    if tantivy_index_path.exists() {
        fs::remove_dir_all(tantivy_index_path)?;
    }
    fs::create_dir_all(tantivy_index_path)?;

    let mut schema_builder = Schema::builder();
    let content_field = schema_builder.add_text_field("content", TEXT | STORED);
    let path_field = schema_builder.add_text_field("path", TEXT | STORED);
    let schema = schema_builder.build();

    let index = Index::create_in_dir(tantivy_index_path, schema)
        .map_err(|e| CkError::Index(format!("Failed to create tantivy index: {e}")))?;

    let mut index_writer = index
        .writer(50_000_000)
        .map_err(|e| CkError::Index(format!("Failed to create index writer: {e}")))?;

    for file_path in files {
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
        .map_err(|e| CkError::Index(format!("Failed to commit index: {e}")))?;

    Ok(())
}

#[allow(dead_code)]
async fn hybrid_search(options: &SearchOptions) -> Result<Vec<SearchResult>> {
    hybrid_search_with_progress(options, None).await
}

/// English filler words excluded from the keyword arm of hybrid search.
/// The keyword arm has no relevance scoring of its own, so terms like "the"
/// or "does" would flood it with matches that pollute the fused ranking.
const HYBRID_STOPWORDS: &[&str] = &[
    "the", "and", "for", "with", "that", "this", "from", "into", "when", "where", "how", "what",
    "why", "who", "which", "does", "are", "was", "were", "has", "have", "had", "can", "could",
    "should", "would", "will", "its", "use", "uses", "used", "using", "between", "over", "under",
    "than", "then", "them", "they", "their", "there", "your", "our", "not", "but", "all", "any",
    "each", "other", "some", "such", "only", "own", "same", "more", "most", "very", "happen",
    "happens", "get", "gets", "code", "function", "where", "place",
];

/// Distinct meaningful terms from a hybrid query, for keyword matching.
fn hybrid_query_terms(query: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    query
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .map(str::to_lowercase)
        .filter(|t| t.len() >= 3 && !HYBRID_STOPWORDS.contains(&t.as_str()))
        .filter(|t| seen.insert(t.clone()))
        .collect()
}

/// The keyword arm of hybrid search. Natural-language queries ("how are stale
/// entries cleaned up") almost never match the corpus as a literal regex,
/// which previously degraded hybrid search to semantic-only exactly when the
/// keyword boost mattered most. When the literal pattern finds nothing, retry
/// with a case-insensitive alternation of the query's meaningful terms and
/// rank the matches by how many distinct terms each line covers (the regex
/// engine itself has no scoring — raw traversal order would rank a line
/// matching one common term above a line matching all of them).
fn hybrid_keyword_search(options: &SearchOptions) -> Result<(Vec<SearchResult>, bool)> {
    let literal = regex_search(options)?;
    if !literal.is_empty() || options.fixed_string {
        return Ok((literal, false));
    }

    let terms = hybrid_query_terms(&options.query);
    if terms.len() < 2 {
        return Ok((literal, false));
    }

    let mut keyword_options = options.clone();
    keyword_options.query = terms
        .iter()
        .map(|t| regex::escape(t))
        .collect::<Vec<_>>()
        .join("|");
    keyword_options.case_insensitive = true;
    // The fallback pattern matches single terms, so it can hit far more lines
    // than the literal pattern would; don't let regex_search's internal top_k
    // cut them in traversal order before we rank them below.
    keyword_options.top_k = None;
    let matches = regex_search(&keyword_options)?;

    // Rank matches by the rarity of the terms they contain (IDF over the
    // match set): a line containing a term that matched 3 lines corpus-wide
    // says far more than one containing a term that matched 500. Without
    // this, filler-adjacent terms ("results", "semantic" in a search tool's
    // own repo) drown the discriminative ones.
    let lowered: Vec<String> = matches.iter().map(|r| r.preview.to_lowercase()).collect();
    let doc_freq: Vec<usize> = terms
        .iter()
        .map(|t| {
            lowered
                .iter()
                .filter(|line| line.contains(String::as_str(t)))
                .count()
        })
        .collect();
    let total = matches.len() as f32;
    // String::as_str fully qualified above and below: tantivy's `Value`
    // trait is in scope and its `as_str(&self) -> Option<&str>` would win
    // method resolution.
    let mut scored: Vec<(f32, SearchResult)> = matches
        .into_iter()
        .zip(lowered)
        .map(|(r, line)| {
            let weight: f32 = terms
                .iter()
                .zip(&doc_freq)
                .filter(|(t, _)| line.contains(String::as_str(t)))
                .map(|(_, &df)| ((total + 1.0) / (df as f32 + 1.0)).ln())
                .sum();
            (weight, r)
        })
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    if let Some(top_k) = options.top_k {
        scored.truncate(top_k);
    }
    Ok((scored.into_iter().map(|(_, r)| r).collect(), true))
}

/// Fuse keyword and semantic rankings with Reciprocal Rank Fusion:
/// RRFscore(d) = Σ(r∈R) 1/(k + r(d)), k = 60 (original paper's constant).
///
/// Results are keyed by logical location: a semantic chunk owns every keyword
/// hit whose line falls inside its span. (The previous exact `file:line` key
/// meant chunk-level semantic results and line-level keyword results never
/// shared a key, so nothing ever actually fused.) Each ranking contributes at
/// most one rank per key — its best. Fused entries keep the semantic chunk's
/// content, which carries more context than a single matched line.
/// `keyword_weight` scales the keyword arm's contribution. Literal matches
/// (the user's pattern actually occurs in the corpus) deserve full weight;
/// the synthesized term-OR fallback is a much weaker signal — at full weight
/// its noise demotes results the semantic ranking already had right.
fn rrf_fuse(
    keyword_results: &[SearchResult],
    semantic_results: &[SearchResult],
    keyword_weight: f32,
) -> Vec<SearchResult> {
    const RRF_K: f32 = 60.0;

    struct Fused {
        result: SearchResult,
        keyword_rank: Option<usize>,
        semantic_rank: Option<usize>,
    }

    // Per-file semantic spans, for mapping keyword hits into chunks
    let mut sem_spans: HashMap<String, Vec<(usize, usize, String)>> = HashMap::new();
    let mut combined: HashMap<String, Fused> = HashMap::new();

    for (rank, result) in semantic_results.iter().enumerate() {
        let file = result.file.display().to_string();
        let key = format!(
            "{}:{}-{}",
            file, result.span.line_start, result.span.line_end
        );
        sem_spans.entry(file).or_default().push((
            result.span.line_start,
            result.span.line_end,
            key.clone(),
        ));
        combined.entry(key).or_insert(Fused {
            result: result.clone(),
            keyword_rank: None,
            semantic_rank: Some(rank + 1),
        });
    }

    for (rank, result) in keyword_results.iter().enumerate() {
        let file = result.file.display().to_string();
        let key = sem_spans
            .get(&file)
            .and_then(|spans| {
                spans
                    .iter()
                    .find(|(start, end, _)| (*start..=*end).contains(&result.span.line_start))
                    .map(|(_, _, key)| key.clone())
            })
            .unwrap_or_else(|| format!("{}:{}", file, result.span.line_start));
        combined
            .entry(key)
            .and_modify(|fused| {
                if fused.keyword_rank.is_none() {
                    fused.keyword_rank = Some(rank + 1);
                }
            })
            .or_insert(Fused {
                result: result.clone(),
                keyword_rank: Some(rank + 1),
                semantic_rank: None,
            });
    }

    combined
        .into_values()
        .map(|fused| {
            let mut result = fused.result;
            let rank_score = |rank: Option<usize>| rank.map_or(0.0, |r| 1.0 / (RRF_K + r as f32));
            result.score =
                keyword_weight * rank_score(fused.keyword_rank) + rank_score(fused.semantic_rank);
            result
        })
        .collect()
}

async fn hybrid_search_with_progress(
    options: &SearchOptions,
    progress_callback: Option<SearchProgressCallback>,
) -> Result<Vec<SearchResult>> {
    // Fetch more candidates from each arm than the final cut: fusion can
    // only promote results it sees, and both regex_search and the semantic
    // ranking truncate to top_k internally — at the original top_k, a result
    // boosted by the other arm would never reach the fusion stage at all.
    let mut arm_options = options.clone();
    arm_options.top_k = options.top_k.map(|k| (k * 5).max(50));

    if let Some(ref callback) = progress_callback {
        callback("Running keyword search...");
    }
    let (keyword_results, keyword_is_fallback) = hybrid_keyword_search(&arm_options)?;

    if let Some(ref callback) = progress_callback {
        callback("Running semantic search...");
    }
    let semantic_results =
        semantic_search_v3_with_progress(&arm_options, progress_callback).await?;

    let keyword_weight = if keyword_is_fallback { 0.3 } else { 1.0 };
    let mut rrf_results = rrf_fuse(&keyword_results, &semantic_results.matches, keyword_weight);

    // Apply threshold filtering to raw RRF scores
    if let Some(threshold) = options.threshold {
        rrf_results.retain(|result| result.score >= threshold);
    }

    rrf_results.retain(|result| path_matches_include(&result.file, &options.include_patterns));

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

fn should_exclude_path(path: &Path, globset: &GlobSet) -> bool {
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
                    if entry.file_type().is_file() && !should_exclude_path(entry.path(), &globset) {
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
                            if path.is_file() && !should_exclude_path(&path, &globset) {
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

/// Returns the indexing stats when a directory-level smart update ran, or
/// `None` for the single-file fast path (which reports no stats).
async fn ensure_index_updated_with_progress(
    path: &Path,
    force_reindex: bool,
    need_embeddings: bool,
    progress_callback: Option<ck_index::ProgressCallback>,
    detailed_progress_callback: Option<ck_index::DetailedProgressCallback>,
    file_options: &ck_core::FileCollectionOptions,
    model_override: Option<&str>,
) -> Result<Option<ck_index::UpdateStats>> {
    // Find index root for .ck directory location
    let index_root_buf = find_nearest_index_root(path).unwrap_or_else(|| {
        if path.is_file() {
            path.parent().unwrap_or(path).to_path_buf()
        } else {
            path.to_path_buf()
        }
    });
    let index_root = &index_root_buf;

    // Pass the original path to indexing function so it can index just that file/directory
    // The indexing function will use collect_files() which now handles individual files correctly
    if force_reindex {
        let stats = ck_index::smart_update_index_with_detailed_progress(
            index_root,
            true,
            progress_callback,
            detailed_progress_callback,
            need_embeddings,
            file_options,
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
        return Ok(Some(stats));
    }

    // For incremental updates with individual files, we need special handling
    // to ensure only the specific file is indexed, not the entire directory
    if path.is_file() {
        // Index just this one file
        use ck_index::index_file;
        index_file(path, need_embeddings).await?;
        Ok(None)
    } else {
        // For directories, use the standard smart update
        let stats = ck_index::smart_update_index_with_detailed_progress(
            index_root,
            false,
            progress_callback,
            detailed_progress_callback,
            need_embeddings,
            file_options,
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
        Ok(Some(stats))
    }
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
        let include_markdown = lang == ck_core::Language::Markdown;
        let sections: Vec<(usize, usize, String)> = chunks
            .into_iter()
            .filter(|chunk| {
                if include_markdown {
                    matches!(
                        chunk.chunk_type,
                        ck_chunk::ChunkType::Module | ck_chunk::ChunkType::Text
                    )
                } else {
                    matches!(
                        chunk.chunk_type,
                        ck_chunk::ChunkType::Function
                            | ck_chunk::ChunkType::Class
                            | ck_chunk::ChunkType::Method
                    )
                }
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

    fn make_result(file: &str, line_start: usize, line_end: usize, preview: &str) -> SearchResult {
        SearchResult {
            file: PathBuf::from(file),
            span: Span {
                byte_start: 0,
                byte_end: 0,
                line_start,
                line_end,
            },
            score: 0.0,
            preview: preview.to_string(),
            lang: None,
            symbol: None,
            chunk_hash: None,
            index_epoch: None,
        }
    }

    #[test]
    fn test_hybrid_query_terms_filters_stopwords_and_dedupes() {
        let terms =
            hybrid_query_terms("How does the RRF rank fusion merge rank results from regex?");
        assert_eq!(
            terms,
            vec!["rrf", "rank", "fusion", "merge", "results", "regex"]
        );

        // Single short/stopword-only queries produce nothing
        assert!(hybrid_query_terms("how does the it").is_empty());
    }

    #[test]
    fn test_rrf_fuse_merges_keyword_hit_into_containing_semantic_chunk() {
        // Semantic chunk spans lines 10-50; keyword hit lands on line 20
        let semantic = vec![
            make_result("src/a.rs", 10, 50, "fn fuse() { /* rrf */ }"),
            make_result("src/b.rs", 1, 5, "unrelated chunk"),
        ];
        let keyword = vec![make_result("src/a.rs", 20, 20, "let rrf_score = ranks")];

        let fused = rrf_fuse(&keyword, &semantic, 1.0);

        // The keyword hit fused into the chunk: 3 inputs, 2 outputs
        assert_eq!(fused.len(), 2);
        let chunk = fused
            .iter()
            .find(|r| r.file == PathBuf::from("src/a.rs"))
            .unwrap();
        // Chunk-level span retained, score = both lists at rank 1
        assert_eq!((chunk.span.line_start, chunk.span.line_end), (10, 50));
        let expected = 1.0 / 61.0 + 1.0 / 61.0;
        assert!((chunk.score - expected).abs() < 1e-6);

        // The fused result must outrank the semantic-only one
        let other = fused
            .iter()
            .find(|r| r.file == PathBuf::from("src/b.rs"))
            .unwrap();
        assert!(chunk.score > other.score);
    }

    #[test]
    fn test_rrf_fuse_keyword_only_hit_keeps_own_identity() {
        let semantic = vec![make_result("src/a.rs", 10, 50, "chunk")];
        let keyword = vec![make_result("src/z.rs", 7, 7, "standalone line")];

        let fused = rrf_fuse(&keyword, &semantic, 1.0);
        assert_eq!(fused.len(), 2);
        let standalone = fused
            .iter()
            .find(|r| r.file == PathBuf::from("src/z.rs"))
            .unwrap();
        assert!((standalone.score - 1.0 / 61.0).abs() < 1e-6);
    }

    #[test]
    fn test_rrf_fuse_counts_each_list_once_per_key() {
        // Two keyword hits inside the same semantic chunk: only the best
        // keyword rank contributes, not both.
        let semantic = vec![make_result("src/a.rs", 10, 50, "chunk")];
        let keyword = vec![
            make_result("src/a.rs", 12, 12, "first hit"),
            make_result("src/a.rs", 40, 40, "second hit"),
        ];

        let fused = rrf_fuse(&keyword, &semantic, 1.0);
        assert_eq!(fused.len(), 1);
        let expected = 1.0 / 61.0 + 1.0 / 61.0; // sem rank 1 + best keyword rank 1
        assert!((fused[0].score - expected).abs() < 1e-6);
    }

    #[test]
    fn test_lexical_corpus_fingerprint_tracks_changes() {
        let temp_dir = TempDir::new().unwrap();
        let a = temp_dir.path().join("a.txt");
        let b = temp_dir.path().join("b.txt");
        fs::write(&a, "one").unwrap();
        fs::write(&b, "two").unwrap();

        let original = lexical_corpus_fingerprint(&[a.clone(), b.clone()]);

        // Order-insensitive
        assert_eq!(
            original,
            lexical_corpus_fingerprint(&[b.clone(), a.clone()])
        );

        // Content change (different size) changes the fingerprint
        fs::write(&a, "one but longer").unwrap();
        assert_ne!(
            original,
            lexical_corpus_fingerprint(&[a.clone(), b.clone()])
        );

        // Removing a file changes the fingerprint
        let shrunk = lexical_corpus_fingerprint(std::slice::from_ref(&a));
        assert_ne!(shrunk, lexical_corpus_fingerprint(&[a, b]));
    }

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
            fs::write(temp_dir.path().join(format!("file{i}.txt")), "test content").unwrap();
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

    // Default model config is fastembed; without that feature ck-embed
    // falls back to DummyEmbedder (zero vectors), so semantic search
    // returns nothing and these tests have nothing to assert against.
    #[cfg(feature = "fastembed")]
    #[tokio::test]
    async fn test_subdirectory_search_uses_parent_ckignore() {
        // Regression test for issue where searching in subdirectory doesn't use parent .ckignore
        // Bug: When searching ~/parent/subdir/, .ckignore is loaded from subdir (doesn't exist)
        // instead of from parent (where index and .ckignore live)

        let temp_dir = TempDir::new().unwrap();
        let parent = temp_dir.path();
        let subdir = parent.join("subproject");
        fs::create_dir(&subdir).unwrap();

        // Create .ckignore at parent level excluding *.tmp files
        fs::write(parent.join(".ckignore"), "*.tmp\n").unwrap();

        // Create test files in parent directory
        fs::write(parent.join("parent.txt"), "searchable content in parent").unwrap();
        fs::write(parent.join("ignored.tmp"), "this should not be indexed").unwrap();

        // Create test files in subdirectory
        fs::write(subdir.join("nested.txt"), "searchable content in subdir").unwrap();
        fs::write(
            subdir.join("also_ignored.tmp"),
            "this should not be indexed either",
        )
        .unwrap();

        // First, search from parent to create the index
        let parent_options = SearchOptions {
            mode: SearchMode::Semantic,
            query: "searchable".to_string(),
            path: parent.to_path_buf(),
            top_k: Some(10),
            threshold: Some(0.1),
            ..Default::default()
        };

        let _ = search(&parent_options).await;

        // Give indexing a moment to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Now search from SUBDIRECTORY - this is where the bug occurs
        // The engine should find parent .ck index and use parent .ckignore
        // But currently it loads .ckignore from subdir (doesn't exist)
        let subdir_options = SearchOptions {
            mode: SearchMode::Semantic,
            query: "content".to_string(),
            path: subdir.clone(),
            top_k: Some(10),
            threshold: Some(0.1),
            ..Default::default()
        };

        let results = search(&subdir_options).await.unwrap();

        // ASSERTION 1: .tmp files should be excluded (currently FAILS due to bug)
        let tmp_files: Vec<_> = results
            .iter()
            .filter(|r| r.file.to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(
            tmp_files.is_empty(),
            "Bug: .tmp files were indexed despite parent .ckignore. Found {} .tmp files: {:?}",
            tmp_files.len(),
            tmp_files.iter().map(|r| &r.file).collect::<Vec<_>>()
        );

        // ASSERTION 2: Should find .txt files in subdirectory
        let txt_in_subdir = results.iter().any(|r| r.file.ends_with("nested.txt"));
        assert!(txt_in_subdir, "Should find nested.txt in subdirectory");

        // ASSERTION 3: No .ck directory should be created in subdirectory
        assert!(
            !subdir.join(".ck").exists(),
            "Should not create .ck directory in subdirectory"
        );
    }

    // Default model config is fastembed; without that feature ck-embed
    // falls back to DummyEmbedder (zero vectors), so semantic search
    // returns nothing and these tests have nothing to assert against.
    #[cfg(feature = "fastembed")]
    #[tokio::test]
    async fn test_multiple_ckignore_files_merge_correctly() {
        // Test that multiple .ckignore files in the hierarchy are all applied
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let parent = temp_dir.path();
        let subdir = parent.join("subdir");
        let deeper = subdir.join("deeper");
        fs::create_dir(&subdir).unwrap();
        fs::create_dir(&deeper).unwrap();

        // Create hierarchical .ckignore files
        fs::write(parent.join(".ckignore"), "*.log\n").unwrap();
        fs::write(subdir.join(".ckignore"), "*.tmp\n").unwrap();
        fs::write(deeper.join(".ckignore"), "*.cache\n").unwrap();

        // Create test files at each level
        fs::write(parent.join("root.txt"), "searchable").unwrap();
        fs::write(parent.join("root.log"), "should be ignored").unwrap();

        fs::write(subdir.join("mid.txt"), "searchable").unwrap();
        fs::write(subdir.join("mid.log"), "should be ignored by parent").unwrap();
        fs::write(subdir.join("mid.tmp"), "should be ignored by local").unwrap();

        fs::write(deeper.join("deep.txt"), "searchable").unwrap();
        fs::write(deeper.join("deep.log"), "should be ignored by grandparent").unwrap();
        fs::write(deeper.join("deep.tmp"), "should be ignored by parent").unwrap();
        fs::write(deeper.join("deep.cache"), "should be ignored by local").unwrap();

        // Index from parent
        let parent_options = SearchOptions {
            mode: SearchMode::Semantic,
            query: "searchable".to_string(),
            path: parent.to_path_buf(),
            top_k: Some(20),
            threshold: Some(0.1),
            ..Default::default()
        };

        let _ = search(&parent_options).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Search from deeper directory - should respect ALL three .ckignore files
        let deeper_options = SearchOptions {
            mode: SearchMode::Semantic,
            query: "ignored".to_string(),
            path: deeper.clone(),
            top_k: Some(20),
            threshold: Some(0.1),
            ..Default::default()
        };

        let results = search(&deeper_options).await.unwrap();

        // All ignored files should be excluded
        let has_log = results
            .iter()
            .any(|r| r.file.to_string_lossy().ends_with(".log"));
        let has_tmp = results
            .iter()
            .any(|r| r.file.to_string_lossy().ends_with(".tmp"));
        let has_cache = results
            .iter()
            .any(|r| r.file.to_string_lossy().ends_with(".cache"));

        assert!(
            !has_log,
            "*.log files should be excluded by parent .ckignore"
        );
        assert!(
            !has_tmp,
            "*.tmp files should be excluded by subdir .ckignore"
        );
        assert!(
            !has_cache,
            "*.cache files should be excluded by deeper .ckignore"
        );

        // Should still find .txt files
        let has_txt = results
            .iter()
            .any(|r| r.file.to_string_lossy().ends_with(".txt"));
        assert!(has_txt, "Should find .txt files (not ignored)");
    }

    // Default model config is fastembed; without that feature ck-embed
    // falls back to DummyEmbedder (zero vectors) and the assertions can't
    // distinguish a scoped match from no match at all.
    #[cfg(feature = "fastembed")]
    #[tokio::test]
    async fn test_scoped_search_does_not_lose_results_to_global_top_k() {
        // Regression test for the bug where scoped semantic search applied
        // top_k BEFORE the path filter, so a small top_k against a whole-
        // codebase index could return zero matches when the global top
        // results all lived outside the requested scope.
        //
        // Reproduction:
        //   - Index a parent dir that contains many files about TOPIC_A
        //     (so they dominate the global top_k for that query)
        //   - Search inside a sibling subdir that contains a file about
        //     TOPIC_A, with top_k smaller than the TOPIC_A file count
        //   - Before the fix: zero results inside subdir
        //   - After the fix:  the in-scope file is returned
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let parent = temp_dir.path();
        let noisy = parent.join("noisy");
        let scoped = parent.join("scoped");
        fs::create_dir(&noisy).unwrap();
        fs::create_dir(&scoped).unwrap();

        // 8 files in noisy/ that all match the query "database connection".
        // top_k=3 will be entirely consumed by these globally.
        for i in 0..8 {
            fs::write(
                noisy.join(format!("noise_{i}.txt")),
                format!(
                    "function open_database_connection_{i}() {{\n    \
                     // establish a database connection to postgres\n    \
                     // handle database connection errors gracefully\n}}\n"
                ),
            )
            .unwrap();
        }

        // One in-scope file that also matches the query
        fs::write(
            scoped.join("target.txt"),
            "function connect() {\n    \
             // open a database connection to the primary store\n    \
             // database connection pool config goes here\n}\n",
        )
        .unwrap();

        // Index from parent so .ck lives at parent root and covers both subdirs.
        let index_options = SearchOptions {
            mode: SearchMode::Semantic,
            query: "database connection".to_string(),
            path: parent.to_path_buf(),
            top_k: Some(20),
            threshold: Some(0.0),
            ..Default::default()
        };
        let _ = search(&index_options).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Now search scoped to `scoped/` with a small top_k. The bug:
        // the 3 global top results all live in `noisy/`, so the path
        // filter rejects all of them and we get [].
        let scoped_options = SearchOptions {
            mode: SearchMode::Semantic,
            query: "database connection".to_string(),
            path: scoped.clone(),
            top_k: Some(3),
            threshold: Some(0.0),
            ..Default::default()
        };

        let results = search(&scoped_options).await.unwrap();

        assert!(
            !results.is_empty(),
            "Scoped search returned zero results — top_k was applied \
             before the path filter (the bug this test guards against)."
        );
        let all_in_scope = results.iter().all(|r| {
            r.file.starts_with(&scoped)
                || r.file.canonicalize().ok() == scoped.join("target.txt").canonicalize().ok()
        });
        assert!(
            all_in_scope,
            "Some results leaked out of the requested scope: {:?}",
            results.iter().map(|r| &r.file).collect::<Vec<_>>()
        );
    }
}
