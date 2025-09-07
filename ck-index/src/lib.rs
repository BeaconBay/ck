use anyhow::Result;
use ck_core::{FileMetadata, Span, compute_file_hash, get_sidecar_path, get_default_exclude_patterns};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;
use rayon::prelude::*;

pub type ProgressCallback = Box<dyn Fn(&str) + Send + Sync>;

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

fn should_exclude_path(path: &Path, exclude_patterns: &[String]) -> bool {
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

pub async fn index_directory(path: &Path, compute_embeddings: bool) -> Result<()> {
    tracing::info!("index_directory called with compute_embeddings={}", compute_embeddings);
    let index_dir = path.join(".ck");
    fs::create_dir_all(&index_dir)?;
    
    let manifest_path = index_dir.join("manifest.json");
    let mut manifest = load_or_create_manifest(&manifest_path)?;
    
    let exclude_patterns = get_default_exclude_patterns();
    let files: Vec<PathBuf> = WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| {
            !e.path().starts_with(&index_dir) && 
            !should_exclude_path(e.path(), &exclude_patterns)
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| is_text_file(e.path()))
        .map(|e| e.path().to_path_buf())
        .collect();
    
    let entries: Vec<(PathBuf, IndexEntry)> = if compute_embeddings {
        // Sequential processing when computing embeddings (embedder not thread-safe)
        tracing::info!("Creating embedder for {} files", files.len());
        let mut embedder = ck_embed::create_embedder(None)?;
        files
            .iter()
            .filter_map(|file_path| {
                match index_single_file(file_path, path, Some(&mut embedder)) {
                    Ok(entry) => Some((file_path.clone(), entry)),
                    Err(e) => {
                        tracing::warn!("Failed to index {:?}: {}", file_path, e);
                        None
                    }
                }
            })
            .collect()
    } else {
        // Parallel processing when not computing embeddings
        files
            .par_iter()
            .filter_map(|file_path| {
                match index_single_file(file_path, path, None) {
                    Ok(entry) => Some((file_path.clone(), entry)),
                    Err(e) => {
                        tracing::warn!("Failed to index {:?}: {}", file_path, e);
                        None
                    }
                }
            })
            .collect()
    };
    
    for (file_path, entry) in entries {
        let sidecar_path = get_sidecar_path(path, &file_path);
        save_index_entry(&sidecar_path, &entry)?;
        manifest.files.insert(file_path, entry.metadata);
    }
    
    manifest.updated = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    save_manifest(&manifest_path, &manifest)?;
    
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
    manifest.files.insert(file_path.to_path_buf(), entry.metadata);
    manifest.updated = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    save_manifest(&manifest_path, &manifest)?;
    
    Ok(())
}

pub async fn update_index(path: &Path, compute_embeddings: bool) -> Result<()> {
    let index_dir = path.join(".ck");
    if !index_dir.exists() {
        return index_directory(path, compute_embeddings).await;
    }
    
    let manifest_path = index_dir.join("manifest.json");
    let mut manifest = load_or_create_manifest(&manifest_path)?;
    
    let exclude_patterns = get_default_exclude_patterns();
    let files: Vec<PathBuf> = WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| {
            !e.path().starts_with(&index_dir) && 
            !should_exclude_path(e.path(), &exclude_patterns)
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| is_text_file(e.path()))
        .map(|e| e.path().to_path_buf())
        .collect();
    
    let updates: Vec<(PathBuf, IndexEntry)> = if compute_embeddings {
        // Sequential processing when computing embeddings
        let mut embedder = ck_embed::create_embedder(None)?;
        files
            .iter()
            .filter_map(|file_path| {
                let needs_update = match manifest.files.get(file_path) {
                    Some(metadata) => {
                        match compute_file_hash(file_path) {
                            Ok(hash) => hash != metadata.hash,
                            Err(_) => false,
                        }
                    }
                    None => true,
                };
                if needs_update {
                    match index_single_file(file_path, path, Some(&mut embedder)) {
                        Ok(entry) => Some((file_path.clone(), entry)),
                        Err(e) => {
                            tracing::warn!("Failed to index {:?}: {}", file_path, e);
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
                    Some(metadata) => {
                        match compute_file_hash(file_path) {
                            Ok(hash) => hash != metadata.hash,
                            Err(_) => false,
                        }
                    }
                    None => true,
                };
                
                if needs_update {
                    match index_single_file(file_path, path, None) {
                        Ok(entry) => Some((file_path.clone(), entry)),
                        Err(e) => {
                            tracing::warn!("Failed to index {:?}: {}", file_path, e);
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

pub fn cleanup_index(path: &Path) -> Result<CleanupStats> {
    let index_dir = path.join(".ck");
    if !index_dir.exists() {
        return Ok(CleanupStats::default());
    }
    
    let manifest_path = index_dir.join("manifest.json");
    let mut manifest = load_or_create_manifest(&manifest_path)?;
    
    // Find all current files in the repository
    let exclude_patterns = get_default_exclude_patterns();
    let current_files: HashSet<PathBuf> = WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| {
            !e.path().starts_with(&index_dir) && 
            !should_exclude_path(e.path(), &exclude_patterns)
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| is_text_file(e.path()))
        .map(|e| e.path().to_path_buf())
        .collect();
    
    let mut stats = CleanupStats::default();
    
    // Remove orphaned manifest entries (files that no longer exist)
    let orphaned_files: Vec<PathBuf> = manifest.files
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
                    if let Some(original_path) = sidecar_to_original_path(path, &index_dir, path) {
                        if !current_files.contains(&original_path) && !manifest.files.contains_key(&original_path) {
                            fs::remove_file(path)?;
                            stats.orphaned_sidecars_removed += 1;
                        }
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
        if sidecar_path.exists() {
            if let Ok(entry) = load_index_entry(&sidecar_path) {
                stats.total_chunks += entry.chunks.len();
                stats.total_size_bytes += entry.metadata.size;
                
                // Count embedded chunks
                let embedded = entry.chunks.iter().filter(|c| c.embedding.is_some()).count();
                stats.embedded_chunks += embedded;
            }
        }
    }
    
    // Calculate index size on disk
    if let Ok(entries) = WalkDir::new(&index_dir).into_iter().collect::<Result<Vec<_>, _>>() {
        for entry in entries {
            if entry.file_type().is_file() {
                if let Ok(metadata) = entry.metadata() {
                    stats.index_size_bytes += metadata.len();
                }
            }
        }
    }
    
    Ok(stats)
}

pub async fn smart_update_index(path: &Path, compute_embeddings: bool) -> Result<UpdateStats> {
    smart_update_index_with_progress(path, false, None, compute_embeddings).await
}

pub async fn smart_update_index_with_progress(path: &Path, force_rebuild: bool, progress_callback: Option<ProgressCallback>, compute_embeddings: bool) -> Result<UpdateStats> {
    let index_dir = path.join(".ck");
    let mut stats = UpdateStats::default();
    
    if force_rebuild {
        clean_index(path)?;
        index_directory(path, compute_embeddings).await?;
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
    
    let exclude_patterns = get_default_exclude_patterns();
    let current_files: Vec<PathBuf> = WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| {
            !e.path().starts_with(&index_dir) && 
            !should_exclude_path(e.path(), &exclude_patterns)
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| is_text_file(e.path()))
        .map(|e| e.path().to_path_buf())
        .collect();
    
    // First pass: determine which files need updating and collect stats
    let mut files_to_update = Vec::new();
    
    for file_path in current_files {
        let needs_update = match manifest.files.get(&file_path) {
            Some(metadata) => {
                match compute_file_hash(&file_path) {
                    Ok(hash) => {
                        if hash != metadata.hash {
                            stats.files_modified += 1;
                            true
                        } else {
                            stats.files_up_to_date += 1;
                            false
                        }
                    }
                    Err(_) => {
                        stats.files_errored += 1;
                        false
                    }
                }
            }
            None => {
                stats.files_added += 1;
                true
            }
        };
        
        if needs_update {
            files_to_update.push(file_path);
        }
    }
    
    // Second pass: index the files that need updating
    let updates: Vec<(PathBuf, IndexEntry)> = if compute_embeddings {
        // Sequential processing when computing embeddings
        let mut embedder = ck_embed::create_embedder(None)?;
        files_to_update
            .iter()
            .filter_map(|file_path| {
                if let Some(ref callback) = progress_callback {
                    if let Some(file_name) = file_path.file_name() {
                        callback(&file_name.to_string_lossy());
                    }
                }
                match index_single_file(file_path, path, Some(&mut embedder)) {
                    Ok(entry) => Some((file_path.clone(), entry)),
                    Err(e) => {
                        tracing::warn!("Failed to index {:?}: {}", file_path, e);
                        None
                    }
                }
            })
            .collect()
    } else {
        // Parallel processing when not computing embeddings
        files_to_update
            .par_iter()
            .filter_map(|file_path| {
                if let Some(ref callback) = progress_callback {
                    if let Some(file_name) = file_path.file_name() {
                        callback(&file_name.to_string_lossy());
                    }
                }
                match index_single_file(file_path, path, None) {
                    Ok(entry) => Some((file_path.clone(), entry)),
                    Err(e) => {
                        tracing::warn!("Failed to index {:?}: {}", file_path, e);
                        None
                    }
                }
            })
            .collect()
    };
    
    stats.files_indexed = updates.len();
    
    for (file_path, entry) in updates {
        let sidecar_path = get_sidecar_path(path, &file_path);
        save_index_entry(&sidecar_path, &entry)?;
        manifest.files.insert(file_path, entry.metadata);
    }
    
    if stats.files_indexed > 0 || stats.orphaned_files_removed > 0 {
        manifest.updated = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        save_manifest(&manifest_path, &manifest)?;
    }
    
    Ok(stats)
}

fn index_single_file(file_path: &Path, _repo_root: &Path, embedder: Option<&mut Box<dyn ck_embed::Embedder>>) -> Result<IndexEntry> {
    let content = fs::read_to_string(file_path)?;
    let hash = compute_file_hash(file_path)?;
    let metadata = fs::metadata(file_path)?;
    
    let file_metadata = FileMetadata {
        path: file_path.to_path_buf(),
        hash,
        last_modified: metadata.modified()?
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs(),
        size: metadata.len(),
    };
    
    // Detect language for tree-sitter parsing
    let lang = match file_path.extension().and_then(|s| s.to_str()) {
        Some("py") => Some("python"),
        Some("js") => Some("javascript"),
        Some("ts") | Some("tsx") => Some("typescript"),
        Some("hs") | Some("lhs") => Some("haskell"),
        _ => None,
    };
    
    let chunks = ck_chunk::chunk_text(&content, lang)?;
    
    let chunk_entries: Vec<ChunkEntry> = if let Some(embedder) = embedder {
        // Compute embeddings for all chunks
        let chunk_texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
        tracing::info!("Computing embeddings for {} chunks in {:?}", chunk_texts.len(), file_path);
        let embeddings = embedder.embed(&chunk_texts)?;
        
        chunks.into_iter().zip(embeddings.into_iter()).map(|(chunk, embedding)| {
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
        }).collect()
    } else {
        // No embedder, just store spans without embeddings
        chunks.into_iter().map(|chunk| {
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
        }).collect()
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
    match path.extension() {
        Some(ext) => {
            let ext = ext.to_string_lossy().to_lowercase();
            matches!(
                ext.as_str(),
                "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "go" | "java" | "c" | "cpp" | "cc" | "cxx" | "h" | 
                "hpp" | "cs" | "rb" | "php" | "swift" | "kt" | "scala" | "r" | "m" | "mm" |
                "dart" | "jl" | "groovy" | "clj" | "cljs" | "fs" | "fsx" | "erl" | "ex" | "exs" |
                "txt" | "md" | "json" | "yaml" | "yml" | "toml" | "xml" | "html" | "css" |
                "sh" | "bash" | "zsh" | "fish" | "ps1" | "sql" | "vim" | "lua" | "el" | "hs" | "lhs"
            )
        }
        None => false,
    }
}

fn sidecar_to_original_path(sidecar_path: &Path, index_dir: &Path, _repo_root: &Path) -> Option<PathBuf> {
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
        let stats1 = smart_update_index(test_path, false).await.unwrap();
        assert_eq!(stats1.files_added, 1);
        assert_eq!(stats1.files_indexed, 1);
        
        // No changes, should be up to date
        let stats2 = smart_update_index(test_path, false).await.unwrap();
        assert_eq!(stats2.files_up_to_date, 1);
        assert_eq!(stats2.files_indexed, 0);
        
        // Modify file
        fs::write(test_path.join("file1.txt"), "modified content").unwrap();
        let stats3 = smart_update_index(test_path, false).await.unwrap();
        assert_eq!(stats3.files_modified, 1);
        assert_eq!(stats3.files_indexed, 1);
        
        // Add new file
        fs::write(test_path.join("file2.txt"), "new file content").unwrap();
        let stats4 = smart_update_index(test_path, false).await.unwrap();
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
            }
        );
        
        let manifest_path = index_dir.join("manifest.json");
        save_manifest(&manifest_path, &manifest).unwrap();
        
        // Cleanup should remove orphaned entry
        let stats = cleanup_index(test_path).unwrap();
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
            }
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
        let nested_original = sidecar_to_original_path(&nested_sidecar, &index_dir, temp_dir.path());
        assert_eq!(nested_original, Some(PathBuf::from("src/main.rs")));
    }

    #[test]
    fn test_is_text_file() {
        assert!(is_text_file(&PathBuf::from("test.rs")));
        assert!(is_text_file(&PathBuf::from("test.py")));
        assert!(is_text_file(&PathBuf::from("test.hs")));
        assert!(is_text_file(&PathBuf::from("test.lhs")));
        assert!(is_text_file(&PathBuf::from("test.kt")));
        assert!(is_text_file(&PathBuf::from("test.scala")));
        assert!(is_text_file(&PathBuf::from("test.dart")));
        assert!(is_text_file(&PathBuf::from("test.jl")));
        assert!(is_text_file(&PathBuf::from("test.txt")));
        assert!(is_text_file(&PathBuf::from("test.md")));
        assert!(!is_text_file(&PathBuf::from("test.exe")));
        assert!(!is_text_file(&PathBuf::from("test.png")));
        assert!(!is_text_file(&PathBuf::from("test")));
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