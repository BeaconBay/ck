use anyhow::Result;
use ck_core::{FileMetadata, Span, compute_file_hash, get_sidecar_path};
use ignore::{WalkBuilder, overrides::OverrideBuilder};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;
use walkdir::WalkDir;

pub type ProgressCallback = Box<dyn Fn(&str) + Send + Sync>;

// Global interrupt flag
static INTERRUPTED: AtomicBool = AtomicBool::new(false);
static HANDLER_INIT: Once = Once::new();

/// Build override patterns for excluding files during directory traversal
fn build_overrides(
    base_path: &Path,
    exclude_patterns: &[String],
) -> Result<ignore::overrides::Override> {
    let mut builder = OverrideBuilder::new(base_path);

    for pattern in exclude_patterns {
        // Convert to exclude pattern (add ! prefix if not present)
        let exclude_pattern = if pattern.starts_with('!') {
            pattern.clone()
        } else {
            format!("!{}", pattern)
        };
        builder.add(&exclude_pattern)?;
    }

    Ok(builder.build()?)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    pub metadata: FileMetadata,
    pub chunks: Vec<ChunkEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkEntry {
    pub span: Span,
    pub embedding: Option<Vec<f32>>,
    pub chunk_type: Option<String>, // "function", "class", "method", or None for generic
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexManifest {
    pub version: String,
    pub created: u64,
    pub updated: u64,
    pub files: HashMap<PathBuf, FileMetadata>,
}

impl Default for IndexManifest {
    fn default() -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            version: "0.1.0".to_string(),
            created: now,
            updated: now,
            files: HashMap::new(),
        }
    }
}

pub fn collect_files(
    path: &Path,
    respect_gitignore: bool,
    exclude_patterns: &[String],
) -> Result<Vec<PathBuf>> {
    let index_dir = path.join(".ck");
    let overrides = build_overrides(path, exclude_patterns)?;

    if respect_gitignore {
        Ok(WalkBuilder::new(path)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .hidden(false)
            .overrides(overrides)
            .build()
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                let path = entry.path();
                entry.file_type().is_some_and(|ft| ft.is_file())
                    && is_text_file(path)
                    && !path.starts_with(&index_dir)
            })
            .map(|entry| entry.path().to_path_buf())
            .collect())
    } else {
        // Use WalkBuilder without gitignore support, but still apply overrides
        use ck_core::get_default_exclude_patterns;
        let default_patterns = get_default_exclude_patterns();

        // Combine default patterns with user exclude patterns
        let mut all_patterns = default_patterns;
        all_patterns.extend(exclude_patterns.iter().cloned());
        let combined_overrides = build_overrides(path, &all_patterns)?;

        Ok(WalkBuilder::new(path)
            .git_ignore(false)
            .hidden(false)
            .overrides(combined_overrides)
            .build()
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                let path = entry.path();
                entry.file_type().is_some_and(|ft| ft.is_file())
                    && is_text_file(path)
                    && !path.starts_with(&index_dir)
            })
            .map(|entry| entry.path().to_path_buf())
            .collect())
    }
}

fn collect_files_as_hashset(
    path: &Path,
    respect_gitignore: bool,
    exclude_patterns: &[String],
) -> Result<HashSet<PathBuf>> {
    Ok(collect_files(path, respect_gitignore, exclude_patterns)?
        .into_iter()
        .collect())
}

