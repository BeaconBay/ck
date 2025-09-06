use anyhow::Result;
use ck_core::{CkError, SearchMode, SearchOptions, SearchResult, Span};
use globset::{Glob, GlobSet, GlobSetBuilder};
use regex::{Regex, RegexBuilder};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use rayon::prelude::*;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Schema, STORED, TEXT, Value};
use tantivy::{doc, Index, ReloadPolicy, TantivyDocument};
use ck_ann::AnnIndex;
use std::path::PathBuf as StdPathBuf;

mod semantic_v3;
pub use semantic_v3::{semantic_search_v3, semantic_search_v3_with_progress};

pub type SearchProgressCallback = Box<dyn Fn(&str) + Send + Sync>;

/// Extract content from a file using a span
fn extract_content_from_span(file_path: &Path, span: &ck_core::Span) -> Result<String> {
    let content = fs::read_to_string(file_path)?;
    let lines: Vec<&str> = content.lines().collect();
    
    if span.line_start == 0 || span.line_start > lines.len() {
        return Ok(String::new());
    }
    
    let start_idx = span.line_start - 1; // Convert to 0-based
    let end_idx = (span.line_end - 1).min(lines.len().saturating_sub(1));
    
    if start_idx <= end_idx {
        Ok(lines[start_idx..=end_idx].join("\n"))
    } else {
        Ok(lines[start_idx].to_string())
    }
}

