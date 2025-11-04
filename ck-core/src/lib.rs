pub mod heatmap;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CkError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Index error: {0}")]
    Index(String),

    #[error("Search error: {0}")]
    Search(String),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("Span validation error: {0}")]
    SpanValidation(String),

    #[error("Other error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, CkError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Haskell,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Ruby,
    Php,
    Swift,
    Kotlin,
    Zig,
    Pdf,
}

impl Language {
    pub fn from_extension(ext: &str) -> Option<Self> {
        // Convert to lowercase for case-insensitive matching
        match ext.to_lowercase().as_str() {
            "rs" => Some(Language::Rust),
            "py" => Some(Language::Python),
            "js" => Some(Language::JavaScript),
            "ts" | "tsx" => Some(Language::TypeScript),
            "hs" | "lhs" => Some(Language::Haskell),
            "go" => Some(Language::Go),
            "java" => Some(Language::Java),
            "c" => Some(Language::C),
            "cpp" | "cc" | "cxx" | "c++" => Some(Language::Cpp),
            "h" | "hpp" => Some(Language::Cpp), // Assume C++ for headers
            "cs" => Some(Language::CSharp),
            "rb" => Some(Language::Ruby),
            "php" => Some(Language::Php),
            "swift" => Some(Language::Swift),
            "kt" | "kts" => Some(Language::Kotlin),
            "zig" => Some(Language::Zig),
            "pdf" => Some(Language::Pdf),
            _ => None,
        }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(Self::from_extension)
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Language::Rust => "rust",
            Language::Python => "python",
            Language::JavaScript => "javascript",
            Language::TypeScript => "typescript",
            Language::Haskell => "haskell",
            Language::Go => "go",
            Language::Java => "java",
            Language::C => "c",
            Language::Cpp => "cpp",
            Language::CSharp => "csharp",
            Language::Ruby => "ruby",
            Language::Php => "php",
            Language::Swift => "swift",
            Language::Kotlin => "kotlin",
            Language::Zig => "zig",
            Language::Pdf => "pdf",
        };
        write!(f, "{}", name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub byte_start: usize,
    pub byte_end: usize,
    pub line_start: usize,
    pub line_end: usize,
}

impl Span {
    /// Create a new Span with validation
    pub fn new(
        byte_start: usize,
        byte_end: usize,
        line_start: usize,
        line_end: usize,
    ) -> Result<Self> {
        let span = Self {
            byte_start,
            byte_end,
            line_start,
            line_end,
        };
        span.validate()?;
        Ok(span)
    }

    /// Create a new Span without validation (for backward compatibility)
    ///
    /// # Safety
    ///
    /// The caller must ensure the span is valid. Use `new()` for validated construction.
    pub fn new_unchecked(
        byte_start: usize,
        byte_end: usize,
        line_start: usize,
        line_end: usize,
    ) -> Self {
        Self {
            byte_start,
            byte_end,
            line_start,
            line_end,
        }
    }

    /// Validate span invariants
    pub fn validate(&self) -> Result<()> {
        // Check for zero line numbers first (lines should be 1-indexed)
        if self.line_start == 0 {
            return Err(CkError::SpanValidation(
                "Line start cannot be zero (lines are 1-indexed)".to_string(),
            ));
        }

        if self.line_end == 0 {
            return Err(CkError::SpanValidation(
                "Line end cannot be zero (lines are 1-indexed)".to_string(),
            ));
        }

        // Check byte range validity
        if self.byte_start > self.byte_end {
            return Err(CkError::SpanValidation(format!(
                "Invalid byte range: start ({}) > end ({})",
                self.byte_start, self.byte_end
            )));
        }

        // Check line range validity
        if self.line_start > self.line_end {
            return Err(CkError::SpanValidation(format!(
                "Invalid line range: start ({}) > end ({})",
                self.line_start, self.line_end
            )));
        }

        Ok(())
    }

    /// Check if this span is valid
    pub fn is_valid(&self) -> bool {
        self.validate().is_ok()
    }

    /// Get byte length of the span
    pub fn byte_len(&self) -> usize {
        self.byte_end.saturating_sub(self.byte_start)
    }

