---
title: AI Agent Setup
description: Configure ck for AI coding assistants like Claude Code and Cursor. Best practices for semantic search, JSONL output, and MCP integration with AI agents.
---

# Configuring ck for AI Coding Agents

Best practices for integrating ck with AI coding assistants like Claude Code, Cursor, and other agent-based development tools.

## Overview

AI coding agents benefit significantly from semantic code search capabilities. This guide shows how to configure ck to work optimally with AI assistants, providing them with powerful code understanding and navigation features.

### Benefits of ck for AI Agents

- **Conceptual search** - Agents can find code by describing functionality, not just keywords
- **Context gathering** - Quickly retrieve relevant code sections for agent analysis
- **Codebase understanding** - Help agents map out architecture and patterns
- **Precise results** - Tunable thresholds ensure high-quality matches
- **Efficient output** - Structured formats (JSON/JSONL) for easy parsing

## Claude Code Integration

Claude Code is Anthropic’s official CLI for Claude. Here’s how to configure it for optimal ck usage.

### Tool Permissions

Add ck commands to your allowed tools in `settings.local.json`:

```json
{
  "approvedCommandPatterns": [
    "Bash(ck:*)",
    "Bash(ck --index:*)",
    "Bash(ck --sem:*)",
    "Bash(ck --hybrid:*)",
    "Bash(ck --lex:*)"
  ]
}
```

This allows Claude Code to:
- Run any ck command without prompting
- Index your codebase automatically
- Perform semantic, hybrid, and lexical searches
- Access all ck features seamlessly

### Usage Patterns Documentation

Create a `.claude/` directory in your project with guidance for Claude:

```markdown
# .claude/ck-semantic-search.md

## Hybrid Code Search with ck

Use `ck` for finding code by meaning, not just keywords.

### Search Modes

- `ck --sem "concept"` - Semantic search (by meaning)
- `ck --lex "keyword"` - Lexical search (full-text)
- `ck --hybrid "query"` - Combined regex + semantic
- `ck --regex "pattern"` - Traditional regex search

### Best Practices

::: tip Recommended Usage Patterns
1. **Index once per session**: Run `ck --index .` at project start
2. **Use semantic for concepts**: "error handling", "database queries"
3. **Use lexical for names**: "getUserById", "AuthController"
4. **Tune threshold**: `--threshold 0.7` for high-confidence results
5. **Limit results**: `--limit 20` for focused output
:::

### Example Workflows

# Find authentication logic
ck --sem "user authentication" src/

# Find all TODO comments
ck --lex "TODO" .

# Find error handling patterns with high confidence
ck --sem --threshold 0.8 "error handling" src/
```

### Real-World Example

