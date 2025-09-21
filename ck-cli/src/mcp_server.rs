use anyhow::Result;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::{ToolCallContext, ToolRoute};
use rmcp::model::{
    CallToolRequestParam, CallToolResult, Content, Implementation, InitializeResult,
    ListToolsResult, Meta, PaginatedRequestParam, ProgressNotificationParam, ProtocolVersion, Tool,
    ToolsCapability,
};
use rmcp::service::RequestContext;
use rmcp::transport;
use rmcp::{ErrorData, Peer, RoleServer};
use rmcp::{ServerHandler, ServiceExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::info;
use walkdir::WalkDir;

use crate::mcp::context::McpContext;
use crate::mcp::session::{PaginationConfig, SearchPage};
use ck_core::{SearchMode, SearchOptions, get_default_exclude_patterns};

/// Default top_k for MCP when not specified by client
/// Higher than CLI default (10) to allow multiple pages
const DEFAULT_MCP_TOP_K: usize = 100;

/// Filter out search results from missing files to prevent errors during result processing
fn filter_valid_results(mut results: Vec<ck_core::SearchResult>) -> Vec<ck_core::SearchResult> {
    results.retain(|result| result.file.exists());
    results
}

/// Trait for extracting pagination parameters from request structures
trait PaginationParams {
    fn get_page_size(&self) -> Option<usize>;
    fn get_include_snippet(&self) -> Option<bool>;
    fn get_snippet_length(&self) -> Option<usize>;
    fn get_context_lines(&self) -> Option<usize>;
    fn get_search_mode(&self) -> String;
    fn get_query(&self) -> String;
    fn get_search_params(&self) -> serde_json::Value;
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct SemanticSearchRequest {
    pub query: String,
    pub path: String,
    pub top_k: Option<usize>,
    pub threshold: Option<f32>,
    // Pagination parameters
    pub cursor: Option<String>,
    pub page_size: Option<usize>,
    pub include_snippet: Option<bool>,
    pub snippet_length: Option<usize>,
    pub context_lines: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct RegexSearchRequest {
    pub pattern: String,
    pub path: String,
    pub ignore_case: Option<bool>,
    pub context: Option<usize>,
    // Pagination parameters
    pub cursor: Option<String>,
    pub page_size: Option<usize>,
    pub include_snippet: Option<bool>,
    pub snippet_length: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct HybridSearchRequest {
    pub query: String,
    pub path: String,
    pub top_k: Option<usize>,
    pub threshold: Option<f32>,
    // Pagination parameters
    pub cursor: Option<String>,
    pub page_size: Option<usize>,
    pub include_snippet: Option<bool>,
    pub snippet_length: Option<usize>,
    pub context_lines: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct IndexStatusRequest {
    pub path: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ReindexRequest {
    pub path: String,
    pub force: Option<bool>,
}

impl PaginationParams for SemanticSearchRequest {
    fn get_page_size(&self) -> Option<usize> {
        self.page_size
    }
    fn get_include_snippet(&self) -> Option<bool> {
        self.include_snippet
    }
    fn get_snippet_length(&self) -> Option<usize> {
        self.snippet_length
    }
    fn get_context_lines(&self) -> Option<usize> {
        self.context_lines
    }
    fn get_search_mode(&self) -> String {
        "semantic".to_string()
    }
    fn get_query(&self) -> String {
        self.query.clone()
    }
    fn get_search_params(&self) -> serde_json::Value {
        json!({
            "top_k": self.top_k,
            "threshold": self.threshold.unwrap_or(0.6)
        })
    }
}

impl PaginationParams for RegexSearchRequest {
    fn get_page_size(&self) -> Option<usize> {
        self.page_size
    }
    fn get_include_snippet(&self) -> Option<bool> {
        self.include_snippet
    }
    fn get_snippet_length(&self) -> Option<usize> {
        self.snippet_length
    }
    fn get_context_lines(&self) -> Option<usize> {
        Some(self.context.unwrap_or(0))
    }
    fn get_search_mode(&self) -> String {
        "regex".to_string()
    }
    fn get_query(&self) -> String {
        self.pattern.clone()
    }
    fn get_search_params(&self) -> serde_json::Value {
        json!({
            "ignore_case": self.ignore_case.unwrap_or(false),
            "context_lines": self.context.unwrap_or(0)
        })
    }
}

impl PaginationParams for HybridSearchRequest {
    fn get_page_size(&self) -> Option<usize> {
        self.page_size
    }
    fn get_include_snippet(&self) -> Option<bool> {
        self.include_snippet
    }
    fn get_snippet_length(&self) -> Option<usize> {
        self.snippet_length
    }
    fn get_context_lines(&self) -> Option<usize> {
        self.context_lines
    }
    fn get_search_mode(&self) -> String {
        "hybrid".to_string()
    }
    fn get_query(&self) -> String {
        self.query.clone()
    }
    fn get_search_params(&self) -> serde_json::Value {
        json!({
            "top_k": self.top_k,
            "threshold": self.threshold.unwrap_or(0.02)
        })
    }
}

#[derive(Clone)]
pub struct CkMcpServer {
    context: McpContext,
    tool_router: ToolRouter<Self>,
}

impl ServerHandler for CkMcpServer {
    fn get_info(&self) -> InitializeResult {
        InitializeResult {
            protocol_version: ProtocolVersion::V_2024_11_05,
            server_info: Implementation {
                name: "ck".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: Some("CK Semantic Search Server".to_string()),
                website_url: Some("https://github.com/BeaconBay/ck".to_string()),
                icons: None,
            },
            capabilities: rmcp::model::ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                ..Default::default()
            },
            instructions: Some(r#"CK is a semantic code search engine that helps you find code by meaning, not just text matching.

## Available Tools:

- **semantic_search**: Find code by describing what it does, not exact text. Best for conceptual searches like "function that handles authentication" or "code that processes payments"
- **regex_search**: Traditional pattern matching. Use for exact text, symbols, or specific code patterns
- **hybrid_search**: Combines semantic and regex search with RRF ranking. Best when you want both conceptual matches and specific keywords
- **index_status**: Check if a directory is indexed and ready for semantic search
- **reindex**: Force rebuild of the semantic index when code has changed
- **health_check**: Verify the server is running and responsive

## Usage Tips:

1. Semantic search works best with natural language queries describing functionality
2. The first semantic search in a directory triggers automatic indexing
3. Use regex_search for exact matches, variable names, or specific syntax
4. Hybrid search is ideal when you know some keywords but want related code too
5. All searches respect .gitignore by default
6. Use pagination parameters to control result size and prevent large token responses

## Pagination Parameters:

All search tools support:
- **page_size** (default: 50, max: 200) - Results per page
- **include_snippet** (default: true) - Include code snippets
- **snippet_length** (default: 500) - Max characters per snippet
- **cursor** - Opaque cursor for subsequent pages
- **context_lines** - Lines of context (semantic/hybrid only)

## Examples:

- Semantic: "error handling for database connections"
- Regex: "async fn.*handle_request"
- Hybrid: "authentication login" (finds both exact matches and conceptually related code)
- Paginated: Use page_size=25 and follow next_cursor for large result sets"#.to_string()),
        }
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let tool_context = ToolCallContext::new(self, request, context);
        if let Some(route) = self.tool_router.map.get(&tool_context.name) {
            (route.call)(tool_context).await
        } else {
            Err(ErrorData::method_not_found::<
                rmcp::model::CallToolRequestMethod,
            >())
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools: Vec<Tool> = self
            .tool_router
            .map
            .values()
            .map(|route| route.attr.clone())
            .collect();
        Ok(ListToolsResult {
            tools,
            next_cursor: None,
        })
    }
}

impl CkMcpServer {
    pub fn new(cwd: PathBuf) -> Result<Self> {
        let context = McpContext::new(cwd)?;
        let tool_router = Self::create_tool_router();
        Ok(Self {
            context,
            tool_router,
        })
    }

    /// Extract pagination configuration from request parameters
    fn extract_pagination_config(
        page_size: Option<usize>,
        include_snippet: Option<bool>,
        snippet_length: Option<usize>,
        context_lines: Option<usize>,
    ) -> PaginationConfig {
        PaginationConfig {
            page_size: page_size.unwrap_or(50),
            include_snippet: include_snippet.unwrap_or(true),
            snippet_length: snippet_length.unwrap_or(500),
            context_lines: context_lines.unwrap_or(0),
        }
        .validate()
    }

    /// Convert SearchPage to structured JSON response
    fn search_page_to_json(
        page: SearchPage,
        query: &str,
        mode: &str,
        search_params: serde_json::Value,
    ) -> serde_json::Value {
        let results: Vec<serde_json::Value> = page.matches.iter().map(|result| {
            let match_type = format!("{}_match", mode);
            let mut match_obj = json!({
                "file": {
                    "path": result.file.to_string_lossy(),
                    "language": result.lang.as_ref().map(|l| l.to_string()).unwrap_or("unknown".to_string())
                },
                "match": {
                    "span": {
                        "byte_start": result.span.byte_start,
                        "byte_end": result.span.byte_end,
                        "line_start": result.span.line_start,
                        "line_end": result.span.line_end
                    },
                    "content": result.preview
                },
                "type": match_type
            });

            // Add score for semantic and hybrid searches
            if mode == "semantic" || mode == "hybrid" {
                match_obj["match"]["score"] = json!(result.score);
                if mode == "hybrid" {
                    match_obj["match"]["rrf_score"] = json!(result.score);
                }
            }

            // Add line number for regex searches
            if mode == "regex" {
                match_obj["match"]["line_number"] = json!(result.span.line_start);
            }

            match_obj
        }).collect();

        json!({
            "search": {
                "query": query,
                "mode": mode,
                "parameters": search_params
            },
            "results": {
                "matches": results,
                "count": page.count,
                "total_count": page.total_count,
                "has_more": page.has_more,
                "truncated": page.truncated
            },
            "pagination": {
                "next_cursor": page.next_cursor,
                "page_size": page.original_page_size,
                "current_page": page.current_page
            },
            "metadata": {
                "search_time_ms": 0, // TODO: Add timing
                "index_stats": null  // TODO: Add index information
            }
        })
    }

    /// Handle paginated search request (when cursor is provided)
    async fn handle_paginated_request<T>(
        &self,
        cursor: &str,
        request: &T,
    ) -> Result<(String, Value), ErrorData>
    where
        T: PaginationParams,
    {
        let config = Self::extract_pagination_config(
            request.get_page_size(),
            request.get_include_snippet(),
            request.get_snippet_length(),
            request.get_context_lines(),
        );

        let page = self
            .context
            .session_manager
            .get_page_by_cursor(cursor, config)
            .await
            .map_err(|e| ErrorData::invalid_params(e, None))?;

        let mode = request.get_search_mode();
        let query = request.get_query();
        let search_params = request.get_search_params();

        let structured_result = Self::search_page_to_json(page, &query, &mode, search_params);

        let summary = format!(
            "Retrieved page {} of {} search results for '{}'",
            structured_result["pagination"]["current_page"], mode, query
        );

        Ok((summary, structured_result))
    }

    fn create_tool_router() -> ToolRouter<Self> {
        let mut router = ToolRouter::new();
        router.add_route(Self::health_check_route());
        router.add_route(Self::semantic_search_route());
        router.add_route(Self::regex_search_route());
        router.add_route(Self::hybrid_search_route());
        router.add_route(Self::index_status_route());
        router.add_route(Self::reindex_route());
        router
    }

    fn health_check_route() -> ToolRoute<Self> {
        let input_schema = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {},
            "additionalProperties": false,
        });
        let tool = Tool {
            name: "health_check".into(),
            title: Some("Health Check".into()),
            description: Some("Health check tool to verify server status".into()),
            input_schema: Arc::new(input_schema.as_object().unwrap().clone()),
            output_schema: None,
            annotations: None,
            icons: None,
        };

        ToolRoute::new_dyn(tool, |context: ToolCallContext<'_, CkMcpServer>| {
            Box::pin(async move {
                let status_data = json!({
                    "status": "healthy",
                    "server": "ck",
                    "version": env!("CARGO_PKG_VERSION"),
                    "protocol": "mcp",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "cwd": context.service.context.cwd.to_string_lossy()
                });

                let summary = format!(
                    "CK Semantic Search Server v{} is healthy and ready (MCP protocol, working directory: {})",
                    env!("CARGO_PKG_VERSION"),
                    context.service.context.cwd.to_string_lossy()
                );

                Ok(CallToolResult {
                    content: vec![
                        Content::text(summary),
                        Content::json(status_data.clone())
                            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
                    ],
                    structured_content: Some(status_data),
                    is_error: Some(false),
                    meta: None,
                })
            })
        })
    }

    fn semantic_search_route() -> ToolRoute<Self> {
        let schema = schemars::schema_for!(SemanticSearchRequest);
        let input_schema = serde_json::to_value(schema).unwrap();
        let tool = Tool {
            name: "semantic_search".into(),
            title: Some("Semantic Search".into()),
            description: Some("Search for code semantically using embeddings".into()),
            input_schema: Arc::new(input_schema.as_object().unwrap().clone()),
            output_schema: None,
            annotations: None,
            icons: None,
        };

        ToolRoute::new_dyn(tool, |context: ToolCallContext<'_, CkMcpServer>| {
            Box::pin(async move {
                let arguments = context.arguments.clone().unwrap_or_default();
                let request: SemanticSearchRequest =
                    serde_json::from_value(serde_json::Value::Object(arguments)).map_err(|e| {
                        rmcp::ErrorData::invalid_params(format!("Invalid parameters: {}", e), None)
                    })?;

                let service: &CkMcpServer = context.service;
                let meta = context.request_context.meta.clone();
                let peer = context.request_context.peer;
                match service
                    .handle_semantic_search(request, Some(meta), Some(peer))
                    .await
                {
                    Ok((summary, result)) => Ok(CallToolResult {
                        content: vec![
                            Content::text(summary),
                            Content::json(result.clone())
                                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
                        ],
                        structured_content: Some(result),
                        is_error: Some(false),
                        meta: None,
                    }),
                    Err(e) => Err(e),
                }
            })
        })
    }

    fn regex_search_route() -> ToolRoute<Self> {
        let schema = schemars::schema_for!(RegexSearchRequest);
        let input_schema = serde_json::to_value(schema).unwrap();
        let tool = Tool {
            name: "regex_search".into(),
            title: Some("Regex Search".into()),
            description: Some("Search for code using regular expressions (grep-style)".into()),
            input_schema: Arc::new(input_schema.as_object().unwrap().clone()),
            output_schema: None,
            annotations: None,
            icons: None,
        };

        ToolRoute::new_dyn(tool, |context: ToolCallContext<'_, CkMcpServer>| {
            Box::pin(async move {
                let arguments = context.arguments.clone().unwrap_or_default();
                let request: RegexSearchRequest =
                    serde_json::from_value(serde_json::Value::Object(arguments)).map_err(|e| {
                        rmcp::ErrorData::invalid_params(format!("Invalid parameters: {}", e), None)
                    })?;

                let service: &CkMcpServer = context.service;
                match service.handle_regex_search(request).await {
                    Ok((summary, result)) => Ok(CallToolResult {
                        content: vec![
                            Content::text(summary),
                            Content::json(result.clone())
                                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
                        ],
                        structured_content: Some(result),
                        is_error: Some(false),
                        meta: None,
                    }),
                    Err(e) => Err(e),
                }
            })
        })
    }

    fn hybrid_search_route() -> ToolRoute<Self> {
        let schema = schemars::schema_for!(HybridSearchRequest);
        let input_schema = serde_json::to_value(schema).unwrap();
        let tool = Tool {
            name: "hybrid_search".into(),
            title: Some("Hybrid Search".into()),
            description: Some(
                "Hybrid search combining regex and semantic search with RRF ranking".into(),
            ),
            input_schema: Arc::new(input_schema.as_object().unwrap().clone()),
            output_schema: None,
            annotations: None,
            icons: None,
        };

        ToolRoute::new_dyn(tool, |context: ToolCallContext<'_, CkMcpServer>| {
            Box::pin(async move {
                let arguments = context.arguments.clone().unwrap_or_default();
                let request: HybridSearchRequest =
                    serde_json::from_value(serde_json::Value::Object(arguments)).map_err(|e| {
                        rmcp::ErrorData::invalid_params(format!("Invalid parameters: {}", e), None)
                    })?;

                let service: &CkMcpServer = context.service;
                match service.handle_hybrid_search(request).await {
                    Ok((summary, result)) => Ok(CallToolResult {
                        content: vec![
                            Content::text(summary),
                            Content::json(result.clone())
                                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
                        ],
                        structured_content: Some(result),
                        is_error: Some(false),
                        meta: None,
                    }),
                    Err(e) => Err(e),
                }
            })
        })
    }

    fn index_status_route() -> ToolRoute<Self> {
        let schema = schemars::schema_for!(IndexStatusRequest);
        let input_schema = serde_json::to_value(schema).unwrap();
        let tool = Tool {
            name: "index_status".into(),
            title: Some("Index Status".into()),
            description: Some("Get information about the index status for a directory".into()),
            input_schema: Arc::new(input_schema.as_object().unwrap().clone()),
            output_schema: None,
            annotations: None,
            icons: None,
        };

        ToolRoute::new_dyn(tool, |context: ToolCallContext<'_, CkMcpServer>| {
            Box::pin(async move {
                let arguments = context.arguments.clone().unwrap_or_default();
                let request: IndexStatusRequest =
                    serde_json::from_value(serde_json::Value::Object(arguments)).map_err(|e| {
                        rmcp::ErrorData::invalid_params(format!("Invalid parameters: {}", e), None)
                    })?;

                let service: &CkMcpServer = context.service;
                let meta = context.request_context.meta.clone();
                let peer = context.request_context.peer;
                match service
                    .handle_index_status(request, Some(meta), Some(peer))
                    .await
                {
                    Ok((summary, result)) => Ok(CallToolResult {
                        content: vec![
                            Content::text(summary),
                            Content::json(result.clone())
                                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
                        ],
                        structured_content: Some(result),
                        is_error: Some(false),
                        meta: None,
                    }),
                    Err(e) => Err(e),
                }
            })
        })
    }

    fn reindex_route() -> ToolRoute<Self> {
        let schema = schemars::schema_for!(ReindexRequest);
        let input_schema = serde_json::to_value(schema).unwrap();
        let tool = Tool {
            name: "reindex".into(),
            title: Some("Reindex Directory".into()),
            description: Some("Force reindexing of a directory with progress tracking".into()),
            input_schema: Arc::new(input_schema.as_object().unwrap().clone()),
            output_schema: None,
            annotations: None,
            icons: None,
        };

        ToolRoute::new_dyn(tool, |context: ToolCallContext<'_, CkMcpServer>| {
            Box::pin(async move {
                let arguments = context.arguments.clone().unwrap_or_default();
                let request: ReindexRequest =
                    serde_json::from_value(serde_json::Value::Object(arguments)).map_err(|e| {
                        rmcp::ErrorData::invalid_params(format!("Invalid parameters: {}", e), None)
                    })?;

                let service: &CkMcpServer = context.service;
                let meta = context.request_context.meta.clone();
                let peer = context.request_context.peer;
                match service
                    .handle_reindex(request, Some(meta), Some(peer))
                    .await
                {
                    Ok((summary, result)) => Ok(CallToolResult {
                        content: vec![
                            Content::text(summary),
                            Content::json(result.clone())
                                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
                        ],
                        structured_content: Some(result),
                        is_error: Some(false),
                        meta: None,
                    }),
                    Err(e) => Err(e),
                }
            })
        })
    }

    pub async fn run(&self) -> Result<()> {
        info!("Starting ck MCP server");

        let stdio_transport = transport::stdio();
        let running_service = self.clone().serve(stdio_transport).await?;
        running_service.waiting().await?;
        Ok(())
    }

    pub async fn handle_semantic_search(
        &self,
        request: SemanticSearchRequest,
        meta: Option<Meta>,
        peer: Option<Peer<RoleServer>>,
    ) -> Result<(String, Value), ErrorData> {
        // Handle pagination via cursor
        if let Some(cursor) = &request.cursor {
            return self.handle_paginated_request(cursor, &request).await;
        }

        let query = request.query.clone();
        let path = request.path;
        let top_k = request.top_k;
        let threshold = request.threshold;
        let path_buf = PathBuf::from(path);

        // Clone values before they're moved into SearchOptions
        let query_clone = query.clone();
        let path_clone = path_buf.clone();

        // Validate path exists
        if !path_buf.exists() {
            return Err(ErrorData::invalid_params(
                format!("Path does not exist: {}", path_buf.display()),
                None,
            ));
        }

        // Extract pagination config
        let config = Self::extract_pagination_config(
            request.page_size,
            request.include_snippet,
            request.snippet_length,
            request.context_lines,
        );

        // Create progress callback for indexing if we have a progress token and peer
        let indexing_progress_callback = if let (Some(meta), Some(peer)) = (&meta, &peer) {
            if let Some(progress_token) = meta.get_progress_token() {
                let token = progress_token.clone();
                let peer = peer.clone();
                let step_count = Arc::new(AtomicUsize::new(0));
                Some(Box::new(move |message: &str| {
                    let token = token.clone();
                    let peer = peer.clone();
                    let message = message.to_string();
                    let current_step = step_count.fetch_add(1, Ordering::SeqCst) + 1;
                    tokio::spawn(async move {
                        let _ = peer
                            .notify_progress(ProgressNotificationParam {
                                progress_token: token,
                                progress: current_step as f64,
                                total: None, // Unknown total for indexing
                                message: Some(message),
                            })
                            .await;
                    });
                }) as ck_engine::IndexingProgressCallback)
            } else {
                None
            }
        } else {
            None
        };

        let options = SearchOptions {
            mode: SearchMode::Semantic,
            query,
            path: path_buf,
            top_k: top_k.or(Some(DEFAULT_MCP_TOP_K)), // User-defined or MCP default
            threshold: threshold.or(Some(0.6)),
            case_insensitive: false,
            whole_word: false,
            fixed_string: false,
            line_numbers: false,
            context_lines: 0,
            before_context_lines: 0,
            after_context_lines: 0,
            recursive: true,
            json_output: false,
            jsonl_output: true,
            no_snippet: false,
            reindex: false,
            show_scores: true,
            show_filenames: true,
            files_with_matches: false,
            files_without_matches: false,
            exclude_patterns: get_default_exclude_patterns(),
            respect_gitignore: true,
            full_section: false,
            rerank: false,
            rerank_model: None,
            embedding_model: None,
        };

        // Note: Embedders are created fresh for each request by ck-engine
        // Caching would require exposing search APIs that accept pre-created embedders

        // Perform the search with progress reporting
        let search_results = match ck_engine::search_enhanced_with_indexing_progress(
            &options,
            None, // No search progress callback for MCP
            indexing_progress_callback,
            None, // No detailed indexing progress callback for MCP
        )
        .await
        {
            Ok(results) => results,
            Err(e) => return Err(ErrorData::internal_error(e.to_string(), None)),
        };

        // Create session and get first page
        let page = self
            .context
            .session_manager
            .get_first_page(
                options,
                filter_valid_results(search_results.matches),
                config,
            )
            .await
            .map_err(|e| ErrorData::internal_error(e, None))?;

        let search_params = json!({
            "top_k": top_k.unwrap_or(DEFAULT_MCP_TOP_K),
            "threshold": threshold.unwrap_or(0.6)
        });

        let current_page = page.current_page;
        let structured_result =
            Self::search_page_to_json(page, &query_clone, "semantic", search_params);

        let summary = format!(
            "Semantic search for '{}' found {} matches in {} (threshold: {:.2}, top_k: {}) - Page {}",
            query_clone,
            structured_result["results"]["count"],
            path_clone.display(),
            threshold.unwrap_or(0.6),
            top_k.unwrap_or(DEFAULT_MCP_TOP_K),
            current_page
        );

        Ok((summary, structured_result))
    }

    pub async fn handle_regex_search(
        &self,
        request: RegexSearchRequest,
    ) -> Result<(String, Value), ErrorData> {
        // Handle pagination via cursor
        if let Some(cursor) = &request.cursor {
            return self.handle_paginated_request(cursor, &request).await;
        }
        let pattern = request.pattern.clone();
        let path = request.path;
        let ignore_case = request.ignore_case;
        let context = request.context;
        let path_buf = PathBuf::from(path);

        // Clone values before they're moved into SearchOptions
        let pattern_clone = pattern.clone();
        let path_clone = path_buf.clone();

        // Validate path exists
        if !path_buf.exists() {
            return Err(ErrorData::invalid_params(
                format!("Path does not exist: {}", path_buf.display()),
                None,
            ));
        }

        let context_lines = context.unwrap_or(0);

        // Extract pagination config
        let config = Self::extract_pagination_config(
            request.page_size,
            request.include_snippet,
            request.snippet_length,
            Some(context_lines),
        );

        let options = SearchOptions {
            mode: SearchMode::Regex,
            query: pattern,
            path: path_buf,
            top_k: None,     // No limit for regex search
            threshold: None, // No threshold for regex search
            case_insensitive: ignore_case.unwrap_or(false),
            whole_word: false,
            fixed_string: false,
            line_numbers: true,
            context_lines,
            before_context_lines: context_lines,
            after_context_lines: context_lines,
            recursive: true,
            json_output: false,
            jsonl_output: true,
            no_snippet: false,
            reindex: false,
            show_scores: false, // No scores for regex search
            show_filenames: true,
            files_with_matches: false,
            files_without_matches: false,
            exclude_patterns: get_default_exclude_patterns(),
            respect_gitignore: true,
            full_section: false,
            rerank: false,
            rerank_model: None,
            embedding_model: None,
        };

        // Perform the search (no indexing needed for regex)
        let search_results = match ck_engine::search_enhanced_with_indexing_progress(
            &options, None, // No search progress callback for MCP
            None, // No indexing progress callback for MCP
            None, // No detailed indexing progress callback for MCP
        )
        .await
        {
            Ok(results) => results,
            Err(e) => return Err(ErrorData::internal_error(e.to_string(), None)),
        };

        // Create session and get first page
        let page = self
            .context
            .session_manager
            .get_first_page(
                options,
                filter_valid_results(search_results.matches),
                config,
            )
            .await
            .map_err(|e| ErrorData::internal_error(e, None))?;

        let search_params = json!({
            "ignore_case": ignore_case.unwrap_or(false),
            "context_lines": context.unwrap_or(0)
        });

        let structured_result =
            Self::search_page_to_json(page, &pattern_clone, "regex", search_params);

        let summary = format!(
            "Regex search for pattern '{}' found {} matches in {} (case_sensitive: {}, context: {} lines) - Page 1",
            pattern_clone,
            structured_result["results"]["count"],
            path_clone.display(),
            !ignore_case.unwrap_or(false),
            context.unwrap_or(0)
        );

        Ok((summary, structured_result))
    }

    pub async fn handle_hybrid_search(
        &self,
        request: HybridSearchRequest,
    ) -> Result<(String, Value), ErrorData> {
        // Handle pagination via cursor
        if let Some(cursor) = &request.cursor {
            return self.handle_paginated_request(cursor, &request).await;
        }
        let query = request.query.clone();
        let path = request.path;
        let top_k = request.top_k;
        let threshold = request.threshold;
        let path_buf = PathBuf::from(path);

        // Clone values before they're moved into SearchOptions
        let query_clone = query.clone();
        let path_clone = path_buf.clone();

        // Validate path exists
        if !path_buf.exists() {
            return Err(ErrorData::invalid_params(
                format!("Path does not exist: {}", path_buf.display()),
                None,
            ));
        }

        // Extract pagination config
        let config = Self::extract_pagination_config(
            request.page_size,
            request.include_snippet,
            request.snippet_length,
            request.context_lines,
        );

        let options = SearchOptions {
            mode: SearchMode::Hybrid,
            query,
            path: path_buf,
            top_k: top_k.or(Some(DEFAULT_MCP_TOP_K)), // User-defined or MCP default
            threshold: threshold.or(Some(0.02)),      // Lower threshold for hybrid (RRF scores)
            case_insensitive: false,
            whole_word: false,
            fixed_string: false,
            line_numbers: false,
            context_lines: 0,
            before_context_lines: 0,
            after_context_lines: 0,
            recursive: true,
            json_output: false,
            jsonl_output: true,
            no_snippet: false,
            reindex: false,
            show_scores: true,
            show_filenames: true,
            files_with_matches: false,
            files_without_matches: false,
            exclude_patterns: get_default_exclude_patterns(),
            respect_gitignore: true,
            full_section: false,
            rerank: false,
            rerank_model: None,
            embedding_model: None,
        };

        // Perform the search (suppress progress callbacks for MCP)
        let search_results = match ck_engine::search_enhanced_with_indexing_progress(
            &options, None, // No search progress callback for MCP
            None, // No indexing progress callback for MCP
            None, // No detailed indexing progress callback for MCP
        )
        .await
        {
            Ok(results) => results,
            Err(e) => return Err(ErrorData::internal_error(e.to_string(), None)),
        };

        // Create session and get first page
        let page = self
            .context
            .session_manager
            .get_first_page(
                options,
                filter_valid_results(search_results.matches),
                config,
            )
            .await
            .map_err(|e| ErrorData::internal_error(e, None))?;

        let search_params = json!({
            "top_k": top_k.unwrap_or(DEFAULT_MCP_TOP_K),
            "threshold": threshold.unwrap_or(0.02)
        });

        let current_page = page.current_page;
        let structured_result =
            Self::search_page_to_json(page, &query_clone, "hybrid", search_params);

        let summary = format!(
            "Hybrid search for '{}' found {} matches in {} (threshold: {:.3}, top_k: {}, combines semantic + regex) - Page {}",
            query_clone,
            structured_result["results"]["count"],
            path_clone.display(),
            threshold.unwrap_or(0.02),
            top_k.unwrap_or(DEFAULT_MCP_TOP_K),
            current_page
        );

        Ok((summary, structured_result))
    }

    async fn handle_index_status(
        &self,
        request: IndexStatusRequest,
        _meta: Option<Meta>,
        _peer: Option<Peer<RoleServer>>,
    ) -> Result<(String, Value), ErrorData> {
        let path = request.path;
        let path_buf = PathBuf::from(path);

        // Validate path exists
        if !path_buf.exists() {
            return Err(ErrorData::invalid_params(
                format!("Path does not exist: {}", path_buf.display()),
                None,
            ));
        }

        // Use concurrency lock for this directory
        let lock = self.context.get_index_lock(&path_buf).await;
        let _guard = lock.lock().await;

        // Check if index exists and get stats
        let index_path = path_buf.join(".ck");
        let index_exists = index_path.exists();

        let mut index_info = json!({
            "path": path_buf.to_string_lossy(),
            "index_exists": index_exists,
            "index_path": index_path.to_string_lossy(),
        });

        if index_exists {
            // Try to get more detailed information about the index
            if let Ok(metadata) = std::fs::metadata(&index_path) {
                index_info["index_size_bytes"] = json!(metadata.len());
                index_info["last_modified"] = json!(
                    metadata
                        .modified()
                        .map(|t| t
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs())
                        .unwrap_or(0)
                );
            }

            // Get detailed index statistics using ck_index with caching
            if let Some(cached_stats) = self.context.stats_cache.get(&path_buf).await {
                index_info["total_files"] = json!(cached_stats.file_count);
                index_info["total_chunks"] = json!(cached_stats.chunk_count);
                index_info["cache_hit"] = json!(true);
            } else if let Ok(index_stats) = ck_index::get_index_stats(&path_buf) {
                index_info["total_files"] = json!(index_stats.total_files);
                index_info["total_chunks"] = json!(index_stats.total_chunks);
                index_info["cache_hit"] = json!(false);

                // Update cache with fresh stats
                let cache_stats = crate::mcp::cache::IndexStats {
                    file_count: index_stats.total_files,
                    chunk_count: index_stats.total_chunks,
                    model_name: "unknown".to_string(), // TODO: Get from manifest
                    last_updated: std::time::SystemTime::now(),
                    is_valid: true,
                };
                self.context
                    .stats_cache
                    .update(path_buf.clone(), cache_stats)
                    .await;
            } else {
                // Fallback: Count files in directory for estimation
                let file_count = WalkDir::new(&path_buf)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                    .count();

                index_info["estimated_file_count"] = json!(file_count);
            }
        }

        let structured_result = json!({
            "index_status": index_info,
            "metadata": {
                "checked_at": chrono::Utc::now().to_rfc3339(),
                "path_type": if path_buf.is_dir() { "directory" } else { "file" }
            }
        });

        let summary = if index_exists {
            let file_count = index_info
                .get("total_files")
                .or_else(|| index_info.get("estimated_file_count"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let chunk_count = index_info
                .get("total_chunks")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            if chunk_count > 0 {
                format!(
                    "Index exists for {} with {} files and {} chunks",
                    path_buf.display(),
                    file_count,
                    chunk_count
                )
            } else {
                format!(
                    "Index exists for {} with {} files",
                    path_buf.display(),
                    file_count
                )
            }
        } else {
            format!(
                "No index found for {} - indexing would be required for semantic search",
                path_buf.display()
            )
        };

        Ok((summary, structured_result))
    }

    async fn handle_reindex(
        &self,
        request: ReindexRequest,
        meta: Option<Meta>,
        peer: Option<Peer<RoleServer>>,
    ) -> Result<(String, Value), ErrorData> {
        let path = request.path;
        let force = request.force.unwrap_or(false);
        let path_buf = PathBuf::from(path);

        // Validate path exists
        if !path_buf.exists() {
            return Err(ErrorData::invalid_params(
                format!("Path does not exist: {}", path_buf.display()),
                None,
            ));
        }

        // Use concurrency lock for this directory
        let lock = self.context.get_index_lock(&path_buf).await;
        let _guard = lock.lock().await;

        // Create progress callback for reindexing if we have a progress token and peer
        let progress_callback = if let (Some(meta), Some(peer)) = (&meta, &peer) {
            if let Some(progress_token) = meta.get_progress_token() {
                let token = progress_token.clone();
                let peer = peer.clone();
                let step_count = Arc::new(AtomicUsize::new(0));
                Some(Box::new(move |message: &str| {
                    let token = token.clone();
                    let peer = peer.clone();
                    let message = message.to_string();
                    let current_step = step_count.fetch_add(1, Ordering::SeqCst) + 1;
                    tokio::spawn(async move {
                        let _ = peer
                            .notify_progress(ProgressNotificationParam {
                                progress_token: token,
                                progress: current_step as f64,
                                total: None, // Unknown total for reindexing
                                message: Some(message),
                            })
                            .await;
                    });
                }) as ck_engine::IndexingProgressCallback)
            } else {
                None
            }
        } else {
            None
        };

        // Create search options for reindexing
        let options = SearchOptions {
            mode: SearchMode::Semantic, // Use semantic mode to ensure embeddings are computed
            query: String::new(),       // Empty query for reindexing only
            path: path_buf.clone(),
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
            jsonl_output: true,
            no_snippet: false,
            reindex: force, // Use the force parameter directly
            show_scores: false,
            show_filenames: false,
            files_with_matches: false,
            files_without_matches: false,
            exclude_patterns: get_default_exclude_patterns(),
            respect_gitignore: true,
            full_section: false,
            rerank: false,
            rerank_model: None,
            embedding_model: None,
        };

        // Perform reindexing
        let start_time = std::time::Instant::now();
        let reindex_result = match ck_engine::search_enhanced_with_indexing_progress(
            &options,
            None, // No search progress callback
            progress_callback,
            None, // No detailed indexing progress callback
        )
        .await
        {
            Ok(_) => {
                let duration = start_time.elapsed();

                // Invalidate cache after reindexing
                self.context.stats_cache.invalidate(&path_buf).await;

                json!({
                    "status": "success",
                    "duration_ms": duration.as_millis(),
                    "path": path_buf.to_string_lossy(),
                    "force": force,
                })
            }
            Err(e) => {
                return Err(ErrorData::internal_error(
                    format!("Reindexing failed: {}", e),
                    None,
                ));
            }
        };

        let structured_result = json!({
            "reindex_result": reindex_result,
            "metadata": {
                "completed_at": chrono::Utc::now().to_rfc3339(),
                "path_type": if path_buf.is_dir() { "directory" } else { "file" }
            }
        });

        let summary = format!(
            "Successfully reindexed {} in {}ms",
            path_buf.display(),
            reindex_result.get("duration_ms").unwrap_or(&json!(0))
        );

        Ok((summary, structured_result))
    }
}