pub async fn index_directory(
    path: &Path,
    compute_embeddings: bool,
    respect_gitignore: bool,
    exclude_patterns: &[String],
) -> Result<()> {
    tracing::info!(
        "index_directory called with compute_embeddings={}",
        compute_embeddings
    );
    let index_dir = path.join(".ck");
    fs::create_dir_all(&index_dir)?;

    let manifest_path = index_dir.join("manifest.json");
    let mut manifest = load_or_create_manifest(&manifest_path)?;

    let files = collect_files(path, respect_gitignore, exclude_patterns)?;

    if compute_embeddings {
        // Sequential processing with streaming - write each file immediately
        tracing::info!("Creating embedder for {} files", files.len());
        let mut embedder = ck_embed::create_embedder(None)?;
        let mut _processed_count = 0;

        for file_path in files.iter() {
            match index_single_file(file_path, path, Some(&mut embedder)) {
                Ok(entry) => {
                    // Write sidecar immediately
                    let sidecar_path = get_sidecar_path(path, file_path);
                    save_index_entry(&sidecar_path, &entry)?;

                    // Update and save manifest immediately
                    manifest.files.insert(file_path.clone(), entry.metadata);
                    manifest.updated = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    save_manifest(&manifest_path, &manifest)?;
                    _processed_count += 1;
                }
                Err(e) => {
                    // Suppress warnings for binary files and UTF-8 errors in .git directories
                    let error_msg = e.to_string();
                    let is_binary_skip = error_msg.contains("Binary file, skipping");
                    let is_utf8_error = error_msg.contains("stream did not contain valid UTF-8");
                    let is_git_file = file_path.components().any(|c| c.as_os_str() == ".git");

                    if !(is_binary_skip || is_utf8_error && is_git_file) {
                        tracing::warn!("Failed to index {:?}: {}", file_path, e);
                    }
                }
            }
        }
    } else {
        // Parallel processing with streaming using producer-consumer pattern
        use std::sync::mpsc;
        use std::thread;

        let (tx, rx) = mpsc::channel();
        let files_clone = files.clone();
        let path_clone = path.to_path_buf();

        // Spawn worker thread for parallel processing
        let worker_handle = thread::spawn(move || {
            files_clone.par_iter().for_each(|file_path| {
                match index_single_file(file_path, &path_clone, None) {
                    Ok(entry) => {
                        if tx.send((file_path.clone(), entry)).is_err() {
                            // Receiver dropped, stop processing
                        }
                    }
                    Err(e) => {
                        // Suppress warnings for binary files and UTF-8 errors in .git directories
                        let error_msg = e.to_string();
                        let is_binary_skip = error_msg.contains("Binary file, skipping");
                        let is_utf8_error =
                            error_msg.contains("stream did not contain valid UTF-8");
                        let is_git_file = file_path.components().any(|c| c.as_os_str() == ".git");

                        if !(is_binary_skip || is_utf8_error && is_git_file) {
                            tracing::warn!("Failed to index {:?}: {}", file_path, e);
                        }
                    }
                }
            });
        });

        // Main thread: stream results as they arrive
        while let Ok((file_path, entry)) = rx.recv() {
            // Write sidecar immediately
            let sidecar_path = get_sidecar_path(path, &file_path);
            save_index_entry(&sidecar_path, &entry)?;

            // Update and save manifest immediately
            manifest.files.insert(file_path, entry.metadata);
            manifest.updated = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            save_manifest(&manifest_path, &manifest)?;
        }

        // Wait for worker to complete
        worker_handle
            .join()
            .map_err(|_| anyhow::anyhow!("Worker thread panicked"))?;
    }

    // Manifest is already updated after each file in streaming mode
    // Only save manifest if using parallel processing (non-embedding case)
    if !compute_embeddings {
        manifest.updated = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        save_manifest(&manifest_path, &manifest)?;
    }

    Ok(())
}

pub async fn index_file(file_path: &Path, compute_embeddings: bool) -> Result<()> {
    let repo_root = find_repo_root(file_path)?;
    let index_dir = repo_root.join(".ck");
    fs::create_dir_all(&index_dir)?;

    let manifest_path = index_dir.join("manifest.json");
    let mut manifest = load_or_create_manifest(&manifest_path)?;

    let entry = if compute_embeddings {
        let mut embedder = ck_embed::create_embedder(None)?;
        index_single_file(file_path, &repo_root, Some(&mut embedder))?
    } else {
        index_single_file(file_path, &repo_root, None)?
    };
    let sidecar_path = get_sidecar_path(&repo_root, file_path);

    save_index_entry(&sidecar_path, &entry)?;
    manifest
        .files
        .insert(file_path.to_path_buf(), entry.metadata);
    manifest.updated = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    save_manifest(&manifest_path, &manifest)?;

    Ok(())
}