fn find_nearest_index_root(path: &Path) -> Option<StdPathBuf> {
    let mut current = if path.is_file() { path.parent().unwrap_or(path) } else { path };
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

pub async fn search(options: &SearchOptions) -> Result<Vec<SearchResult>> {
    search_with_progress(options, None).await
}

pub async fn search_with_progress(options: &SearchOptions, progress_callback: Option<SearchProgressCallback>) -> Result<Vec<SearchResult>> {
    // Auto-update index if needed (unless it's regex-only mode)
    if !matches!(options.mode, SearchMode::Regex) {
        let need_embeddings = matches!(options.mode, SearchMode::Semantic | SearchMode::Hybrid);
        ensure_index_updated(&options.path, options.reindex, need_embeddings).await?;
    }
    
    match options.mode {
        SearchMode::Regex => regex_search(options),
        SearchMode::Lexical => lexical_search(options).await,
        SearchMode::Semantic => {
            // Use v3 semantic search (reads pre-computed embeddings from sidecars using spans)
            semantic_search_v3_with_progress(options, progress_callback).await
        },
        SearchMode::Hybrid => hybrid_search_with_progress(options, progress_callback).await,
    }
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
        .map_err(|e| CkError::Regex(e))?;
    
    let files = collect_files(&options.path, options.recursive, &options.exclude_patterns)?;
    
    let results: Vec<Vec<SearchResult>> = files
        .par_iter()
        .filter_map(|file_path| {
            match search_file(&regex, file_path, options) {
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

fn search_file(regex: &Regex, file_path: &Path, options: &SearchOptions) -> Result<Vec<SearchResult>> {
    let content = fs::read_to_string(file_path)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut results = Vec::new();
    
    // If full_section is enabled, try to parse the file and find code sections
    let code_sections = if options.full_section {
        extract_code_sections(file_path, &content)
    } else {
        None
    };
    
    for (line_idx, line) in lines.iter().enumerate() {
        let line_number = line_idx + 1;
        
        if regex.is_match(line) {
            let preview = if options.full_section {
                // Try to find the containing code section
                if let Some(ref sections) = code_sections {
                    if let Some(section) = find_containing_section(sections, line_idx) {
                        section.clone()
                    } else {
                        // Fall back to context lines if no section found
                        get_context_preview(&lines, line_idx, options)
                    }
                } else {
                    get_context_preview(&lines, line_idx, options)
                }
            } else {
                get_context_preview(&lines, line_idx, options)
            };
            
            results.push(SearchResult {
                file: file_path.to_path_buf(),
                span: Span {
                    byte_start: 0,
                    byte_end: line.len(),
                    line_start: line_number,
                    line_end: line_number,
                },
                score: 1.0,
                preview,
                lang: detect_language(file_path),
                symbol: None,
            });
        }
    }
    
    Ok(results)
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
        
        raw_results.push((_score, SearchResult {
            file: file_path,
            span: Span {
                byte_start: 0,
                byte_end: content_text.len(),
                line_start: 1,
                line_end: content_text.lines().count(),
            },
            score: _score,
            preview,
            lang: detect_language(&PathBuf::from(path_text)),
            symbol: None,
        }));
    }
    
    // Normalize scores to 0-1 range and apply threshold
    let mut results = Vec::new();
    if !raw_results.is_empty() {
        let max_score = raw_results.iter().map(|(score, _)| *score).fold(0.0f32, f32::max);
        if max_score > 0.0 {
            for (raw_score, mut result) in raw_results {
                let normalized_score = raw_score / max_score;
                
                // Apply threshold filtering with normalized score
                if let Some(threshold) = options.threshold {
                    if normalized_score < threshold {
                        continue;
                    }
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
    
    let mut index_writer = index.writer(50_000_000)
        .map_err(|e| CkError::Index(format!("Failed to create index writer: {}", e)))?;
    
    let files = collect_files(&index_root, true, &options.exclude_patterns)?;
    
    for file_path in &files {
        if let Ok(content) = fs::read_to_string(file_path) {
            let doc = doc!(
                content_field => content,
                path_field => file_path.display().to_string()
            );
            index_writer.add_document(doc)?;
        }
    }
    
    index_writer.commit()
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
        
        raw_results.push((_score, SearchResult {
            file: file_path,
            span: Span {
                byte_start: 0,
                byte_end: content_text.len(),
                line_start: 1,
                line_end: content_text.lines().count(),
            },
            score: _score,
            preview,
            lang: detect_language(&PathBuf::from(path_text)),
            symbol: None,
        }));
    }
    
    // Normalize scores to 0-1 range and apply threshold
    let mut results = Vec::new();
    if !raw_results.is_empty() {
        let max_score = raw_results.iter().map(|(score, _)| *score).fold(0.0f32, f32::max);
        if max_score > 0.0 {
            for (raw_score, mut result) in raw_results {
                let normalized_score = raw_score / max_score;
                
                // Apply threshold filtering with normalized score
                if let Some(threshold) = options.threshold {
                    if normalized_score < threshold {
                        continue;
                    }
                }
                
                result.score = normalized_score;
                results.push(result);
            }
        }
    }
    
    Ok(results)
}

#[allow(dead_code)]
async fn semantic_search(options: &SearchOptions) -> Result<Vec<SearchResult>> {
    semantic_search_with_progress(options, None).await
}

async fn semantic_search_with_progress(options: &SearchOptions, progress_callback: Option<SearchProgressCallback>) -> Result<Vec<SearchResult>> {
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
    
    let ann_index_path = index_dir.join("ann_index.bin");
    let embeddings_path = index_dir.join("embeddings.json");
    
    if !ann_index_path.exists() || !embeddings_path.exists() {
        return build_semantic_index_with_progress(options, progress_callback).await;
    }
    
    // Load the ANN index
    let ann_index = ck_ann::SimpleIndex::load(&ann_index_path)?;
    
    // Load file metadata
    let embeddings_data = fs::read_to_string(&embeddings_path)?;
    let file_embeddings: Vec<(PathBuf, String)> = serde_json::from_str(&embeddings_data)?;
    
    // Create embedder and embed the query
    if let Some(ref callback) = progress_callback {
        callback("Loading embedding model...");
    }
    
    let mut embedder = if let Some(ref callback) = progress_callback {
        let _cb = callback.as_ref();
        let model_cb = Box::new(|msg: &str| {
            // Note: We can't directly use the callback here due to lifetime issues
            // For now, we'll just use eprintln! until we can restructure this better
            eprintln!("Model: {}", msg);
        }) as ck_embed::ModelDownloadCallback;
        ck_embed::create_embedder_with_progress(Some("BAAI/bge-small-en-v1.5"), Some(model_cb))?
    } else {
        ck_embed::create_embedder(Some("BAAI/bge-small-en-v1.5"))?
    };
    let query_embeddings = embedder.embed(&[options.query.clone()])?;
    
    if query_embeddings.is_empty() {
        return Ok(Vec::new());
    }
    
    let query_embedding = &query_embeddings[0];
    
    // Search using ANN
    let top_k = options.top_k.unwrap_or(10);
    let similar_docs = ann_index.search(query_embedding, top_k);
    
    let mut results = Vec::new();
    
    // Check if we're searching a specific file vs. a directory
    let filter_by_file = options.path.is_file();
    let target_file = if filter_by_file {
        Some(options.path.canonicalize().unwrap_or_else(|_| options.path.clone()))
    } else {
        None
    };
    
    for (doc_id, similarity) in similar_docs {
        // Apply threshold filtering
        if let Some(threshold) = options.threshold {
            if similarity < threshold {
                continue;
            }
        }
        
        if let Some((file_path, content)) = file_embeddings.get(doc_id as usize) {
            // Filter by target file if specified
            if let Some(target) = &target_file {
                let canonical_result = file_path.canonicalize().unwrap_or_else(|_| file_path.clone());
                if canonical_result != *target {
                    continue; // Skip this result if it doesn't match the target file
                }
            }
            
            // If full_section is enabled and this is a code section, return the full content
            let preview = if options.full_section {
                content.clone()
            } else {
                content.lines().take(3).collect::<Vec<_>>().join("\n")
            };
            
            results.push(SearchResult {
                file: file_path.clone(),
                span: Span {
                    byte_start: 0,
                    byte_end: content.len(),
                    line_start: 1,
                    line_end: content.lines().count(),
                },
                score: similarity,
                preview,
                lang: detect_language(file_path),
                symbol: None,
            });
        }
    }
    
    Ok(results)
}

#[allow(dead_code)]
async fn build_semantic_index(options: &SearchOptions) -> Result<Vec<SearchResult>> {
    build_semantic_index_with_progress(options, None).await
}

async fn build_semantic_index_with_progress(options: &SearchOptions, progress_callback: Option<SearchProgressCallback>) -> Result<Vec<SearchResult>> {
    // Handle both files and directories by finding the appropriate directory for indexing
    let index_root = if options.path.is_file() {
        options.path.parent().unwrap_or(&options.path)
    } else {
        &options.path
    };
    
    let index_dir = index_root.join(".ck");
    let ann_index_path = index_dir.join("ann_index.bin");
    let embeddings_path = index_dir.join("embeddings.json");
    
    fs::create_dir_all(&index_dir)?;
    
    if let Some(ref callback) = progress_callback {
        callback("Building semantic index (no index found)...");
    }
    
    // Always print this important message, even in quiet mode for indexing operations
    eprintln!("Building semantic index (no existing index found)...");
    
    // Collect files and their content
    let files = collect_files(&index_root, true, &options.exclude_patterns)?;
    
    if let Some(ref callback) = progress_callback {
        callback(&format!("Found {} files to index", files.len()));
    }
    eprintln!("Found {} files to embed and index", files.len());
    
    let mut file_embeddings = Vec::new();
    let mut embeddings = Vec::new();
    
    // Create embedder with progress callback
    if let Some(ref callback) = progress_callback {
        callback("Loading embedding model...");
    }
    
    let model_callback = if progress_callback.is_some() {
        Some(Box::new(|msg: &str| {
            eprintln!("Model: {}", msg);
        }) as ck_embed::ModelDownloadCallback)
    } else {
        None
    };
    
    let mut embedder = ck_embed::create_embedder_with_progress(Some("BAAI/bge-small-en-v1.5"), model_callback)?;
    
    if let Some(ref callback) = progress_callback {
        callback("Generating embeddings for code chunks...");
    }
    
    for (file_idx, file_path) in files.iter().enumerate() {
        if let Ok(content) = fs::read_to_string(file_path) {
            if let Some(ref callback) = progress_callback {
                let file_name = file_path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| file_path.to_string_lossy().to_string());
                callback(&format!("Processing {}/{}: {}", file_idx + 1, files.len(), file_name));
            }
            
            // Chunk the content for better embeddings
            let chunks = ck_chunk::chunk_text(&content, detect_language(file_path).as_deref())?;
            
            for chunk in chunks {
                let chunk_embeddings = embedder.embed(&[chunk.text.clone()])?;
                if !chunk_embeddings.is_empty() {
                    embeddings.push(chunk_embeddings[0].clone());
                    file_embeddings.push((file_path.clone(), chunk.text));
                }
            }
        }
    }
    
    if let Some(ref callback) = progress_callback {
        callback(&format!("Built {} embeddings, creating search index...", embeddings.len()));
    }
    eprintln!("Generated {} embeddings, building search index...", embeddings.len());
    
    // Build ANN index
    let index = ck_ann::SimpleIndex::build(&embeddings)?;
    index.save(&ann_index_path)?;
    
    // Save file embeddings metadata
    let embeddings_json = serde_json::to_string(&file_embeddings)?;
    fs::write(&embeddings_path, embeddings_json)?;
    
    if let Some(ref callback) = progress_callback {
        callback("Semantic index built successfully, running search...");
    }
    eprintln!("Semantic index built successfully!");
    
    // After building, search again - inline to avoid recursion
    let ann_index = ck_ann::SimpleIndex::load(&ann_index_path)?;
    
    // Load file metadata
    let embeddings_data = fs::read_to_string(&embeddings_path)?;
    let file_embeddings: Vec<(PathBuf, String)> = serde_json::from_str(&embeddings_data)?;
    
    // Create embedder and embed the query
    let mut embedder = ck_embed::create_embedder(Some("BAAI/bge-small-en-v1.5"))?;
    let query_embeddings = embedder.embed(&[options.query.clone()])?;
    
    if query_embeddings.is_empty() {
        return Ok(Vec::new());
    }
    
    let query_embedding = &query_embeddings[0];
    
    // Search using ANN
    let top_k = options.top_k.unwrap_or(10);
    let similar_docs = ann_index.search(query_embedding, top_k);
    
    let mut results = Vec::new();
    
    // Check if we're searching a specific file vs. a directory
    let filter_by_file = options.path.is_file();
    let target_file = if filter_by_file {
        Some(options.path.canonicalize().unwrap_or_else(|_| options.path.clone()))
    } else {
        None
    };
    
    for (doc_id, similarity) in similar_docs {
        // Apply threshold filtering
        if let Some(threshold) = options.threshold {
            if similarity < threshold {
                continue;
            }
        }
        
        if let Some((file_path, content)) = file_embeddings.get(doc_id as usize) {
            // Filter by target file if specified
            if let Some(target) = &target_file {
                let canonical_result = file_path.canonicalize().unwrap_or_else(|_| file_path.clone());
                if canonical_result != *target {
                    continue; // Skip this result if it doesn't match the target file
                }
            }
            
            // If full_section is enabled and this is a code section, return the full content
            let preview = if options.full_section {
                content.clone()
            } else {
                content.lines().take(3).collect::<Vec<_>>().join("\n")
            };
            
            results.push(SearchResult {
                file: file_path.clone(),
                span: Span {
                    byte_start: 0,
                    byte_end: content.len(),
                    line_start: 1,
                    line_end: content.lines().count(),
                },
                score: similarity,
                preview,
                lang: detect_language(file_path),
                symbol: None,
            });
        }
    }
    
    Ok(results)
}

#[allow(dead_code)]
async fn hybrid_search(options: &SearchOptions) -> Result<Vec<SearchResult>> {
    hybrid_search_with_progress(options, None).await
}

async fn hybrid_search_with_progress(options: &SearchOptions, progress_callback: Option<SearchProgressCallback>) -> Result<Vec<SearchResult>> {
    if let Some(ref callback) = progress_callback {
        callback("Running regex search...");
    }
    let regex_results = regex_search(options)?;
    
    if let Some(ref callback) = progress_callback {
        callback("Running semantic search...");
    }
    let semantic_results = semantic_search_with_progress(options, progress_callback).await?;
    
    let mut combined = HashMap::new();
    
    for (rank, result) in regex_results.iter().enumerate() {
        let key = format!("{}:{}", result.file.display(), result.span.line_start);
        combined.entry(key).or_insert(Vec::new()).push((rank + 1, result.clone()));
    }
    
    for (rank, result) in semantic_results.iter().enumerate() {
        let key = format!("{}:{}", result.file.display(), result.span.line_start);
        combined.entry(key).or_insert(Vec::new()).push((rank + 1, result.clone()));
    }
    
    // Calculate RRF scores according to original paper: RRFscore(d) = Σ(r∈R) 1/(k + r(d))
    let mut rrf_results: Vec<SearchResult> = combined
        .into_iter()
        .map(|(_, ranks)| {
            let mut result = ranks[0].1.clone();
            let rrf_score = ranks.iter().map(|(rank, _)| 1.0 / (60.0 + *rank as f32)).sum();
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
        if let std::path::Component::Normal(name) = component {
            if globset.is_match(name) {
                return true;
            }
        }
    }
    false
}

fn collect_files(path: &Path, recursive: bool, exclude_patterns: &[String]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let globset = build_globset(exclude_patterns);
    
    if path.is_file() {
        // Always add single files, even if they're excluded (user explicitly requested)
        files.push(path.to_path_buf());
    } else if recursive {
        for entry in WalkDir::new(path)
            .into_iter()
            .filter_entry(|e| {
                // Skip excluded directories entirely for efficiency
                let name = e.file_name();
                !globset.is_match(e.path()) && !globset.is_match(name)
            }) {
            match entry {
                Ok(entry) => {
                    if entry.file_type().is_file() && !should_exclude_path(entry.path(), exclude_patterns) {
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

fn detect_language(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| match ext {
            "rs" => "rust",
            "py" => "python",
            "js" => "javascript",
            "ts" => "typescript",
            "go" => "go",
            "java" => "java",
            "c" => "c",
            "cpp" | "cc" | "cxx" => "cpp",
            "h" | "hpp" => "cpp",
            "cs" => "csharp",
            "rb" => "ruby",
            "php" => "php",
            "swift" => "swift",
            "kt" => "kotlin",
            _ => ext,
        })
        .map(String::from)
}

async fn ensure_index_updated(path: &Path, force_reindex: bool, need_embeddings: bool) -> Result<()> {
    
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
        let stats = ck_index::smart_update_index_with_progress(index_root, false, None, need_embeddings).await?;
        if stats.files_indexed > 0 || stats.orphaned_files_removed > 0 {
            tracing::info!("Index updated: {} files indexed, {} orphaned files removed", 
                          stats.files_indexed, stats.orphaned_files_removed);
        }
        return Ok(());
    }
    
    // Always use smart_update_index for incremental updates (handles both new and existing indexes)
    let stats = ck_index::smart_update_index_with_progress(index_root, false, None, need_embeddings).await?;
    if stats.files_indexed > 0 || stats.orphaned_files_removed > 0 {
        tracing::info!("Index updated: {} files indexed, {} orphaned files removed", 
                      stats.files_indexed, stats.orphaned_files_removed);
    }
    
    Ok(())
}

fn get_context_preview(lines: &[&str], line_idx: usize, options: &SearchOptions) -> String {
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
    // Detect language for tree-sitter parsing
    let lang = match file_path.extension().and_then(|s| s.to_str()) {
        Some("py") => Some("python"),
        Some("js") => Some("javascript"),
        Some("ts") | Some("tsx") => Some("typescript"),
        _ => return None,
    };
    
    // Parse the file with tree-sitter and extract function/class sections
    if let Ok(chunks) = ck_chunk::chunk_text(content, lang) {
        let sections: Vec<(usize, usize, String)> = chunks
            .into_iter()
            .filter(|chunk| matches!(
                chunk.chunk_type,
                ck_chunk::ChunkType::Function | 
                ck_chunk::ChunkType::Class | 
                ck_chunk::ChunkType::Method
            ))
            .map(|chunk| {
                (
                    chunk.span.line_start - 1,  // Convert to 0-based index
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

fn find_containing_section(sections: &[(usize, usize, String)], line_idx: usize) -> Option<&String> {
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
    fn test_detect_language() {
        assert_eq!(detect_language(&PathBuf::from("test.rs")), Some("rust".to_string()));
        assert_eq!(detect_language(&PathBuf::from("test.py")), Some("python".to_string()));
        assert_eq!(detect_language(&PathBuf::from("test.js")), Some("javascript".to_string()));
        assert_eq!(detect_language(&PathBuf::from("test.unknown")), Some("unknown".to_string()));
        assert_eq!(detect_language(&PathBuf::from("noext")), None);
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
        let rust_matches: Vec<_> = results.iter()
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
        fs::write(temp_dir.path().join("word_test.txt"), "rust rusty rustacean").unwrap();

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
            fs::write(temp_dir.path().join(format!("file{}.txt", i)), "test content").unwrap();
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
    fn test_search_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "line 1: hello\nline 2: world\nline 3: rust programming").unwrap();

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
}