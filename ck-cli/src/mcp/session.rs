use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::debug;
use uuid::Uuid;

use ck_core::{SearchOptions, SearchResult};

/// Default session TTL in seconds (5 minutes)
const DEFAULT_SESSION_TTL: u64 = 300;

/// Maximum number of concurrent sessions
const MAX_SESSIONS: usize = 100;

/// Default page size for pagination
const DEFAULT_PAGE_SIZE: usize = 50;

/// Maximum page size to prevent excessive memory usage
const MAX_PAGE_SIZE: usize = 200;

/// Maximum snippet length to prevent excessive response size
const MAX_SNIPPET_LENGTH: usize = 2000;

/// Default snippet length
const DEFAULT_SNIPPET_LENGTH: usize = 500;

/// SearchSession stores cached search results for pagination
#[derive(Debug, Clone)]
pub struct SearchSession {
    #[allow(dead_code)]
    pub id: Uuid,
    #[allow(dead_code)]
    pub search_options: SearchOptions,
    pub results: Vec<SearchResult>,
    #[allow(dead_code)]
    pub created_at: SystemTime,
    pub last_accessed: SystemTime,
    pub total_count: usize,
    #[allow(dead_code)]
    pub search_completed: bool,
    pub search_params_hash: String,
}

/// Cursor for pagination - opaque to clients
#[derive(Debug, Serialize, Deserialize)]
pub struct PaginationCursor {
    pub session_id: Uuid,
    pub offset: usize,
    pub search_params_hash: String,
    pub timestamp: u64,
    pub version: u32,
    #[serde(default = "default_page_size")]
    pub original_page_size: usize,
}

fn default_page_size() -> usize {
    50 // Default page size for backward compatibility
}

/// Page of search results
#[derive(Debug)]
pub struct SearchPage {
    pub matches: Vec<SearchResult>,
    pub count: usize,
    pub total_count: Option<usize>,
    pub has_more: bool,
    pub truncated: bool,
    pub next_cursor: Option<String>,
    pub current_page: usize,
    pub original_page_size: usize,
}

/// Configuration for pagination
#[derive(Debug, Clone)]
pub struct PaginationConfig {
    pub page_size: usize,
    pub include_snippet: bool,
    pub snippet_length: usize,
    pub context_lines: usize,
}

impl Default for PaginationConfig {
    fn default() -> Self {
        Self {
            page_size: DEFAULT_PAGE_SIZE,
            include_snippet: true,
            snippet_length: DEFAULT_SNIPPET_LENGTH,
            context_lines: 0,
        }
    }
}

impl PaginationConfig {
    /// Validate and clamp configuration values
    pub fn validate(mut self) -> Self {
        self.page_size = self.page_size.clamp(1, MAX_PAGE_SIZE);
        self.snippet_length = self.snippet_length.min(MAX_SNIPPET_LENGTH);
        self.context_lines = self.context_lines.min(10);
        self
    }
}

/// SessionManager handles search session lifecycle and pagination
#[derive(Clone)]
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<Uuid, SearchSession>>>,
    session_ttl: u64,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new(DEFAULT_SESSION_TTL)
    }
}