pub async fn update_index(
    path: &Path,
    compute_embeddings: bool,
    respect_gitignore: bool,
    exclude_patterns: &[String],
) -> Result<()> {
    let index_dir = path.join(".ck");
    if !index_dir.exists() {
        return index_directory(
            path,
            compute_embeddings,
            respect_gitignore,
            exclude_patterns,
        )
        .await;
    }

    let manifest_path = index_dir.join("manifest.json");
    let mut manifest = load_or_create_manifest(&manifest_path)?;

    let files = collect_files(path, respect_gitignore, exclude_patterns)?;

    let updates: Vec<(PathBuf, IndexEntry)> = if compute_embeddings {
        // Sequential processing when computing embeddings
        let mut embedder = ck_embed::create_embedder(None)?;
        files
            .iter()
            .filter_map(|file_path| {
                let needs_update = match manifest.files.get(file_path) {
                    Some(metadata) => match compute_file_hash(file_path) {
                        Ok(hash) => hash != metadata.hash,
                        Err(_) => false,
                    },
                    None => true,
                };
                if needs_update {
                    match index_single_file(file_path, path, Some(&mut embedder)) {
                        Ok(entry) => Some((file_path.clone(), entry)),
                        Err(e) => {
                            // Suppress warnings for binary files and UTF-8 errors in .git directories
                            let error_msg = e.to_string();
                            let is_binary_skip = error_msg.contains("Binary file, skipping");
                            let is_utf8_error =
                                error_msg.contains("stream did not contain valid UTF-8");
                            let is_git_file =
                                file_path.components().any(|c| c.as_os_str() == ".git");

                            if !(is_binary_skip || is_utf8_error && is_git_file) {
                                tracing::warn!("Failed to index {:?}: {}", file_path, e);
                            }
                            None
                        }
                    }
                } else {
                    None
                }
            })
            .collect()
    } else {
        // Parallel processing when not computing embeddings
        files
            .par_iter()
            .filter_map(|file_path| {
                let needs_update = match manifest.files.get(file_path) {
                    Some(metadata) => match compute_file_hash(file_path) {
                        Ok(hash) => hash != metadata.hash,
                        Err(_) => false,
                    },
                    None => true,
                };

                if needs_update {
                    match index_single_file(file_path, path, None) {
                        Ok(entry) => Some((file_path.clone(), entry)),
                        Err(e) => {
                            // Suppress warnings for binary files and UTF-8 errors in .git directories
                            let error_msg = e.to_string();
                            let is_binary_skip = error_msg.contains("Binary file, skipping");
                            let is_utf8_error =
                                error_msg.contains("stream did not contain valid UTF-8");
                            let is_git_file =
                                file_path.components().any(|c| c.as_os_str() == ".git");

                            if !(is_binary_skip || is_utf8_error && is_git_file) {
                                tracing::warn!("Failed to index {:?}: {}", file_path, e);
                            }
                            None
                        }
                    }
                } else {
                    None
                }
            })
            .collect()
    };

    for (file_path, entry) in updates {
        let sidecar_path = get_sidecar_path(path, &file_path);
        save_index_entry(&sidecar_path, &entry)?;
        manifest.files.insert(file_path, entry.metadata);
    }

    if !manifest.files.is_empty() {
        manifest.updated = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        save_manifest(&manifest_path, &manifest)?;
    }

    Ok(())
}

pub fn clean_index(path: &Path) -> Result<()> {
    let index_dir = path.join(".ck");
    if index_dir.exists() {
        fs::remove_dir_all(&index_dir)?;
    }
    Ok(())
}

pub fn cleanup_index(
    path: &Path,
    respect_gitignore: bool,
    exclude_patterns: &[String],
) -> Result<CleanupStats> {
    let index_dir = path.join(".ck");
    if !index_dir.exists() {
        return Ok(CleanupStats::default());
    }

    let manifest_path = index_dir.join("manifest.json");
    let mut manifest = load_or_create_manifest(&manifest_path)?;

    // Find all current files in the repository
    let current_files = collect_files_as_hashset(path, respect_gitignore, exclude_patterns)?;

    let mut stats = CleanupStats::default();

    // Remove orphaned manifest entries (files that no longer exist)
    let orphaned_files: Vec<PathBuf> = manifest
        .files
        .keys()
        .filter(|&file_path| !current_files.contains(file_path))
        .cloned()
        .collect();

    for file_path in &orphaned_files {
        manifest.files.remove(file_path);

        // Also remove the sidecar file
        let sidecar_path = get_sidecar_path(path, file_path);
        if sidecar_path.exists() {
            fs::remove_file(&sidecar_path)?;
            stats.orphaned_sidecars_removed += 1;
        }

        stats.orphaned_entries_removed += 1;
    }

    // Find and remove orphaned sidecar files
    if index_dir.exists() {
        for entry in WalkDir::new(&index_dir) {
            let entry = entry?;
            if entry.file_type().is_file() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("ck") {
                    // Try to reconstruct the original file path
                    if let Some(original_path) = sidecar_to_original_path(path, &index_dir, path)
                        && !current_files.contains(&original_path)
                        && !manifest.files.contains_key(&original_path)
                    {
                        fs::remove_file(path)?;
                        stats.orphaned_sidecars_removed += 1;
                    }
                }
            }
        }
    }

    // Remove empty directories in .ck
    remove_empty_dirs(&index_dir)?;

    // Update manifest if changes were made
    if stats.orphaned_entries_removed > 0 {
        manifest.updated = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        save_manifest(&manifest_path, &manifest)?;
    }

    Ok(stats)
}

