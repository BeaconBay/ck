use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

// EmbedderCache removed - embedder caching would require deeper integration with ck-engine.
// Currently, search functions create embedders internally via ck_embed::create_embedder().
// Future optimization: expose search APIs that accept pre-created embedders for true caching.

/// Cache for index statistics with TTL. Stores the full `ck_index::IndexStats`
/// so cache hits report the same fields as fresh reads (chunk counts, embedded
/// chunk counts, on-disk index size).
pub use ck_index::IndexStats;

#[derive(Clone)]
pub struct StatsCache {
    stats: Arc<RwLock<HashMap<PathBuf, (IndexStats, SystemTime)>>>,
    ttl: Duration,
}

impl StatsCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            stats: Arc::new(RwLock::new(HashMap::new())),
            ttl,
        }
    }

    pub async fn get(&self, path: &PathBuf) -> Option<IndexStats> {
        let stats = self.stats.read().await;
        if let Some((stats, cached_at)) = stats.get(path)
            && cached_at.elapsed().unwrap_or_default() < self.ttl
        {
            return Some(stats.clone());
        }
        None
    }

    pub async fn update(&self, path: PathBuf, stats: IndexStats) {
        let mut cache = self.stats.write().await;
        cache.insert(path, (stats, SystemTime::now()));
    }

    pub async fn invalidate(&self, path: &PathBuf) {
        let mut cache = self.stats.write().await;
        cache.remove(path);
    }
}

impl Default for StatsCache {
    /// Create a stats cache with default 30-second TTL optimized for MCP server usage
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}
