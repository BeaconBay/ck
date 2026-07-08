use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::info;

use ck_core::{SearchOptions, get_default_exclude_patterns};
use rmcp::ErrorData;

use super::McpResult;
use super::cache::StatsCache;
use super::session::SessionManager;

/// Environment variable that lets an operator extend the MCP sandbox
/// to additional roots (colon-separated, like `PATH`). Each entry must
/// resolve to an existing directory or it's ignored with a warning.
pub const ALLOWED_ROOTS_ENV: &str = "CK_MCP_ALLOWED_ROOTS";

/// Shared context for the MCP server managing resources and configuration
#[derive(Clone)]
pub struct McpContext {
    pub cwd: PathBuf,
    /// Canonical filesystem roots the MCP server is permitted to read.
    /// Every tool handler must route incoming `request.path` through
    /// [`McpContext::resolve_request_path`] which enforces containment.
    pub allowed_roots: Vec<PathBuf>,
    pub stats_cache: StatsCache,
    pub session_manager: SessionManager,
    #[allow(dead_code)]
    pub index_locks: Arc<RwLock<HashMap<PathBuf, Arc<Mutex<()>>>>>,
    #[allow(dead_code)]
    pub operation_tokens: Arc<RwLock<HashMap<String, tokio_util::sync::CancellationToken>>>,
    #[allow(dead_code)]
    pub default_search_options: SearchOptions,
}

impl McpContext {
    pub fn new(cwd: PathBuf) -> McpResult<Self> {
        info!("Initializing MCP context for directory: {}", cwd.display());

        // Sandbox roots: always include the canonical cwd, optionally
        // extend via CK_MCP_ALLOWED_ROOTS. Canonicalize so symlink
        // tricks can't smuggle a path through containment checks.
        let cwd_canonical = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
        let mut allowed_roots: Vec<PathBuf> = vec![cwd_canonical.clone()];
        if let Ok(extra) = std::env::var(ALLOWED_ROOTS_ENV) {
            for entry in extra.split(':').filter(|s| !s.is_empty()) {
                match PathBuf::from(entry).canonicalize() {
                    Ok(root) if root.is_dir() => {
                        if !allowed_roots.iter().any(|r| r == &root) {
                            info!("MCP sandbox root added from env: {}", root.display());
                            allowed_roots.push(root);
                        }
                    }
                    Ok(root) => tracing::warn!(
                        "{ALLOWED_ROOTS_ENV} entry is not a directory, ignoring: {}",
                        root.display()
                    ),
                    Err(e) => tracing::warn!(
                        "{ALLOWED_ROOTS_ENV} entry could not be resolved, ignoring: {entry} ({e})"
                    ),
                }
            }
        }

        let default_search_options = SearchOptions {
            mode: ck_core::SearchMode::Semantic,
            query: String::new(),
            path: cwd.clone(),
            top_k: Some(10),
            threshold: Some(0.6),
            case_insensitive: false,
            whole_word: false,
            fixed_string: false,
            line_numbers: false,
            context_lines: 0,
            before_context_lines: 0,
            after_context_lines: 0,
            recursive: true,
            json_output: false,
            jsonl_output: true, // Default to JSONL for agent consumption
            no_snippet: false,
            reindex: false,
            show_scores: true,
            show_filenames: true,
            files_with_matches: false,
            files_without_matches: false,
            exclude_patterns: get_default_exclude_patterns(),
            include_patterns: Vec::new(),
            respect_gitignore: true,
            use_ckignore: true,
            full_section: false,
            hidden: false,
            rerank: false,
            rerank_model: None,
            embedding_model: None,
        };

        Ok(Self {
            cwd: cwd_canonical,
            allowed_roots,
            stats_cache: StatsCache::default(), // 30-second TTL for MCP responsiveness
            session_manager: SessionManager::default(), // 5-minute TTL for search sessions
            #[allow(dead_code)]
            index_locks: Arc::new(RwLock::new(HashMap::new())),
            #[allow(dead_code)]
            operation_tokens: Arc::new(RwLock::new(HashMap::new())),
            #[allow(dead_code)]
            default_search_options,
        })
    }

    /// Resolve a `request.path` string into a canonical PathBuf, rejecting
    /// anything that escapes the MCP sandbox.
    ///
    /// Containment is checked against the canonical form, so `..` traversal,
    /// symlinks pointing outside, and absolute escape paths are all rejected.
    /// The path must exist — non-existent paths return `invalid_params`.
    pub fn resolve_request_path(&self, raw: &str) -> Result<PathBuf, ErrorData> {
        if raw.is_empty() {
            return Err(ErrorData::invalid_params(
                "path must not be empty".to_string(),
                None,
            ));
        }
        let requested = if Path::new(raw).is_absolute() {
            PathBuf::from(raw)
        } else {
            self.cwd.join(raw)
        };
        if !requested.exists() {
            return Err(ErrorData::invalid_params(
                format!("path does not exist: {raw}"),
                None,
            ));
        }
        let canonical = requested.canonicalize().map_err(|e| {
            ErrorData::invalid_params(format!("could not resolve path '{raw}': {e}"), None)
        })?;
        if !self
            .allowed_roots
            .iter()
            .any(|root| canonical == *root || canonical.starts_with(root))
        {
            return Err(ErrorData::invalid_params(
                format!(
                    "path is outside the MCP allowed roots (set {ALLOWED_ROOTS_ENV} to extend): {raw}"
                ),
                None,
            ));
        }
        Ok(canonical)
    }