pub fn get_index_stats(path: &Path) -> Result<IndexStats> {
    let index_dir = path.join(".ck");
    if !index_dir.exists() {
        return Ok(IndexStats::default());
    }

    let manifest_path = index_dir.join("manifest.json");
    let manifest = load_or_create_manifest(&manifest_path)?;

    let mut stats = IndexStats {
        total_files: manifest.files.len(),
        index_created: manifest.created,
        index_updated: manifest.updated,
        ..Default::default()
    };

    // Calculate total chunks and size
    for file_path in manifest.files.keys() {
        let sidecar_path = get_sidecar_path(path, file_path);
        if sidecar_path.exists()
            && let Ok(entry) = load_index_entry(&sidecar_path)
        {
            stats.total_chunks += entry.chunks.len();
            stats.total_size_bytes += entry.metadata.size;

            // Count embedded chunks
            let embedded = entry
                .chunks
                .iter()
                .filter(|c| c.embedding.is_some())
                .count();
            stats.embedded_chunks += embedded;
        }
    }

    // Calculate index size on disk
    if let Ok(entries) = WalkDir::new(&index_dir)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
    {
        for entry in entries {
            if entry.file_type().is_file()
                && let Ok(metadata) = entry.metadata()
            {
                stats.index_size_bytes += metadata.len();
            }
        }
    }

    Ok(stats)
}

pub async fn smart_update_index(
    path: &Path,
    compute_embeddings: bool,
    respect_gitignore: bool,
    exclude_patterns: &[String],
) -> Result<UpdateStats> {
    smart_update_index_with_progress(
        path,
        false,
        None,
        compute_embeddings,
        respect_gitignore,
        exclude_patterns,
    )
    .await
}

