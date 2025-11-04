use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::info;

use ck_core::{SearchOptions, get_default_exclude_patterns};

use super::McpResult;
use super::cache::StatsCache;
use super::session::SessionManager;

/// Shared context for the MCP server managing resources and configuration
#[derive(Clone)]
pub struct McpContext {
    pub cwd: PathBuf,
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
            rerank: false,
            rerank_model: None,
            embedding_model: None,
        };

        Ok(Self {
            cwd,
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
