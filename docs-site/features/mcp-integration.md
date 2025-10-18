---
title: MCP Integration
description: Built-in MCP (Model Context Protocol) server for AI coding assistants. Integrate ck with Claude Desktop, Cursor, and other MCP-compatible AI tools.
---

# MCP Integration

ck includes a built-in MCP (Model Context Protocol) server for seamless integration with AI coding assistants like Claude Desktop and Cursor.

## What is MCP?

MCP (Model Context Protocol) is Anthropic’s standard for connecting AI assistants to external tools and data sources. ck’s MCP server gives AI agents powerful semantic code search capabilities.

## Quick Start

### Start MCP Server

```bash
# Start the MCP server
ck --serve
```

The server listens on stdio and communicates via JSON-RPC 2.0.

### Install in Claude Desktop

**Using Claude Code CLI (Recommended):**

```bash
claude mcp add ck-search -s user -- ck --serve

# Restart Claude Code if needed
# Verify with:
claude mcp list
```

**Manual Configuration:**

Edit `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) or equivalent:

```json
{
  "mcpServers": {
    "ck": {
      "command": "ck",
      "args": ["--serve"],
      "cwd": "/path/to/your/codebase"
    }
  }
}
```

**Tool Permissions:**

When prompted by Claude Code, approve permissions for ck-search tools.

## Available MCP Tools

### `semantic_search`

Find code by meaning using embeddings.

**Parameters:**
- `query` (string, required): Search query
- `path` (string, optional): Directory to search (defaults to cwd)
- `page_size` (integer, optional): Results per page (default: 25)
- `top_k` (integer, optional): Maximum total results (default: 100)
- `snippet_length` (integer, optional): Snippet size in characters (default: 200)
- `cursor` (string, optional): Pagination cursor
- `threshold` (number, optional): Minimum relevance score (0.0-1.0)

**Example:**
```python
response = await client.call_tool("semantic_search", {
    "query": "authentication logic",
    "path": "/path/to/code",
    "page_size": 25,
    "top_k": 50,
    "threshold": 0.6
})
```

### `regex_search`

Traditional pattern matching (grep-style).

**Parameters:**
- `pattern` (string, required): Regex pattern
- `path` (string, optional): Directory to search
- `page_size` (integer, optional): Results per page
- `case_insensitive` (boolean, optional): Case-insensitive search
- `cursor` (string, optional): Pagination cursor

**Example:**
```python
response = await client.call_tool("regex_search", {
    "pattern": "TODO|FIXME",
    "path": "/path/to/code",
    "case_insensitive": true
})
```

### `hybrid_search`

Combined semantic and keyword search using Reciprocal Rank Fusion.

**Parameters:**
- `query` (string, required): Search query
- `path` (string, optional): Directory to search
- `page_size` (integer, optional): Results per page
- `top_k` (integer, optional): Maximum total results
- `threshold` (number, optional): Minimum relevance score (RRF scale: ~0.01-0.05)

::: warning Hybrid Threshold Scale
Hybrid search uses Reciprocal Rank Fusion (RRF) scoring, producing values in the 0.01-0.05 range, not 0.0-1.0 like semantic search. Typical thresholds: 0.016-0.025. See [Hybrid Search](/features/hybrid-search#understanding-hybrid-thresholds) for details.
:::

**Example:**
```python
response = await client.call_tool("hybrid_search", {
    "query": "connection timeout",
    "path": "/path/to/code",
    "threshold": 0.02  # Note: RRF scale, not 0-1
})
```

### `index_status`

Check indexing status and metadata.

**Parameters:**
- `path` (string, optional): Directory to check

**Returns:**
- Index statistics
- Embedding model info
- File counts
- Last update time

### `reindex`

Force rebuild of search index.

**Parameters:**
- `path` (string, optional): Directory to reindex
- `model` (string, optional): Embedding model to use

**Example:**
```python
await client.call_tool("reindex", {
    "path": "/path/to/code",
    "model": "nomic-v1.5"
})
```

### `default_ckignore`

Retrieve default `.ckignore` patterns used by ck.

**Parameters:**
None

**Returns:**
- String containing default `.ckignore` content

**Use case:**
Useful for editor extensions and tools that need to display or work with ck’s default exclusion patterns without parsing the backend code.

**Example:**
```python
patterns = await client.call_tool("default_ckignore", {})
# Returns multi-line string with default patterns:
# *.png
# *.jpg
# node_modules/
# ...
```

::: tip CLI Alternative
You can also get default patterns via CLI:
```bash
ck --print-default-ckignore
```
:::

### `health_check`

Server status and diagnostics.

**Returns:**
- Server version
- Available models
- System status

## Response Format

All search tools return paginated results:

```json
{
  "results": [
    {
      "file": "src/auth.rs",
      "line": 42,
      "content": "fn authenticate_user(token: &str) -> Result<User>",
      "score": 0.847
    }
  ],
  "pagination": {
    "total_results": 127,
    "returned_results": 25,
    "next_cursor": "abc123xyz",
    "has_more": true
  }
}
```

## Pagination

Handle large result sets efficiently:

```python
# First page
response = await client.call_tool("semantic_search", {
    "query": "authentication logic",
    "path": "/path/to/code",
    "page_size": 25
})