pub async fn smart_update_index_with_progress(
    path: &Path,
    force_rebuild: bool,
    progress_callback: Option<ProgressCallback>,
    compute_embeddings: bool,
    respect_gitignore: bool,
    exclude_patterns: &[String],
) -> Result<UpdateStats> {
    let index_dir = path.join(".ck");
    let mut stats = UpdateStats::default();

    // Set up interrupt handler (only once per process)
    HANDLER_INIT.call_once(|| {
        let _ = ctrlc::set_handler(move || {
            INTERRUPTED.store(true, Ordering::SeqCst);
            eprintln!("\nIndexing interrupted by user. Cleaning up...");
        });
    });

    // Reset interrupt flag for this indexing operation
    INTERRUPTED.store(false, Ordering::SeqCst);

    if force_rebuild {
        clean_index(path)?;
        index_directory(
            path,
            compute_embeddings,
            respect_gitignore,
            exclude_patterns,
        )
        .await?;
        let index_stats = get_index_stats(path)?;
        stats.files_indexed = index_stats.total_files;
        return Ok(stats);
    }

    // Skip cleanup during regular updates to avoid removing valid sidecar files
    // Cleanup should only be done explicitly via --clean-orphans
    // let cleanup_stats = cleanup_index(path)?;
    // stats.orphaned_files_removed = cleanup_stats.orphaned_entries_removed;

    // Then perform incremental update
    fs::create_dir_all(&index_dir)?;
    let manifest_path = index_dir.join("manifest.json");
    let mut manifest = load_or_create_manifest(&manifest_path)?;

    let current_files = collect_files(path, respect_gitignore, exclude_patterns)?;

    // First pass: determine which files need updating and collect stats
    let mut files_to_update = Vec::new();
    let mut manifest_changed = false;

    for file_path in current_files {
        // Check for interrupt
        if INTERRUPTED.load(Ordering::SeqCst) {
            eprintln!("Indexing interrupted during file scanning.");
            return Ok(stats);
        }

        if let Some(metadata) = manifest.files.get(&file_path) {
            let fs_meta = match fs::metadata(&file_path) {
                Ok(m) => m,
                Err(_) => {
                    stats.files_errored += 1;
                    continue;
                }
            };

            let fs_last_modified = match fs_meta.modified().and_then(|m| {
                m.duration_since(SystemTime::UNIX_EPOCH)
                    .map_err(|_| std::io::Error::other("Time error"))
            }) {
                Ok(dur) => dur.as_secs(),
                Err(_) => {
                    stats.files_errored += 1;
                    continue;
                }
            };
            let fs_size = fs_meta.len();

            if fs_last_modified == metadata.last_modified && fs_size == metadata.size {
                stats.files_up_to_date += 1;
                continue;
            }

            let hash = match compute_file_hash(&file_path) {
                Ok(h) => h,
                Err(_) => {
                    stats.files_errored += 1;
                    continue;
                }
            };

            if hash != metadata.hash {
                stats.files_modified += 1;
                files_to_update.push(file_path);
            } else {
                stats.files_up_to_date += 1;
                let new_metadata = FileMetadata {
                    path: file_path.clone(),
                    hash,
                    last_modified: fs_last_modified,
                    size: fs_size,
                };
                manifest.files.insert(file_path, new_metadata);
                manifest_changed = true;
            }
        } else {
            stats.files_added += 1;
            files_to_update.push(file_path);
        }
    }

    // Second pass: index the files that need updating
    if compute_embeddings {
        // Sequential processing with streaming - write each file immediately
        let mut embedder = ck_embed::create_embedder(None)?;
        let mut _processed_count = 0;

        for file_path in files_to_update.iter() {
            // Check for interrupt
            if INTERRUPTED.load(Ordering::SeqCst) {
                eprintln!(
                    "Indexing interrupted. {} files processed.",
                    _processed_count
                );
                break;
            }

            if let Some(ref callback) = progress_callback
                && let Some(file_name) = file_path.file_name()
            {
                callback(&file_name.to_string_lossy());
            }

            match index_single_file(file_path, path, Some(&mut embedder)) {
                Ok(entry) => {
                    // Write sidecar immediately
                    let sidecar_path = get_sidecar_path(path, file_path);
                    save_index_entry(&sidecar_path, &entry)?;

                    // Update and save manifest immediately
                    manifest.files.insert(file_path.clone(), entry.metadata);
                    manifest.updated = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    save_manifest(&manifest_path, &manifest)?;
                    _processed_count += 1;
                }
                Err(e) => {
                    // Suppress warnings for binary files and UTF-8 errors in .git directories
                    let error_msg = e.to_string();
                    let is_binary_skip = error_msg.contains("Binary file, skipping");
                    let is_utf8_error = error_msg.contains("stream did not contain valid UTF-8");
                    let is_git_file = file_path.components().any(|c| c.as_os_str() == ".git");

                    if !(is_binary_skip || is_utf8_error && is_git_file) {
                        tracing::warn!("Failed to index {:?}: {}", file_path, e);
                    }
                    stats.files_errored += 1;
                }
            }
        }

        stats.files_indexed = _processed_count;
    } else {
        // Parallel processing with streaming using producer-consumer pattern
        use std::sync::mpsc;
        use std::thread;

        let (tx, rx) = mpsc::channel();
        let files_clone = files_to_update.clone();
        let path_clone = path.to_path_buf();

        // Spawn worker thread for parallel processing
        let worker_handle = thread::spawn(move || {
            files_clone.par_iter().for_each(|file_path| {
                // Check for interrupt
                if INTERRUPTED.load(Ordering::SeqCst) {
                    return;
                }

                match index_single_file(file_path, &path_clone, None) {
                    Ok(entry) => {
                        if tx.send((file_path.clone(), entry)).is_err() {
                            // Receiver dropped, stop processing
                        }
                    }
                    Err(e) => {
                        // Suppress warnings for binary files and UTF-8 errors in .git directories
                        let error_msg = e.to_string();
                        let is_binary_skip = error_msg.contains("Binary file, skipping");
                        let is_utf8_error =
                            error_msg.contains("stream did not contain valid UTF-8");
                        let is_git_file = file_path.components().any(|c| c.as_os_str() == ".git");

                        if !(is_binary_skip || is_utf8_error && is_git_file) {
                            tracing::warn!("Failed to index {:?}: {}", file_path, e);
                        }
                    }
                }
            });
        });

        // Main thread: stream results as they arrive
        let mut _processed_count = 0;
        while let Ok((file_path, entry)) = rx.recv() {
            // Check for interrupt
            if INTERRUPTED.load(Ordering::SeqCst) {
                eprintln!(
                    "Indexing interrupted. {} files processed.",
                    _processed_count
                );
                drop(rx); // Drop receiver to signal worker to stop
                break;
            }

            if let Some(ref callback) = progress_callback
                && let Some(file_name) = file_path.file_name()
            {
                callback(&file_name.to_string_lossy());
            }

            // Write sidecar immediately
            let sidecar_path = get_sidecar_path(path, &file_path);
            save_index_entry(&sidecar_path, &entry)?;

            // Update and save manifest immediately
            manifest.files.insert(file_path, entry.metadata);
            manifest.updated = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            save_manifest(&manifest_path, &manifest)?;
            _processed_count += 1;
        }

        stats.files_indexed = _processed_count;

        // Wait for worker to complete
        worker_handle
            .join()
            .map_err(|_| anyhow::anyhow!("Worker thread panicked"))?;
    }

    // For sequential processing (embeddings), manifest is already saved after each file
    // Only save manifest for parallel processing or if there were metadata-only changes
    if !compute_embeddings
        && (stats.files_indexed > 0 || stats.orphaned_files_removed > 0 || manifest_changed)
    {
        manifest.updated = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        save_manifest(&manifest_path, &manifest)?;
    }

    Ok(stats)
}