impl SessionManager {
    /// Create a new SessionManager
    pub fn new(session_ttl: u64) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            session_ttl,
        }
    }

    /// Create a new search session with results
    pub async fn create_session(
        &self,
        search_options: SearchOptions,
        results: Vec<SearchResult>,
    ) -> Result<Uuid, String> {
        let session_id = Uuid::new_v4();
        let now = SystemTime::now();
        let search_params_hash = Self::hash_search_options(&search_options);

        let session = SearchSession {
            id: session_id,
            search_options,
            total_count: results.len(),
            results,
            created_at: now,
            last_accessed: now,
            search_completed: true,
            search_params_hash,
        };

        let mut sessions = self.sessions.write().await;

        // Check if we need to evict old sessions
        if sessions.len() >= MAX_SESSIONS {
            self.evict_oldest_session(&mut sessions).await;
        }

        sessions.insert(session_id, session);
        debug!(
            "Created search session {} with {} results",
            session_id,
            sessions.get(&session_id).unwrap().total_count
        );

        Ok(session_id)
    }

    /// Get a page of results from a session
    pub async fn get_page(
        &self,
        session_id: Uuid,
        offset: usize,
        config: PaginationConfig,
    ) -> Result<SearchPage, String> {
        let mut sessions = self.sessions.write().await;

        let session = sessions
            .get_mut(&session_id)
            .ok_or_else(|| "Session not found or expired".to_string())?;

        // Check if session has expired
        if self.is_session_expired(session) {
            sessions.remove(&session_id);
            return Err("Session has expired".to_string());
        }

        // Update last accessed time
        session.last_accessed = SystemTime::now();

        // Calculate page bounds
        let total_results = session.results.len();
        let end_offset = (offset + config.page_size).min(total_results);
        let has_more = end_offset < total_results;

        if offset >= total_results {
            return Ok(SearchPage {
                matches: vec![],
                count: 0,
                total_count: Some(total_results),
                has_more: false,
                truncated: false,
                next_cursor: None,
                current_page: (offset / config.page_size) + 1,
                original_page_size: config.page_size,
            });
        }

        // Extract the page of results
        let mut page_results = session.results[offset..end_offset].to_vec();

        // Apply snippet configuration
        if config.include_snippet {
            for result in &mut page_results {
                if result.preview.len() > config.snippet_length {
                    result.preview.truncate(config.snippet_length);
                    result.preview.push_str("...");
                }
            }
        } else {
            for result in &mut page_results {
                result.preview = "[snippet omitted]".to_string();
            }
        }

        // Generate next cursor if there are more results
        let next_cursor = if has_more {
            Some(self.create_cursor(
                session_id,
                end_offset,
                &session.search_params_hash,
                config.page_size,
            )?)
        } else {
            None
        };

        Ok(SearchPage {
            matches: page_results,
            count: end_offset - offset,
            total_count: Some(total_results),
            has_more,
            truncated: false, // TODO: Implement truncation logic
            next_cursor,
            current_page: (offset / config.page_size) + 1,
            original_page_size: config.page_size,
        })
    }

    /// Get first page and create session if needed
    pub async fn get_first_page(
        &self,
        search_options: SearchOptions,
        results: Vec<SearchResult>,
        config: PaginationConfig,
    ) -> Result<SearchPage, String> {
        let session_id = self.create_session(search_options, results).await?;
        self.get_page(session_id, 0, config).await
    }

    /// Parse cursor and get the corresponding page
    pub async fn get_page_by_cursor(
        &self,
        cursor: &str,
        config: PaginationConfig,
    ) -> Result<SearchPage, String> {
        let parsed_cursor = self.parse_cursor(cursor)?;

        // Validate cursor timestamp (not too old)
        let cursor_age = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("System time error: {}", e))?
            .as_secs()
            - parsed_cursor.timestamp;

        if cursor_age > self.session_ttl {
            return Err("Cursor has expired".to_string());
        }

        // Use the original page size from the cursor to maintain consistency
        let mut adjusted_config = config;
        adjusted_config.page_size = parsed_cursor.original_page_size;

        self.get_page(
            parsed_cursor.session_id,
            parsed_cursor.offset,
            adjusted_config,
        )
        .await
    }

    /// Create a base64-encoded cursor
    fn create_cursor(
        &self,
        session_id: Uuid,
        offset: usize,
        search_params_hash: &str,
        original_page_size: usize,
    ) -> Result<String, String> {
        let cursor = PaginationCursor {
            session_id,
            offset,
            search_params_hash: search_params_hash.to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| format!("System time error: {}", e))?
                .as_secs(),
            version: 1,
            original_page_size,
        };

        let cursor_json = serde_json::to_string(&cursor)
            .map_err(|e| format!("Failed to serialize cursor: {}", e))?;

        Ok(BASE64.encode(cursor_json.as_bytes()))
    }

    /// Parse a base64-encoded cursor
    fn parse_cursor(&self, cursor: &str) -> Result<PaginationCursor, String> {
        let cursor_bytes = BASE64
            .decode(cursor)
            .map_err(|e| format!("Invalid cursor format: {}", e))?;

        let cursor_json = String::from_utf8(cursor_bytes)
            .map_err(|e| format!("Invalid cursor encoding: {}", e))?;

        let parsed_cursor: PaginationCursor = serde_json::from_str(&cursor_json)
            .map_err(|e| format!("Invalid cursor structure: {}", e))?;

        // Validate cursor version
        if parsed_cursor.version != 1 {
            return Err("Unsupported cursor version".to_string());
        }

        Ok(parsed_cursor)
    }

    /// Hash search options to detect parameter changes
    fn hash_search_options(options: &SearchOptions) -> String {
        let mut hasher = Sha256::new();

        // Hash the essential search parameters
        hasher.update(options.query.as_bytes());
        hasher.update(options.path.to_string_lossy().as_bytes());
        hasher.update(format!("{:?}", options.mode).as_bytes());
        hasher.update(format!("{:?}", options.top_k).as_bytes());
        hasher.update(format!("{:?}", options.threshold).as_bytes());
        hasher.update(options.case_insensitive.to_string().as_bytes());
        hasher.update(options.whole_word.to_string().as_bytes());
        hasher.update(options.context_lines.to_string().as_bytes());

        format!("{:x}", hasher.finalize())
    }

    /// Check if a session has expired
    fn is_session_expired(&self, session: &SearchSession) -> bool {
        let now = SystemTime::now();
        let session_age = now
            .duration_since(session.last_accessed)
            .unwrap_or_default()
            .as_secs();
        session_age > self.session_ttl
    }

    /// Evict the oldest session to make room for new ones
    async fn evict_oldest_session(&self, sessions: &mut HashMap<Uuid, SearchSession>) {
        if let Some(oldest_id) = sessions
            .iter()
            .min_by_key(|(_, session)| session.last_accessed)
            .map(|(id, _)| *id)
        {
            sessions.remove(&oldest_id);
            debug!("Evicted oldest session: {}", oldest_id);
        }
    }

    /// Clean up expired sessions (should be called periodically)
    #[allow(dead_code)]
    pub async fn cleanup_expired_sessions(&self) -> usize {
        let mut sessions = self.sessions.write().await;
        let _initial_count = sessions.len();

        let expired_ids: Vec<Uuid> = sessions
            .iter()
            .filter(|(_, session)| self.is_session_expired(session))
            .map(|(id, _)| *id)
            .collect();

        for id in &expired_ids {
            sessions.remove(id);
        }

        let cleaned_count = expired_ids.len();
        if cleaned_count > 0 {
            debug!("Cleaned up {} expired sessions", cleaned_count);
        }

        cleaned_count
    }

    /// Get session statistics
    #[allow(dead_code)]
    pub async fn get_stats(&self) -> SessionStats {
        let sessions = self.sessions.read().await;
        let _now = SystemTime::now();

        let mut total_results = 0;
        let mut expired_count = 0;

        for session in sessions.values() {
            total_results += session.total_count;
            if self.is_session_expired(session) {
                expired_count += 1;
            }
        }

        SessionStats {
            total_sessions: sessions.len(),
            expired_sessions: expired_count,
            total_cached_results: total_results,
            memory_usage_estimate: sessions.len() * std::mem::size_of::<SearchSession>(),
        }
    }
}