# Next page
if response["pagination"]["next_cursor"]:
    next_response = await client.call_tool("semantic_search", {
        "query": "authentication logic",
        "path": "/path/to/code",
        "cursor": response["pagination"]["next_cursor"]
    })
```

## AI Agent Best Practices

::: tip Configure ck for AI Agents
For comprehensive guidance on configuring ck for AI coding assistants like Claude Code, see [AI Agent Setup](/guide/ai-agent-setup).
:::

### Query Formulation

```python
# ✅ Good: Specific, actionable queries
await semantic_search("error handling patterns")
await semantic_search("user authentication logic")
await semantic_search("database connection pooling")

# ❌ Poor: Vague or overly broad
await semantic_search("code")
await semantic_search("functions")
```

### Result Filtering

```python
# Use threshold for quality control
results = await semantic_search(
    "caching",
    threshold=0.7  # Only high-confidence matches
)

# Limit results for performance
results = await semantic_search(
    "auth",
    top_k=10  # Top 10 results only
)
```

### Combining Search Types

```python
# Start with semantic for broad understanding
semantic_results = await semantic_search("authentication")

# Refine with hybrid for specific patterns
hybrid_results = await hybrid_search("jwt token")

# Use regex for exact strings
regex_results = await regex_search("Bearer .+")
```

## Use Cases

### Code Understanding

```python
# Find entry points
await semantic_search("main entry point")

# Understand architecture
await semantic_search("dependency injection")

# Find configuration
await semantic_search("config loading")
```

### Code Modification

```python
# Find target code
results = await semantic_search("user authentication")

# Find related tests
test_results = await semantic_search("auth test")

# Find similar patterns for consistency
similar = await semantic_search("validation pattern")
```

### Documentation Generation

```python
# Find public APIs
api_results = await semantic_search("public api")

# Extract function signatures
functions = await hybrid_search("fn |function |def ")

# Generate docs from results
docs = generate_docs(api_results)
```

### Security Analysis

```python
# Find authentication code
auth = await semantic_search("authentication")

# Find input validation
validation = await semantic_search("input sanitization")

# Find secrets (with caution)
secrets = await regex_search("api_key|password|secret")
```

## Performance Considerations

### Index Management

```python
# Check index before heavy operations
status = await index_status()
if not status["indexed"]:
    await reindex()

# Reindex after bulk changes
await git_pull()
await reindex()
```

### Pagination Strategy

```python
# For exploratory search: small pages
results = await semantic_search(query, page_size=10)

# For comprehensive search: larger pages
results = await semantic_search(query, page_size=50)

# Process incrementally to avoid memory issues
while has_more:
    page = await semantic_search(query, cursor=cursor)
    process(page["results"])
    cursor = page["pagination"]["next_cursor"]
```

### Result Caching

```python
# Cache search results for repeated queries
cache = {}
def cached_search(query):
    if query in cache:
        return cache[query]
    results = await semantic_search(query)
    cache[query] = results
    return results
```

## Error Handling

```python
try:
    results = await client.call_tool("semantic_search", {
        "query": "pattern",
        "path": "/path/to/code"
    })
except MCPError as e:
    if "index not found" in str(e):
        # Build index first
        await client.call_tool("reindex", {"path": "/path/to/code"})
        results = await client.call_tool("semantic_search", {
            "query": "pattern",
            "path": "/path/to/code"
        })
    else:
        raise
```

## Troubleshooting

### Server Not Responding

```bash
# Check if ck is installed
which ck

# Test server manually
echo '{"jsonrpc":"2.0","method":"tools/list","id":1}' | ck --serve

# Check logs (if available)
cat ~/.claude/logs/mcp.log
```

### Tool Not Available

```bash
# Verify installation
claude mcp list | grep ck

# Reinstall
claude mcp remove ck-search
claude mcp add ck-search -s user -- ck --serve
```

### Slow Searches

```python
# Reduce result count
results = await semantic_search(query, top_k=10)

# Increase threshold
results = await semantic_search(query, threshold=0.7)

# Use smaller snippets
results = await semantic_search(query, snippet_length=100)
```

## Security Considerations

- **Local execution** – All code analysis happens locally
- **No network calls** – No data sent to external services
- **File access** – MCP server has same permissions as user
- **Sandboxing** – Consider running in containerized environment

## Next Steps

- Configure [AI agent setup](/guide/ai-agent-setup) for optimal integration
- Read [semantic search](/features/semantic-search) documentation
- Learn about [hybrid search](/features/hybrid-search)
- Explore [embedding models](/reference/models)
- Check [CLI reference](/reference/cli)