fn index_single_file(
    file_path: &Path,
    _repo_root: &Path,
    embedder: Option<&mut Box<dyn ck_embed::Embedder>>,
) -> Result<IndexEntry> {
    // Skip binary files to avoid UTF-8 warnings
    if !is_text_file(file_path) {
        return Err(anyhow::anyhow!("Binary file, skipping"));
    }

    let content = fs::read_to_string(file_path)?;
    let hash = compute_file_hash(file_path)?;
    let metadata = fs::metadata(file_path)?;

    let file_metadata = FileMetadata {
        path: file_path.to_path_buf(),
        hash,
        last_modified: metadata
            .modified()?
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs(),
        size: metadata.len(),
    };

    // Detect language for tree-sitter parsing
    let lang = ck_core::Language::from_path(file_path);

    let chunks = ck_chunk::chunk_text(&content, lang)?;

    let chunk_entries: Vec<ChunkEntry> = if let Some(embedder) = embedder {
        // Compute embeddings for all chunks
        let chunk_texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
        tracing::info!(
            "Computing embeddings for {} chunks in {:?}",
            chunk_texts.len(),
            file_path
        );
        let embeddings = embedder.embed(&chunk_texts)?;

        chunks
            .into_iter()
            .zip(embeddings)
            .map(|(chunk, embedding)| {
                let chunk_type_str = match chunk.chunk_type {
                    ck_chunk::ChunkType::Function => Some("function".to_string()),
                    ck_chunk::ChunkType::Class => Some("class".to_string()),
                    ck_chunk::ChunkType::Method => Some("method".to_string()),
                    ck_chunk::ChunkType::Module => Some("module".to_string()),
                    ck_chunk::ChunkType::Text => None,
                };
                ChunkEntry {
                    span: chunk.span,
                    embedding: Some(embedding),
                    chunk_type: chunk_type_str,
                }
            })
            .collect()
    } else {
        // No embedder, just store spans without embeddings
        chunks
            .into_iter()
            .map(|chunk| {
                let chunk_type_str = match chunk.chunk_type {
                    ck_chunk::ChunkType::Function => Some("function".to_string()),
                    ck_chunk::ChunkType::Class => Some("class".to_string()),
                    ck_chunk::ChunkType::Method => Some("method".to_string()),
                    ck_chunk::ChunkType::Module => Some("module".to_string()),
                    ck_chunk::ChunkType::Text => None,
                };
                ChunkEntry {
                    span: chunk.span,
                    embedding: None,
                    chunk_type: chunk_type_str,
                }
            })
            .collect()
    };

    Ok(IndexEntry {
        metadata: file_metadata,
        chunks: chunk_entries,
    })
}

fn load_or_create_manifest(path: &Path) -> Result<IndexManifest> {
    if path.exists() {
        let data = fs::read(path)?;
        Ok(serde_json::from_slice(&data)?)
    } else {
        Ok(IndexManifest::default())
    }
}