/// Statistics about session manager state
#[derive(Debug)]
#[allow(dead_code)]
pub struct SessionStats {
    pub total_sessions: usize,
    pub expired_sessions: usize,
    pub total_cached_results: usize,
    pub memory_usage_estimate: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ck_core::{Language, SearchMode};
    use std::path::PathBuf;

    fn create_test_search_options() -> SearchOptions {
        SearchOptions {
            mode: SearchMode::Semantic,
            query: "test query".to_string(),
            path: PathBuf::from("/test/path"),
            top_k: Some(10),
            threshold: Some(0.5),
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
            show_scores: true,
            show_filenames: true,
            files_with_matches: false,
            files_without_matches: false,
            exclude_patterns: vec![],
            include_patterns: Vec::new(),
            respect_gitignore: true,
            use_ckignore: true,
            full_section: false,
            rerank: false,
            rerank_model: None,
            embedding_model: None,
        }
    }

    fn create_test_results(count: usize) -> Vec<SearchResult> {
        (0..count)
            .map(|i| SearchResult {
                file: PathBuf::from(format!("/test/file_{}.rs", i)),
                preview: format!("Test result {} content", i),
                span: ck_core::Span {
                    byte_start: i * 100,
                    byte_end: (i + 1) * 100,
                    line_start: i + 1,
                    line_end: i + 1,
                },
                score: 0.8 - (i as f32 * 0.01),
                lang: Some(Language::Rust),
                symbol: None,
                chunk_hash: None,
                index_epoch: None,
            })
            .collect()
    }