    /// Get line count of the span
    pub fn line_count(&self) -> usize {
        self.line_end.saturating_sub(self.line_start) + 1
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub path: PathBuf,
    pub hash: String,
    pub last_modified: u64,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub file: PathBuf,
    pub span: Span,
    pub score: f32,
    pub preview: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<Language>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_epoch: Option<u64>,
}

/// Enhanced search results that include near-miss information for threshold queries
#[derive(Debug, Clone)]
pub struct SearchResults {
    pub matches: Vec<SearchResult>,
    /// The highest scoring result below the threshold (if any)
    pub closest_below_threshold: Option<SearchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSearchResult {
    pub file: String,
    pub span: Span,
    pub lang: Option<Language>,
    pub symbol: Option<String>,
    pub score: f32,
    pub signals: SearchSignals,
    pub preview: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonlSearchResult {
    pub path: String,
    pub span: Span,
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_epoch: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchSignals {
    pub lex_rank: Option<usize>,
    pub vec_rank: Option<usize>,
    pub rrf_score: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SearchMode {
    Regex,
    Lexical,
    Semantic,
    Hybrid,
}

#[derive(Debug, Clone)]
pub struct IncludePattern {
    pub path: PathBuf,
    pub is_dir: bool,
}

/// Configuration for file collection during indexing and search operations.
/// This struct encapsulates all settings related to which files should be included
/// or excluded when traversing a directory tree.
#[derive(Debug, Clone)]
pub struct FileCollectionOptions {
    /// Whether to respect .gitignore files
    pub respect_gitignore: bool,
    /// Whether to respect .ckignore files hierarchically
    pub use_ckignore: bool,
    /// Patterns to exclude files/directories
    pub exclude_patterns: Vec<String>,
}

impl From<&SearchOptions> for FileCollectionOptions {
    fn from(opts: &SearchOptions) -> Self {
        Self {
            respect_gitignore: opts.respect_gitignore,
            use_ckignore: true, // Always use .ckignore for hierarchical ignore support
            exclude_patterns: opts.exclude_patterns.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub mode: SearchMode,
    pub query: String,
    pub path: PathBuf,
    pub top_k: Option<usize>,
    pub threshold: Option<f32>,
    pub case_insensitive: bool,
    pub whole_word: bool,
    pub fixed_string: bool,
    pub line_numbers: bool,
    pub context_lines: usize,
    pub before_context_lines: usize,
    pub after_context_lines: usize,
    pub recursive: bool,
    pub json_output: bool,
    pub jsonl_output: bool,
    pub no_snippet: bool,
    pub reindex: bool,
    pub show_scores: bool,
    pub show_filenames: bool,
    pub files_with_matches: bool,
    pub files_without_matches: bool,
    pub exclude_patterns: Vec<String>,
    pub include_patterns: Vec<IncludePattern>,
    pub respect_gitignore: bool,
    pub use_ckignore: bool,
    pub full_section: bool,
    // Enhanced embedding options (search-time only)
    pub rerank: bool,
    pub rerank_model: Option<String>,
    pub embedding_model: Option<String>,
}

impl JsonlSearchResult {
    pub fn from_search_result(result: &SearchResult, include_snippet: bool) -> Self {
        Self {
            path: result.file.to_string_lossy().to_string(),
            span: result.span.clone(),
            language: result.lang.as_ref().map(|l| l.to_string()),
            snippet: if include_snippet {
                Some(result.preview.clone())
            } else {
                None
            },
            score: if result.score >= 0.0 {
                Some(result.score)
            } else {
                None
            },
            chunk_hash: result.chunk_hash.clone(),
            index_epoch: result.index_epoch,
        }
    }
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            mode: SearchMode::Regex,
            query: String::new(),
            path: PathBuf::from("."),
            top_k: None,
            threshold: None,
            case_insensitive: false,
            whole_word: false,
            fixed_string: false,
            line_numbers: false,
            context_lines: 0,
            before_context_lines: 0,
            after_context_lines: 0,
            recursive: true,
            json_output: false,
            jsonl_output: false,
            no_snippet: false,
            reindex: false,
            show_scores: false,
            show_filenames: false,
            files_with_matches: false,
            files_without_matches: false,
            exclude_patterns: get_default_exclude_patterns(),
            include_patterns: Vec::new(),
            respect_gitignore: true,
            use_ckignore: true,
            full_section: false,
            // Enhanced embedding options (search-time only)
            rerank: false,
            rerank_model: None,
            embedding_model: None,
        }
    }
}

/// Get default exclusion patterns for directories that should be skipped during search.
/// These are common cache, build, and system directories that rarely contain user code.
pub fn get_default_exclude_patterns() -> Vec<String> {
    vec![
        // ck's own index directory
        ".ck".to_string(),
        // AI/ML model cache directories
        ".fastembed_cache".to_string(),
        ".cache".to_string(),
        "__pycache__".to_string(),
        // Version control
        ".git".to_string(),
        ".svn".to_string(),
        ".hg".to_string(),
        // Build directories
        "target".to_string(),       // Rust
        "build".to_string(),        // Various
        "dist".to_string(),         // JavaScript/Python
        "node_modules".to_string(), // JavaScript
        ".gradle".to_string(),      // Java
        ".mvn".to_string(),         // Maven
        "bin".to_string(),          // Various
        "obj".to_string(),          // .NET
        // Python virtual environments
        "venv".to_string(),
        ".venv".to_string(),
        "env".to_string(),
        ".env".to_string(),
        "virtualenv".to_string(),
        // IDE/Editor directories
        ".vscode".to_string(),
        ".idea".to_string(),
        ".eclipse".to_string(),
        // Temporary directories
        "tmp".to_string(),
        "temp".to_string(),
        ".tmp".to_string(),
    ]
}

/// Get default .ckignore file content
pub fn get_default_ckignore_content() -> &'static str {
    r#"# .ckignore - Default patterns for ck semantic search
# Created automatically during first index
# Syntax: same as .gitignore (glob patterns, ! for negation)

# Images
*.png
*.jpg
*.jpeg
*.gif
*.bmp
*.svg
*.ico
*.webp
*.tiff

# Video
*.mp4
*.avi
*.mov
*.mkv
*.wmv
*.flv
*.webm

# Audio
*.mp3
*.wav
*.flac
*.aac
*.ogg
*.m4a

# Binary/Compiled
*.exe
*.dll
*.so
*.dylib
*.a
*.lib
*.obj
*.o

# Archives
*.zip
*.tar
*.tar.gz
*.tgz
*.rar
*.7z
*.bz2
*.gz

# Data files
*.db
*.sqlite
*.sqlite3
*.parquet
*.arrow

# Config formats (issue #27)
*.json
*.yaml
*.yml

# Add your custom patterns below this line
"#
}

/// Read and parse .ckignore file, returning patterns
pub fn read_ckignore_patterns(repo_root: &Path) -> Result<Vec<String>> {
    let ckignore_path = repo_root.join(".ckignore");

    if !ckignore_path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&ckignore_path).map_err(CkError::Io)?;

    let patterns: Vec<String> = content
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.to_string())
        .collect();

    Ok(patterns)
}

/// Create .ckignore file with default content if it doesn't exist
pub fn create_ckignore_if_missing(repo_root: &Path) -> Result<bool> {
    let ckignore_path = repo_root.join(".ckignore");

    if ckignore_path.exists() {
        return Ok(false); // Already exists
    }

    std::fs::write(&ckignore_path, get_default_ckignore_content()).map_err(CkError::Io)?;

    Ok(true) // Created new file
}

/// Build exclusion patterns with proper priority ordering
///
/// This centralizes the pattern building logic used across CLI, TUI, and MCP interfaces
/// to prevent drift and ensure consistent behavior.
///
/// Builds exclusion patterns for file collection by combining command-line
/// excludes with default patterns. .ckignore files are now handled separately
/// by WalkBuilder's hierarchical ignore system.
///
/// Priority order:
/// 1. Additional excludes (from command-line or API calls)
/// 2. Default patterns (if use_defaults is true)
///
/// Note: .ckignore files are loaded hierarchically by WalkBuilder, not here.
///
/// # Arguments
/// * `additional_excludes` - Additional exclusion patterns (e.g., from CLI flags)
/// * `use_defaults` - Whether to include default exclusion patterns
///
/// # Returns
/// Combined list of exclusion patterns
pub fn build_exclude_patterns(additional_excludes: &[String], use_defaults: bool) -> Vec<String> {
    let mut patterns = Vec::new();

    // 1. Add additional exclude patterns (e.g., from command-line)
    patterns.extend(additional_excludes.iter().cloned());

    // 2. Add defaults (lowest priority)
    // Note: .ckignore files are now handled hierarchically by WalkBuilder
    if use_defaults {
        patterns.extend(get_default_exclude_patterns());
    }

    patterns
}

pub fn get_sidecar_path(repo_root: &Path, file_path: &Path) -> PathBuf {
    let relative = file_path.strip_prefix(repo_root).unwrap_or(file_path);
    let mut sidecar = repo_root.join(".ck");
    sidecar.push(relative);
    let ext = relative
        .extension()
        .map(|e| format!("{}.ck", e.to_string_lossy()))
        .unwrap_or_else(|| "ck".to_string());
    sidecar.set_extension(ext);
    sidecar
}

pub fn compute_file_hash(path: &Path) -> Result<String> {
    use std::io::Read;

    let mut file = std::fs::File::open(path)?;
    let mut hasher = blake3::Hasher::new();

    // Stream the file in 64KB chunks to avoid loading entire file into memory
    let mut buffer = [0u8; 65536]; // 64KB buffer
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let hash = hasher.finalize();
    Ok(hash.to_hex().to_string())
}

/// Compute blake3 hash of chunk content for incremental indexing
/// This enables us to detect which chunks have changed and only re-embed those
///
/// Hashes all fields that affect the chunk's display and meaning:
/// - text: the main chunk content
/// - leading_trivia: doc comments and comments before the chunk
/// - trailing_trivia: comments after the chunk
pub fn compute_chunk_hash(
    text: &str,
    leading_trivia: &[String],
    trailing_trivia: &[String],
) -> String {
    let mut hasher = blake3::Hasher::new();

    // Hash the main text
    hasher.update(text.as_bytes());

    // Hash leading trivia (doc comments, preceding comments)
    for trivia in leading_trivia {
        hasher.update(trivia.as_bytes());
    }

    // Hash trailing trivia (following comments)
    for trivia in trailing_trivia {
        hasher.update(trivia.as_bytes());
    }

    hasher.finalize().to_hex().to_string()
}

/// PDF-specific utilities
pub mod pdf {
    use std::path::{Path, PathBuf};