From [BeaconBay/ck#36](https://github.com/BeaconBay/ck/issues/36):

```bash
# Initial setup
ck --index .

# Semantic search for concepts
ck --sem "error handling" src/
ck --sem "database connection pooling" lib/

# High-confidence matches only
ck --sem --threshold 0.8 "authentication logic" src/

# Limited result set
ck --sem --limit 10 "caching strategy" .
```

## General AI Agent Configuration

These practices apply to any AI coding assistant using ck.

### Index Strategy

**Index once, search many times:**

```bash
# At project start or after significant changes
ck --index .

# Check index status
ck --status .

# Incremental updates happen automatically
# Manual rebuild only if needed
ck --clean . && ck --index .
```

**Why**: Indexing is expensive (1-2 minutes for 1M LOC). Searches are fast (<500ms). Let ck handle incremental updates automatically.

### Search Mode Selection

Choose the right mode for the task:

| Mode | Flag | Best For | Example |
|------|------|----------|---------|
| **Semantic** | `--sem` | Conceptual understanding | "retry logic", "input validation" |
| **Lexical** | `--lex` | Exact terms, names | "AuthService", "TODO", "FIXME" |
| **Hybrid** | `--hybrid` | Balance precision/recall | "JWT token validation" |
| **Regex** | `--regex` | Pattern matching | `class.*Handler`, `fn \w+Error` |

### Performance Tuning

#### Relevance Thresholds

Control result quality with `--threshold`:

```bash
# Exploratory search (cast wide net)
ck --sem --threshold 0.3 "pattern matching" src/

# Balanced search (default)
ck --sem --threshold 0.5 "authentication" src/

# High-confidence only (precise results)
ck --sem --threshold 0.8 "error handling" src/

# Very strict (only closest matches)
ck --sem --threshold 0.9 "exact concept" src/
```

::: tip Threshold Sweet Spot for AI Agents
Start with `0.7` for focused, high-confidence results. If you get too few matches, adjust down to `0.5` for broader coverage.
:::

#### Result Limiting

Prevent overwhelming output:

```bash
# Top 10 results (quick overview)
ck --sem --limit 10 "pattern" src/

# Top 20 results (balanced)
ck --sem --limit 20 "pattern" src/

# Top 50 results (comprehensive)
ck --sem --limit 50 "pattern" src/
```

**Recommendation for AI agents**: Use `--limit 20` by default to keep context windows manageable.

#### Pagination for Large Results

For comprehensive analysis:

```bash
# First page (25 results)
ck --sem --page-size 25 "pattern" src/

# Next page (if needed)
ck --sem --page-size 25 --cursor "abc123" "pattern" src/
```

### Output Formats for AI Consumption

#### JSONL (Recommended)

One JSON object per line - optimal for streaming and parsing:

```bash
# Full output
ck --jsonl --sem "pattern" src/

# Metadata only (smaller, faster)
ck --jsonl --no-snippet --sem "pattern" src/

# Custom snippet length
ck --jsonl --snippet-length 150 --sem "pattern" src/
```

::: tip Why JSONL for AI Agents
- **Stream-friendly**: Process results as they arrive
- **Memory-efficient**: Parse one result at a time
- **Error-resilient**: One malformed line doesn't break everything
- **Standard format**: Used by OpenAI, Anthropic, modern ML pipelines
:::

#### JSON

Single array - good for small result sets:

```bash
ck --json --sem "pattern" src/ | jq '.[].file' | sort -u
```

### Embedding Model Selection

Choose the right model for your use case:

| Model | Best For AI Agents | Trade-offs |
|-------|-------------------|------------|
| **bge-small** (default) | Quick searches, fast indexing | Smaller chunks (400 tokens) |
| **nomic-v1.5** | Large functions, documentation | Larger download (~500MB) |
| **jina-code** | Code-specialized understanding | Larger download (~500MB) |

**Recommendation**:
- Start with `bge-small` (default) - works well for most codebases
- Switch to `jina-code` if agent struggles with code-specific concepts
- Use `nomic-v1.5` for documentation-heavy projects

```bash
# Switch models
ck --switch-model jina-code .
```

## Recommended .ckignore for AI Agents

Optimize indexing for AI-assisted development:

```txt
# .ckignore

# Build artifacts (not useful for understanding code)
target/
dist/
build/
*.o
*.so
*.dylib

# Dependencies (too large, low signal)
node_modules/
vendor/
venv/
.venv/

# Media files (not code)
*.png
*.jpg
*.gif
*.mp4
*.pdf

# Large data files
*.csv
*.json
*.xml
*.log

# Test fixtures (unless you search them)
fixtures/
__snapshots__/
*.snap

# Generated code (if not relevant)
*_pb2.py
*.generated.*

# Documentation (include if you want AI to reference docs)
# docs/
# *.md
```

**Key principle**: Exclude anything that doesn’t help the AI understand your codebase’s logic and architecture.

## Common Workflows

### Codebase Onboarding

Help AI agents understand new projects:

```bash
# Index the project
ck --index .

# Find entry points
ck --sem "main entry point" .
ck --sem "application initialization" .

# Understand architecture
ck --sem "dependency injection" src/
ck --sem "data flow" src/

# Map out key components
ck --sem "authentication" .
ck --sem "database queries" .
ck --sem "API endpoints" .
```

### Code Review Assistance

Find code patterns for review:

```bash
# Security checks
ck --sem "authentication logic" src/
ck --sem "input validation" src/
ck --sem "SQL queries" src/

# Code quality
ck --lex "TODO" .
ck --lex "FIXME" .
ck --sem "error handling" src/

# Performance
ck --sem "caching" src/
ck --sem "database connection" src/
```

### Refactoring Support

Find code to refactor:

```bash
# Find all implementations of a pattern
ck --sem "user validation" src/

# Find similar code for consistency
ck --sem "error response formatting" src/

# Find candidates for extraction
ck --sem --threshold 0.8 "duplicate logic" src/
```

### Documentation Generation

Gather code for documentation:

```bash
# Find public APIs
ck --sem "public api" src/

# Find configuration
ck --sem "configuration options" .

# Extract examples
ck --sem "example usage" tests/
```

## MCP Integration

For MCP-compatible AI agents (Claude Desktop, etc.), use the MCP server:

### Setup

```bash
# Start MCP server
ck --serve

# Configure in Claude Desktop
claude mcp add ck-search -s user -- ck --serve
```

### MCP-Specific Best Practices

When using ck via MCP protocol:

1. **Use pagination** - Large result sets are paginated automatically
2. **Set reasonable page_size** - Default 25 is good for most cases
3. **Use threshold parameter** - Filter results for relevance
4. **Check index status** - Call `index_status` tool before heavy searches
5. **Reindex after bulk changes** - Call `reindex` tool after git pull

See [MCP Integration](/features/mcp-integration) for complete API reference.

## Troubleshooting

### Agent Gets Irrelevant Results

**Solution 1**: Increase threshold

```bash
ck --sem --threshold 0.8 "your query" src/
```

**Solution 2**: Switch to hybrid search

```bash
ck --hybrid "your query" src/
```

**Solution 3**: Try a different model

```bash
ck --switch-model jina-code .
```

### Agent Searches Take Too Long

**Solution 1**: Limit results

```bash
ck --sem --limit 10 "query" src/
```

**Solution 2**: Exclude large directories

```bash
# Add to .ckignore
node_modules/
vendor/
dist/
```

**Solution 3**: Use metadata-only output

```bash
ck --jsonl --no-snippet --sem "query" src/
```

### Agent Misses Obvious Matches

**Solution 1**: Lower threshold

```bash
ck --sem --threshold 0.3 "query" src/
```

**Solution 2**: Use hybrid search

```bash
ck --hybrid "query" src/
```

**Solution 3**: Use lexical search for exact terms

```bash
ck --lex "ExactFunctionName" src/
```

### Near-Miss Threshold Hints

**Feature**: When no results are found above the specified threshold, ck provides helpful "near-miss" hints.

**How it helps AI agents**:
When semantic search finds no matches above your threshold, ck will show the closest match that fell just below the threshold, along with its score. This gives AI agents a clear signal to adjust the threshold parameter intelligently.

**Example output**:
```bash
$ ck --sem --threshold 0.7 "retry logic" src/

No matches found above threshold 0.7

Near-miss:
  src/network/client.rs:156 (score: 0.68)
  "Implements exponential backoff for failed requests"

Hint: Try lowering threshold to 0.65 or use --threshold 0.6
```

**AI agent workflow**:
1. Initial search with `--threshold 0.7` returns no results
2. ck shows near-miss at 0.68
3. Agent automatically retries with `--threshold 0.65`
4. Finds relevant matches

This feedback loop helps agents converge on optimal thresholds without overshooting.

::: tip Why This Matters for AI Agents
AI agents naturally understand threshold tuning. Near-miss hints provide actionable feedback, allowing agents to adjust parameters intelligently rather than guessing or giving up. This is one of ck's "nice affordances" for AI-first usage.
:::

### Index Out of Date

**Solution**: Check status and reindex if needed

```bash
ck --status .
# If stale:
ck --index .
```

## Performance Benchmarks

Typical performance for AI agent workflows:

| Operation | Time | Notes |
|-----------|------|-------|
| Initial index (100K LOC) | ~10s | One-time cost |
| Semantic search | <500ms | Cached index |
| Hybrid search | <300ms | Leverages both indices |
| Lexical search | <100ms | Full-text index |
| Incremental update | <1s | Only changed files |

## Best Practices Summary

✅ **Do**:
- Index once at project start
- Use `--threshold 0.7` as default for AI agents
- Use `--limit 20` to keep context manageable
- Prefer JSONL output for parsing
- Use semantic search for concepts
- Use lexical search for exact identifiers
- Tune thresholds based on result quality

❌ **Don’t**:
- Re-index unnecessarily (ck handles incremental updates)
- Use threshold above 0.9 (too restrictive)
- Use threshold below 0.3 (too noisy)
- Omit `--limit` for exploratory searches (too many results)
- Index huge binary/media/data files
- Mix search modes without understanding differences

## Next Steps

- Read [MCP Integration](/features/mcp-integration) for API details
- See [Choosing an Interface](/guide/choosing-interface) for workflow comparison
- Check [Semantic Search](/features/semantic-search) for search fundamentals
- Review [Advanced Usage](/guide/advanced-usage) for power-user techniques
- Explore [CLI Reference](/reference/cli) for all flags and options
