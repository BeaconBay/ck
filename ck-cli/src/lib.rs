// Library interface for testing internal modules

pub mod mcp;
pub mod mcp_server;
pub mod path_utils;
// TUI is now in its own crate: ck-tui

// Re-export commonly used types for testing
pub use mcp_server::{
    CkMcpServer, HybridSearchRequest, IndexStatusRequest, RegexSearchRequest, SemanticSearchRequest,
};