fn save_manifest(path: &Path, manifest: &IndexManifest) -> Result<()> {
    let data = serde_json::to_vec_pretty(manifest)?;
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, data)?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

fn save_index_entry(path: &Path, entry: &IndexEntry) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = bincode::serialize(entry)?;
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, data)?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

pub fn load_index_entry(path: &Path) -> Result<IndexEntry> {
    let data = fs::read(path)?;
    Ok(bincode::deserialize(&data)?)
}

fn find_repo_root(path: &Path) -> Result<PathBuf> {
    let mut current = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };

    loop {
        if current.join(".ck").exists() || current.join(".git").exists() {
            return Ok(current.to_path_buf());
        }

        match current.parent() {
            Some(parent) => current = parent,
            None => return Ok(path.to_path_buf()),
        }
    }
}

fn is_text_file(path: &Path) -> bool {
    // Use NUL byte heuristic like ripgrep - read first 8KB and check for NUL bytes
    const BUFFER_SIZE: usize = 8192;

    match std::fs::File::open(path) {
        Ok(mut file) => {
            let mut buffer = vec![0; BUFFER_SIZE];
            match file.read(&mut buffer) {
                Ok(bytes_read) => {
                    // If file is empty, consider it text
                    if bytes_read == 0 {
                        return true;
                    }

                    // Check for NUL bytes in the read portion
                    !buffer[..bytes_read].contains(&0)
                }
                Err(_) => false, // If we can't read, assume binary
            }
        }
        Err(_) => false, // If we can't open, assume binary
    }
}

fn sidecar_to_original_path(
    sidecar_path: &Path,
    index_dir: &Path,
    _repo_root: &Path,
) -> Option<PathBuf> {
    let relative_path = sidecar_path.strip_prefix(index_dir).ok()?;
    let original_path = relative_path.with_extension("");

    // Handle the .ck extension removal
    if let Some(name) = original_path.file_name() {
        let name_str = name.to_string_lossy();
        if let Some(original_name) = name_str.strip_suffix(".ck") {
            let mut result = original_path.clone();
            result.set_file_name(original_name);
            return Some(result);
        }
    }

    Some(original_path)
}