    /// Get or create an index lock for the specified directory
    #[allow(dead_code)]
    pub async fn get_index_lock(&self, path: &PathBuf) -> Arc<Mutex<()>> {
        let locks = self.index_locks.read().await;
        if let Some(lock) = locks.get(path) {
            return lock.clone();
        }
        drop(locks);

        let new_lock = Arc::new(Mutex::new(()));
        let mut locks = self.index_locks.write().await;
        locks.insert(path.clone(), new_lock.clone());
        new_lock
    }

    /// Register a cancellation token for an operation
    #[allow(dead_code)]
    pub async fn register_operation(
        &self,
        operation_id: String,
    ) -> tokio_util::sync::CancellationToken {
        let token = tokio_util::sync::CancellationToken::new();
        let mut tokens = self.operation_tokens.write().await;
        tokens.insert(operation_id, token.clone());
        token
    }

    /// Cancel an operation by ID
    #[allow(dead_code)]
    pub async fn cancel_operation(&self, operation_id: &str) -> bool {
        let mut tokens = self.operation_tokens.write().await;
        if let Some(token) = tokens.remove(operation_id) {
            token.cancel();
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod sandbox_tests {
    use super::McpContext;
    use std::fs;
    use tempfile::TempDir;

    fn ctx(root: &std::path::Path) -> McpContext {
        McpContext::new(root.to_path_buf()).expect("context")
    }

    #[test]
    fn accepts_existing_path_inside_root() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("sub");
        fs::create_dir(&sub).unwrap();
        let f = sub.join("a.txt");
        fs::write(&f, "x").unwrap();

        let c = ctx(tmp.path());
        let resolved = c
            .resolve_request_path(f.to_str().unwrap())
            .expect("inside root must succeed");
        assert_eq!(resolved.canonicalize().unwrap(), f.canonicalize().unwrap(),);
    }

    #[test]
    fn accepts_relative_path_inside_root() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("sub");
        fs::create_dir(&sub).unwrap();
        let c = ctx(tmp.path());
        let resolved = c
            .resolve_request_path("sub")
            .expect("relative path inside root");
        assert!(resolved.starts_with(tmp.path().canonicalize().unwrap()));
    }

    #[test]
    fn rejects_absolute_path_outside_root() {
        let tmp_root = TempDir::new().unwrap();
        let tmp_outside = TempDir::new().unwrap();
        let outside_file = tmp_outside.path().join("secret.txt");
        fs::write(&outside_file, "top-secret").unwrap();

        let c = ctx(tmp_root.path());
        let err = c
            .resolve_request_path(outside_file.to_str().unwrap())
            .expect_err("absolute path outside root must be rejected");
        assert!(format!("{err:?}").contains("outside the MCP allowed roots"));
    }

    #[test]
    fn rejects_dot_dot_escape() {
        let tmp_root = TempDir::new().unwrap();
        let sub = tmp_root.path().join("sub");
        fs::create_dir(&sub).unwrap();

        // Build a relative path that climbs out via ..
        // sub/../../  → lands above the sandbox root.
        let c = McpContext::new(sub.clone()).expect("ctx");
        let parent_of_root = tmp_root.path().parent().expect("tmp has parent");
        // resolve_request_path joins relative against cwd (sub), so
        // "../../" from sub lands in tmp_root's parent, outside the
        // sandbox. The path must exist for the function to even reach
        // the containment check, so use parent_of_root directly which
        // is the tmpdir base and definitely exists.
        let _ = parent_of_root; // (kept for readability)
        let err = c
            .resolve_request_path("../../")
            .expect_err(".. escape must be rejected");
        assert!(format!("{err:?}").contains("outside the MCP allowed roots"));
    }

    #[test]
    fn rejects_symlink_pointing_outside_root() {
        // Symlinks should be resolved by canonicalize() and then fail
        // the containment check. Skip on platforms where symlinks need
        // privileges (Windows) — this test runs on Unix CI.
        #[cfg(unix)]
        {
            let tmp_root = TempDir::new().unwrap();
            let tmp_outside = TempDir::new().unwrap();
            let outside_file = tmp_outside.path().join("secret.txt");
            fs::write(&outside_file, "top-secret").unwrap();

            let link = tmp_root.path().join("escape");
            std::os::unix::fs::symlink(&outside_file, &link).unwrap();

            let c = ctx(tmp_root.path());
            let err = c
                .resolve_request_path(link.to_str().unwrap())
                .expect_err("symlink pointing outside must be rejected");
            assert!(format!("{err:?}").contains("outside the MCP allowed roots"));
        }
    }

    #[test]
    fn rejects_empty_path() {
        let tmp = TempDir::new().unwrap();
        let c = ctx(tmp.path());
        let err = c
            .resolve_request_path("")
            .expect_err("empty path must be rejected");
        assert!(format!("{err:?}").contains("must not be empty"));
    }

    #[test]
    fn rejects_nonexistent_path() {
        let tmp = TempDir::new().unwrap();
        let c = ctx(tmp.path());
        let err = c
            .resolve_request_path("definitely-does-not-exist")
            .expect_err("nonexistent path must be rejected");
        assert!(format!("{err:?}").contains("does not exist"));
    }
}
