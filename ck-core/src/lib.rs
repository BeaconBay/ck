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
}

impl Language {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
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
    pub respect_gitignore: bool,
    pub full_section: bool,
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
            respect_gitignore: true,
            full_section: false,
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
    let data = std::fs::read(path)?;
    let hash = blake3::hash(&data);
    Ok(hash.to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_span_creation() {
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
    fn test_language_display() {
        assert_eq!(Language::Rust.to_string(), "rust");
        assert_eq!(Language::Python.to_string(), "python");
        assert_eq!(Language::JavaScript.to_string(), "javascript");
        assert_eq!(Language::TypeScript.to_string(), "typescript");
        assert_eq!(Language::Go.to_string(), "go");
        assert_eq!(Language::Java.to_string(), "java");
    }
}