    /// Check if a file is a PDF by extension (optimized to avoid allocations)
    pub fn is_pdf_file(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("pdf")) // Avoids allocation vs to_lowercase()
            .unwrap_or(false)
    }

    /// Get path for cached PDF content
    pub fn get_content_cache_path(repo_root: &Path, file_path: &Path) -> PathBuf {
        let relative = file_path.strip_prefix(repo_root).unwrap_or(file_path);
        let mut cache_path = repo_root.join(".ck").join("content");
        cache_path.push(relative);

        // Add .txt extension to the cached file
        let ext = relative
            .extension()
            .map(|e| format!("{}.txt", e.to_string_lossy()))
            .unwrap_or_else(|| "txt".to_string());
        cache_path.set_extension(ext);

        cache_path
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::path::PathBuf;

        #[test]
        fn test_is_pdf_file() {
            assert!(is_pdf_file(&PathBuf::from("test.pdf")));
            assert!(is_pdf_file(&PathBuf::from("test.PDF"))); // Case insensitive
            assert!(is_pdf_file(&PathBuf::from("test.Pdf")));
            assert!(!is_pdf_file(&PathBuf::from("test.txt")));
            assert!(!is_pdf_file(&PathBuf::from("test"))); // No extension
            assert!(!is_pdf_file(&PathBuf::from("pdf"))); // Just "pdf", no extension
        }

        #[test]
        fn test_get_content_cache_path() {
            let repo_root = PathBuf::from("/project");
            let file_path = PathBuf::from("/project/docs/manual.pdf");

            let cache_path = get_content_cache_path(&repo_root, &file_path);
            assert_eq!(
                cache_path,
                PathBuf::from("/project/.ck/content/docs/manual.pdf.txt")
            );
        }

        #[test]
        fn test_get_content_cache_path_no_extension() {
            let repo_root = PathBuf::from("/project");
            let file_path = PathBuf::from("/project/docs/manual");

            let cache_path = get_content_cache_path(&repo_root, &file_path);
            assert_eq!(
                cache_path,
                PathBuf::from("/project/.ck/content/docs/manual.txt")
            );
        }

        #[test]
        fn test_get_content_cache_path_relative() {
            let repo_root = PathBuf::from("/project");
            let file_path = PathBuf::from("docs/manual.pdf"); // Relative path

            let cache_path = get_content_cache_path(&repo_root, &file_path);
            assert_eq!(
                cache_path,
                PathBuf::from("/project/.ck/content/docs/manual.pdf.txt")
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_span_valid_creation() {
        // Test valid span creation
        let span = Span::new(0, 10, 1, 2).unwrap();
        assert_eq!(span.byte_start, 0);
        assert_eq!(span.byte_end, 10);
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 2);
        assert!(span.is_valid());
    }

    #[test]
    fn test_span_validation_valid_cases() {
        // Same byte positions (empty span)
        let span = Span::new(10, 10, 1, 1).unwrap();
        assert!(span.is_valid());
        assert_eq!(span.byte_len(), 0);
        assert_eq!(span.line_count(), 1);

        // Multi-line span
        let span = Span::new(0, 100, 1, 10).unwrap();
        assert!(span.is_valid());
        assert_eq!(span.byte_len(), 100);
        assert_eq!(span.line_count(), 10);

        // Single line span
        let span = Span::new(5, 25, 3, 3).unwrap();
        assert!(span.is_valid());
        assert_eq!(span.byte_len(), 20);
        assert_eq!(span.line_count(), 1);
    }

    #[test]
    fn test_span_validation_invalid_byte_range() {
        // Reversed byte range
        let result = Span::new(10, 5, 1, 2);
        assert!(result.is_err());
        if let Err(CkError::SpanValidation(msg)) = result {
            assert!(msg.contains("Invalid byte range"));
            assert!(msg.contains("start (10) > end (5)"));
        } else {
            panic!("Expected SpanValidation error");
        }
    }

    #[test]
    fn test_span_validation_invalid_line_range() {
        // Reversed line range
        let result = Span::new(0, 10, 5, 2);
        assert!(result.is_err());
        if let Err(CkError::SpanValidation(msg)) = result {
            assert!(msg.contains("Invalid line range"));
            assert!(msg.contains("start (5) > end (2)"));
        } else {
            panic!("Expected SpanValidation error");
        }
    }

    #[test]
    fn test_span_validation_zero_line_numbers() {
        // Zero line start
        let result = Span::new(0, 10, 0, 2);
        assert!(result.is_err());
        if let Err(CkError::SpanValidation(msg)) = result {
            assert!(msg.contains("Line start cannot be zero"));
        } else {
            panic!("Expected SpanValidation error");
        }

        // Zero line end
        let result = Span::new(0, 10, 1, 0);
        assert!(result.is_err());
        if let Err(CkError::SpanValidation(msg)) = result {
            assert!(msg.contains("Line end cannot be zero"));
        } else {
            panic!("Expected SpanValidation error");
        }
    }

    #[test]
    fn test_span_unchecked_creation() {
        // Test backward compatibility with unchecked creation
        let span = Span::new_unchecked(10, 5, 0, 1);
        assert_eq!(span.byte_start, 10);
        assert_eq!(span.byte_end, 5);
        assert_eq!(span.line_start, 0);
        assert_eq!(span.line_end, 1);
        assert!(!span.is_valid()); // Should be invalid
    }

    #[test]
    fn test_span_validation_methods() {
        // Valid span
        let valid_span = Span::new_unchecked(0, 10, 1, 2);
        assert!(valid_span.validate().is_ok());
        assert!(valid_span.is_valid());

        // Invalid span (reversed bytes)
        let invalid_span = Span::new_unchecked(10, 5, 1, 2);
        assert!(invalid_span.validate().is_err());
        assert!(!invalid_span.is_valid());

        // Invalid span (zero lines)
        let zero_line_span = Span::new_unchecked(0, 10, 0, 1);
        assert!(zero_line_span.validate().is_err());
        assert!(!zero_line_span.is_valid());
    }

    #[test]
    fn test_span_utility_methods() {
        let span = Span::new(10, 25, 5, 8).unwrap();

        // Test byte_len
        assert_eq!(span.byte_len(), 15);

        // Test line_count
        assert_eq!(span.line_count(), 4); // lines 5, 6, 7, 8

        // Test with single-line span
        let single_line = Span::new(0, 5, 1, 1).unwrap();
        assert_eq!(single_line.line_count(), 1);
        assert_eq!(single_line.byte_len(), 5);

        // Test with empty span
        let empty = Span::new(10, 10, 3, 3).unwrap();
        assert_eq!(empty.byte_len(), 0);
        assert_eq!(empty.line_count(), 1);
    }

    #[test]
    fn test_span_legacy_struct_literal_still_works() {
        // Ensure backward compatibility for existing code using struct literals
        let span = Span {
            byte_start: 0,
            byte_end: 10,
            line_start: 1,
            line_end: 2,
        };

        assert_eq!(span.byte_start, 0);
        assert_eq!(span.byte_end, 10);
        assert_eq!(span.line_start, 1);
        assert_eq!(span.line_end, 2);
        assert!(span.is_valid());
    }

    #[test]
    fn test_search_options_default() {
        let options = SearchOptions::default();
        assert!(matches!(options.mode, SearchMode::Regex));
        assert_eq!(options.query, "");
        assert_eq!(options.path, PathBuf::from("."));
        assert_eq!(options.top_k, None);
        assert_eq!(options.threshold, None);
        assert!(!options.case_insensitive);
        assert!(!options.whole_word);
        assert!(!options.fixed_string);
        assert!(!options.line_numbers);
        assert_eq!(options.context_lines, 0);
        assert!(options.recursive);
        assert!(!options.json_output);
        assert!(!options.reindex);
        assert!(!options.show_scores);
        assert!(!options.show_filenames);
    }

    #[test]
    fn test_file_metadata_serialization() {
        let metadata = FileMetadata {
            path: PathBuf::from("test.txt"),
            hash: "abc123".to_string(),
            last_modified: 1234567890,
            size: 1024,
        };

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: FileMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(metadata.path, deserialized.path);
        assert_eq!(metadata.hash, deserialized.hash);
        assert_eq!(metadata.last_modified, deserialized.last_modified);
        assert_eq!(metadata.size, deserialized.size);
    }

    #[test]
    fn test_search_result_serialization() {
        let result = SearchResult {
            file: PathBuf::from("test.txt"),
            span: Span {
                byte_start: 0,
                byte_end: 10,
                line_start: 1,
                line_end: 1,
            },
            score: 0.95,
            preview: "hello world".to_string(),
            lang: Some(Language::Rust),
            symbol: Some("main".to_string()),
            chunk_hash: Some("abc123".to_string()),
            index_epoch: Some(1699123456),
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SearchResult = serde_json::from_str(&json).unwrap();

        assert_eq!(result.file, deserialized.file);
        assert_eq!(result.score, deserialized.score);
        assert_eq!(result.preview, deserialized.preview);
        assert_eq!(result.lang, deserialized.lang);
        assert_eq!(result.symbol, deserialized.symbol);
        assert_eq!(result.chunk_hash, deserialized.chunk_hash);
        assert_eq!(result.index_epoch, deserialized.index_epoch);
    }

    #[test]
    fn test_jsonl_search_result_conversion() {
        let result = SearchResult {
            file: PathBuf::from("src/auth.rs"),
            span: Span {
                byte_start: 1203,
                byte_end: 1456,
                line_start: 42,
                line_end: 58,
            },
            score: 0.89,
            preview: "function authenticate(user) {...}".to_string(),
            lang: Some(Language::Rust),
            symbol: Some("authenticate".to_string()),
            chunk_hash: Some("abc123def456".to_string()),
            index_epoch: Some(1699123456),
        };

        // Test with snippet
        let jsonl_with_snippet = JsonlSearchResult::from_search_result(&result, true);
        assert_eq!(jsonl_with_snippet.path, "src/auth.rs");
        assert_eq!(jsonl_with_snippet.span.line_start, 42);
        assert_eq!(jsonl_with_snippet.language, Some("rust".to_string()));
        assert_eq!(
            jsonl_with_snippet.snippet,
            Some("function authenticate(user) {...}".to_string())
        );
        assert_eq!(jsonl_with_snippet.score, Some(0.89));
        assert_eq!(
            jsonl_with_snippet.chunk_hash,
            Some("abc123def456".to_string())
        );
        assert_eq!(jsonl_with_snippet.index_epoch, Some(1699123456));

        // Test without snippet
        let jsonl_no_snippet = JsonlSearchResult::from_search_result(&result, false);
        assert_eq!(jsonl_no_snippet.snippet, None);
        assert_eq!(jsonl_no_snippet.path, "src/auth.rs");
    }

    #[test]
    fn test_get_sidecar_path() {
        let repo_root = PathBuf::from("/home/user/project");
        let file_path = PathBuf::from("/home/user/project/src/main.rs");

        let sidecar = get_sidecar_path(&repo_root, &file_path);
        let expected = PathBuf::from("/home/user/project/.ck/src/main.rs.ck");

        assert_eq!(sidecar, expected);
    }

    #[test]
    fn test_get_sidecar_path_no_extension() {
        let repo_root = PathBuf::from("/project");
        let file_path = PathBuf::from("/project/README");

        let sidecar = get_sidecar_path(&repo_root, &file_path);
        let expected = PathBuf::from("/project/.ck/README.ck");

        assert_eq!(sidecar, expected);
    }

    #[test]
    fn test_compute_file_hash() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        fs::write(&file_path, "hello world").unwrap();

        let hash1 = compute_file_hash(&file_path).unwrap();
        let hash2 = compute_file_hash(&file_path).unwrap();

        // Same content should produce same hash
        assert_eq!(hash1, hash2);
        assert!(!hash1.is_empty());

        // Different content should produce different hash
        fs::write(&file_path, "hello rust").unwrap();
        let hash3 = compute_file_hash(&file_path).unwrap();
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_compute_file_hash_nonexistent() {
        let result = compute_file_hash(&PathBuf::from("nonexistent.txt"));
        assert!(result.is_err());
    }

    #[test]
    fn test_compute_file_hash_large_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large_test.txt");

        // Create a file larger than the buffer size (64KB) to test streaming
        let large_content = "a".repeat(100_000); // 100KB content
        fs::write(&file_path, &large_content).unwrap();

        let hash1 = compute_file_hash(&file_path).unwrap();
        let hash2 = compute_file_hash(&file_path).unwrap();

        // Streaming hash should be consistent
        assert_eq!(hash1, hash2);
        assert!(!hash1.is_empty());

        // Verify it's different from smaller content
        fs::write(&file_path, "small content").unwrap();
        let hash3 = compute_file_hash(&file_path).unwrap();
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_json_search_result_serialization() {
        let signals = SearchSignals {
            lex_rank: Some(1),
            vec_rank: Some(2),
            rrf_score: 0.85,
        };

        let result = JsonSearchResult {
            file: "test.txt".to_string(),
            span: Span {
                byte_start: 0,
                byte_end: 5,
                line_start: 1,
                line_end: 1,
            },
            lang: None, // txt is not a supported language
            symbol: None,
            score: 0.95,
            signals,
            preview: "hello".to_string(),
            model: "bge-small".to_string(),
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: JsonSearchResult = serde_json::from_str(&json).unwrap();

        assert_eq!(result.file, deserialized.file);
        assert_eq!(result.score, deserialized.score);
        assert_eq!(result.signals.rrf_score, deserialized.signals.rrf_score);
        assert_eq!(result.model, deserialized.model);
    }

    #[test]
    fn test_language_from_extension() {
        assert_eq!(Language::from_extension("rs"), Some(Language::Rust));
        assert_eq!(Language::from_extension("py"), Some(Language::Python));
        assert_eq!(Language::from_extension("js"), Some(Language::JavaScript));
        assert_eq!(Language::from_extension("ts"), Some(Language::TypeScript));
        assert_eq!(Language::from_extension("tsx"), Some(Language::TypeScript));
        assert_eq!(Language::from_extension("hs"), Some(Language::Haskell));
        assert_eq!(Language::from_extension("lhs"), Some(Language::Haskell));
        assert_eq!(Language::from_extension("go"), Some(Language::Go));
        assert_eq!(Language::from_extension("java"), Some(Language::Java));
        assert_eq!(Language::from_extension("c"), Some(Language::C));
        assert_eq!(Language::from_extension("cpp"), Some(Language::Cpp));
        assert_eq!(Language::from_extension("cs"), Some(Language::CSharp));
        assert_eq!(Language::from_extension("rb"), Some(Language::Ruby));
        assert_eq!(Language::from_extension("php"), Some(Language::Php));
        assert_eq!(Language::from_extension("swift"), Some(Language::Swift));
        assert_eq!(Language::from_extension("kt"), Some(Language::Kotlin));
        assert_eq!(Language::from_extension("kts"), Some(Language::Kotlin));
        assert_eq!(Language::from_extension("unknown"), None);
    }

    #[test]
    fn test_language_from_extension_case_insensitive() {
        // Test uppercase extensions - only for actually supported languages
        assert_eq!(Language::from_extension("RS"), Some(Language::Rust));
        assert_eq!(Language::from_extension("PY"), Some(Language::Python));
        assert_eq!(Language::from_extension("JS"), Some(Language::JavaScript));
        assert_eq!(Language::from_extension("TS"), Some(Language::TypeScript));
        assert_eq!(Language::from_extension("TSX"), Some(Language::TypeScript));
        assert_eq!(Language::from_extension("HS"), Some(Language::Haskell));
        assert_eq!(Language::from_extension("LHS"), Some(Language::Haskell));
        assert_eq!(Language::from_extension("GO"), Some(Language::Go));
        assert_eq!(Language::from_extension("JAVA"), Some(Language::Java));
        assert_eq!(Language::from_extension("C"), Some(Language::C));
        assert_eq!(Language::from_extension("CPP"), Some(Language::Cpp));
        assert_eq!(Language::from_extension("CC"), Some(Language::Cpp));
        assert_eq!(Language::from_extension("CXX"), Some(Language::Cpp));
        assert_eq!(Language::from_extension("H"), Some(Language::Cpp));
        assert_eq!(Language::from_extension("HPP"), Some(Language::Cpp));
        assert_eq!(Language::from_extension("CS"), Some(Language::CSharp));
        assert_eq!(Language::from_extension("RB"), Some(Language::Ruby));
        assert_eq!(Language::from_extension("PHP"), Some(Language::Php));
        assert_eq!(Language::from_extension("SWIFT"), Some(Language::Swift));
        assert_eq!(Language::from_extension("KT"), Some(Language::Kotlin));
        assert_eq!(Language::from_extension("KTS"), Some(Language::Kotlin));
        assert_eq!(Language::from_extension("PDF"), Some(Language::Pdf));

        // Test mixed case extensions
        assert_eq!(Language::from_extension("Rs"), Some(Language::Rust));
        assert_eq!(Language::from_extension("Py"), Some(Language::Python));
        assert_eq!(Language::from_extension("Js"), Some(Language::JavaScript));
        assert_eq!(Language::from_extension("Ts"), Some(Language::TypeScript));
        assert_eq!(Language::from_extension("TsX"), Some(Language::TypeScript));
        assert_eq!(Language::from_extension("Hs"), Some(Language::Haskell));
        assert_eq!(Language::from_extension("Go"), Some(Language::Go));
        assert_eq!(Language::from_extension("Java"), Some(Language::Java));
        assert_eq!(Language::from_extension("Cpp"), Some(Language::Cpp));
        assert_eq!(Language::from_extension("Rb"), Some(Language::Ruby));
        assert_eq!(Language::from_extension("Php"), Some(Language::Php));
        assert_eq!(Language::from_extension("Swift"), Some(Language::Swift));
        assert_eq!(Language::from_extension("Kt"), Some(Language::Kotlin));
        assert_eq!(Language::from_extension("Pdf"), Some(Language::Pdf));

        // Unknown extensions should still return None
        assert_eq!(Language::from_extension("UNKNOWN"), None);
        assert_eq!(Language::from_extension("Unknown"), None);
    }

    #[test]
    fn test_language_from_path() {
        assert_eq!(
            Language::from_path(&PathBuf::from("test.rs")),
            Some(Language::Rust)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("test.py")),
            Some(Language::Python)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("test.js")),
            Some(Language::JavaScript)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("test.hs")),
            Some(Language::Haskell)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("test.lhs")),
            Some(Language::Haskell)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("test.go")),
            Some(Language::Go)
        );
        assert_eq!(Language::from_path(&PathBuf::from("test.unknown")), None); // unknown extensions return None
        assert_eq!(Language::from_path(&PathBuf::from("noext")), None); // no extension
    }