    #[tokio::test]
    async fn test_create_session() {
        let manager = SessionManager::default();
        let options = create_test_search_options();
        let results = create_test_results(10);

        let session_id = manager.create_session(options, results).await.unwrap();
        assert!(!session_id.is_nil());
    }

    #[tokio::test]
    async fn test_get_first_page() {
        let manager = SessionManager::default();
        let options = create_test_search_options();
        let results = create_test_results(100);
        let config = PaginationConfig::default();

        let page = manager
            .get_first_page(options, results, config)
            .await
            .unwrap();

        assert_eq!(page.count, DEFAULT_PAGE_SIZE);
        assert_eq!(page.total_count, Some(100));
        assert!(page.has_more);
        assert!(page.next_cursor.is_some());
    }

    #[tokio::test]
    async fn test_pagination() {
        let manager = SessionManager::default();
        let options = create_test_search_options();
        let results = create_test_results(75);
        let config = PaginationConfig::default();

        // Get first page
        let page1 = manager
            .get_first_page(options, results, config.clone())
            .await
            .unwrap();
        assert_eq!(page1.count, DEFAULT_PAGE_SIZE);
        assert!(page1.has_more);

        // Get second page using cursor
        let cursor = page1.next_cursor.unwrap();
        let page2 = manager.get_page_by_cursor(&cursor, config).await.unwrap();
        assert_eq!(page2.count, 25); // 75 - 50 = 25
        assert!(!page2.has_more);
        assert!(page2.next_cursor.is_none());
    }

    #[tokio::test]
    async fn test_cursor_validation() {
        let manager = SessionManager::default();
        let config = PaginationConfig::default();

        // Test invalid cursor
        let result = manager.get_page_by_cursor("invalid_cursor", config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_session_cleanup() {
        let manager = SessionManager::new(1); // 1 second TTL
        let options = create_test_search_options();
        let results = create_test_results(10);

        let _session_id = manager.create_session(options, results).await.unwrap();

        // Wait for session to expire
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let cleaned = manager.cleanup_expired_sessions().await;
        assert_eq!(cleaned, 1);
    }

    #[tokio::test]
    async fn test_snippet_truncation() {
        let manager = SessionManager::default();
        let options = create_test_search_options();

        // Create results with long content
        let long_content = "a".repeat(1000);
        let mut results = create_test_results(1);
        results[0].preview = long_content;

        let config = PaginationConfig {
            snippet_length: 100,
            ..Default::default()
        };

        let page = manager
            .get_first_page(options, results, config)
            .await
            .unwrap();

        // Should be truncated to 100 chars + "..."
        assert!(page.matches[0].preview.len() <= 103);
        assert!(page.matches[0].preview.ends_with("..."));
    }
}