fn remove_empty_dirs(dir: &Path) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            remove_empty_dirs(&path)?;
            // Try to remove if now empty
            if fs::read_dir(&path)?.next().is_none() {
                let _ = fs::remove_dir(&path);
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CleanupStats {
    pub orphaned_entries_removed: usize,
    pub orphaned_sidecars_removed: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexStats {
    pub total_files: usize,
    pub total_chunks: usize,
    pub embedded_chunks: usize,
    pub total_size_bytes: u64,
    pub index_size_bytes: u64,
    pub index_created: u64,
    pub index_updated: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateStats {
    pub files_indexed: usize,
    pub files_added: usize,
    pub files_modified: usize,
    pub files_up_to_date: usize,
    pub files_errored: usize,
    pub orphaned_files_removed: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_smart_update_index() {
        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path();

        // Create initial file
        fs::write(test_path.join("file1.txt"), "initial content").unwrap();

        // First index
        let stats1 = smart_update_index(test_path, false, true, &[])
            .await
            .unwrap();
        assert_eq!(stats1.files_added, 1);
        assert_eq!(stats1.files_indexed, 1);

        // No changes, should be up to date
        let stats2 = smart_update_index(test_path, false, true, &[])
            .await
            .unwrap();
        assert_eq!(stats2.files_up_to_date, 1);
        assert_eq!(stats2.files_indexed, 0);

        // Modify file
        fs::write(test_path.join("file1.txt"), "modified content").unwrap();
        let stats3 = smart_update_index(test_path, false, true, &[])
            .await
            .unwrap();
        assert_eq!(stats3.files_modified, 1);
        assert_eq!(stats3.files_indexed, 1);

        // Add new file
        fs::write(test_path.join("file2.txt"), "new file content").unwrap();
        let stats4 = smart_update_index(test_path, false, true, &[])
            .await
            .unwrap();
        assert_eq!(stats4.files_added, 1);
        assert_eq!(stats4.files_up_to_date, 1);
        assert_eq!(stats4.files_indexed, 1);
    }

    #[test]
    fn test_cleanup_index() {
        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path();

        // Create index directory and manifest
        let index_dir = test_path.join(".ck");
        fs::create_dir_all(&index_dir).unwrap();

        let mut manifest = IndexManifest::default();
        manifest.files.insert(
            test_path.join("deleted_file.txt"),
            FileMetadata {
                path: test_path.join("deleted_file.txt"),
                hash: "fake_hash".to_string(),
                last_modified: 0,
                size: 0,
            },
        );

        let manifest_path = index_dir.join("manifest.json");
        save_manifest(&manifest_path, &manifest).unwrap();

        // Cleanup should remove orphaned entry
        let stats = cleanup_index(test_path, true, &[]).unwrap();
        assert_eq!(stats.orphaned_entries_removed, 1);

        // Check that manifest was updated
        let updated_manifest = load_or_create_manifest(&manifest_path).unwrap();
        assert_eq!(updated_manifest.files.len(), 0);
    }

    #[test]
    fn test_get_index_stats() {
        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path();

        // No index exists
        let stats = get_index_stats(test_path).unwrap();
        assert_eq!(stats.total_files, 0);

        // Create index
        let index_dir = test_path.join(".ck");
        fs::create_dir_all(&index_dir).unwrap();

        let mut manifest = IndexManifest::default();
        manifest.files.insert(
            test_path.join("test.txt"),
            FileMetadata {
                path: test_path.join("test.txt"),
                hash: "test_hash".to_string(),
                last_modified: 1234567890,
                size: 100,
            },
        );

        let manifest_path = index_dir.join("manifest.json");
        save_manifest(&manifest_path, &manifest).unwrap();

        let stats = get_index_stats(test_path).unwrap();
        assert_eq!(stats.total_files, 1);
    }

    #[test]
    fn test_sidecar_to_original_path() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join(".ck");

        // Test normal file
        let sidecar = index_dir.join("test.txt.ck");
        let original = sidecar_to_original_path(&sidecar, &index_dir, temp_dir.path());
        assert_eq!(original, Some(PathBuf::from("test.txt")));

        // Test nested file
        let nested_sidecar = index_dir.join("src").join("main.rs.ck");
        let nested_original =
            sidecar_to_original_path(&nested_sidecar, &index_dir, temp_dir.path());
        assert_eq!(nested_original, Some(PathBuf::from("src/main.rs")));
    }

    #[test]
    fn test_is_text_file() {
        use std::fs::File;
        use std::io::Write;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a text file (no NUL bytes)
        let text_file = temp_path.join("test.txt");
        let mut file = File::create(&text_file).unwrap();
        file.write_all(b"Hello world\nThis is text content")
            .unwrap();
        assert!(is_text_file(&text_file));

        // Create a text file with unusual extension
        let log_file = temp_path.join("app.log");
        let mut file = File::create(&log_file).unwrap();
        file.write_all(b"2024-01-15 ERROR: Failed to connect")
            .unwrap();
        assert!(is_text_file(&log_file));

        // Create a file without extension but with text content
        let no_ext_file = temp_path.join("README");
        let mut file = File::create(&no_ext_file).unwrap();
        file.write_all(b"This is a README file").unwrap();
        assert!(is_text_file(&no_ext_file));

        // Create a binary file with NUL bytes
        let binary_file = temp_path.join("test.bin");
        let mut file = File::create(&binary_file).unwrap();
        file.write_all(&[
            0x48, 0x65, 0x6C, 0x6C, 0x6F, 0x00, 0x57, 0x6F, 0x72, 0x6C, 0x64,
        ])
        .unwrap(); // "Hello\0World"
        assert!(!is_text_file(&binary_file));

        // Create an empty file (should be considered text)
        let empty_file = temp_path.join("empty.txt");
        File::create(&empty_file).unwrap();
        assert!(is_text_file(&empty_file));

        // Test non-existent file (should return false)
        let nonexistent = temp_path.join("nonexistent.txt");
        assert!(!is_text_file(&nonexistent));
    }

    #[test]
    fn test_remove_empty_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path();

        // Create nested empty directories
        let nested_dir = test_path.join("level1").join("level2").join("level3");
        fs::create_dir_all(&nested_dir).unwrap();

        // Remove empty dirs
        remove_empty_dirs(test_path).unwrap();

        // Check that empty dirs were removed
        assert!(!nested_dir.exists());
        assert!(!test_path.join("level1").join("level2").exists());
        assert!(!test_path.join("level1").exists());
    }
}