    #[test]
    fn test_language_from_path_case_insensitive() {
        // Test uppercase extensions in file paths - only supported languages
        assert_eq!(
            Language::from_path(&PathBuf::from("MAIN.RS")),
            Some(Language::Rust)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("app.PY")),
            Some(Language::Python)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("script.JS")),
            Some(Language::JavaScript)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("types.TS")),
            Some(Language::TypeScript)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("Component.TSX")),
            Some(Language::TypeScript)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("module.HS")),
            Some(Language::Haskell)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("server.GO")),
            Some(Language::Go)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("App.JAVA")),
            Some(Language::Java)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("main.C")),
            Some(Language::C)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("utils.CPP")),
            Some(Language::Cpp)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("Program.CS")),
            Some(Language::CSharp)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("script.RB")),
            Some(Language::Ruby)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("index.PHP")),
            Some(Language::Php)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("App.SWIFT")),
            Some(Language::Swift)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("Main.KT")),
            Some(Language::Kotlin)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("document.PDF")),
            Some(Language::Pdf)
        );

        // Test mixed case extensions in file paths
        assert_eq!(
            Language::from_path(&PathBuf::from("config.Rs")),
            Some(Language::Rust)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("helper.Py")),
            Some(Language::Python)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("utils.Js")),
            Some(Language::JavaScript)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("interfaces.Ts")),
            Some(Language::TypeScript)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("Component.TsX")),
            Some(Language::TypeScript)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("main.Cpp")),
            Some(Language::Cpp)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("report.Pdf")),
            Some(Language::Pdf)
        );

        // Unknown extensions should still return None regardless of case
        assert_eq!(Language::from_path(&PathBuf::from("test.UNKNOWN")), None);
        assert_eq!(Language::from_path(&PathBuf::from("test.Unknown")), None);
    }

    #[test]
    fn test_language_display() {
        assert_eq!(Language::Rust.to_string(), "rust");
        assert_eq!(Language::Python.to_string(), "python");
        assert_eq!(Language::JavaScript.to_string(), "javascript");
        assert_eq!(Language::TypeScript.to_string(), "typescript");
        assert_eq!(Language::Go.to_string(), "go");
        assert_eq!(Language::Java.to_string(), "java");
    }

    #[test]
    fn test_create_ckignore_if_missing() {
        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path();

        // First creation should succeed
        let created = create_ckignore_if_missing(test_path).unwrap();
        assert!(created);

        // Check that file exists
        let ckignore_path = test_path.join(".ckignore");
        assert!(ckignore_path.exists());

        // Check content contains expected patterns
        let content = fs::read_to_string(&ckignore_path).unwrap();
        assert!(content.contains("*.png"));
        assert!(content.contains("*.json"));
        assert!(content.contains("*.yaml"));
        assert!(content.contains("# Images"));
        assert!(content.contains("# Config formats"));

        // Second creation should return false (already exists)
        let created_again = create_ckignore_if_missing(test_path).unwrap();
        assert!(!created_again);
    }

    #[test]
    fn test_read_ckignore_patterns() {
        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path();

        // Test with no .ckignore file
        let patterns = read_ckignore_patterns(test_path).unwrap();
        assert_eq!(patterns.len(), 0);

        // Create a .ckignore file
        let ckignore_path = test_path.join(".ckignore");
        fs::write(
            &ckignore_path,
            r#"# Comment line
*.png
*.jpg

# Another comment
*.json
*.yaml
"#,
        )
        .unwrap();

        // Read patterns
        let patterns = read_ckignore_patterns(test_path).unwrap();
        assert_eq!(patterns.len(), 4);
        assert!(patterns.contains(&"*.png".to_string()));
        assert!(patterns.contains(&"*.jpg".to_string()));
        assert!(patterns.contains(&"*.json".to_string()));
        assert!(patterns.contains(&"*.yaml".to_string()));
        // Comments should be filtered out
        assert!(!patterns.iter().any(|p| p.starts_with('#')));
    }

    #[test]
    fn test_read_ckignore_patterns_with_empty_lines() {
        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path();

        let ckignore_path = test_path.join(".ckignore");
        fs::write(
            &ckignore_path,
            r#"
*.png

*.jpg


*.json
"#,
        )
        .unwrap();

        let patterns = read_ckignore_patterns(test_path).unwrap();
        assert_eq!(patterns.len(), 3);
        assert!(patterns.contains(&"*.png".to_string()));
        assert!(patterns.contains(&"*.jpg".to_string()));
        assert!(patterns.contains(&"*.json".to_string()));
    }

    #[test]
    fn test_get_default_ckignore_content() {
        let content = get_default_ckignore_content();

        // Check that default content includes key patterns
        assert!(content.contains("*.png"));
        assert!(content.contains("*.jpg"));
        assert!(content.contains("*.mp4"));
        assert!(content.contains("*.mp3"));
        assert!(content.contains("*.exe"));
        assert!(content.contains("*.zip"));
        assert!(content.contains("*.db"));
        assert!(content.contains("*.json"));
        assert!(content.contains("*.yaml"));

        // Check that it has comments
        assert!(content.contains("# Images"));
        assert!(content.contains("# Video"));
        assert!(content.contains("# Audio"));
        assert!(content.contains("# Config formats"));

        // Check for issue reference
        assert!(content.contains("issue #27"));
    }
}
