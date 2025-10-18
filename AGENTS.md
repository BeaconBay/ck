# ck for AI Coding Assistants

Quick reference for AI agents using ck semantic code search.

## Quick Start

```bash
# Index the project (run once at start)
ck --index .

# Semantic search (find by meaning)
ck --sem "error handling" src/

# Check index status
ck --status .
```

## Search Modes

| Mode | Flag | Use When | Example |
|------|------|----------|---------|
| Semantic | `--sem` | Finding concepts, patterns | "retry logic", "input validation" |
| Lexical | `--lex` | Finding exact terms, names | "getUserById", "TODO" |
| Hybrid | `--hybrid` | Balancing precision/recall | "JWT token validation" |
| Regex | `--regex` | Pattern matching | `class.*Handler` |

## Essential Flags

### Threshold (quality control)
```bash
--threshold 0.7   # High-confidence (recommended default)
--threshold 0.5   # Balanced (default if not specified)
--threshold 0.3   # Exploratory (cast wide net)
```

### Result Limiting
```bash
--limit 20        # Recommended default for AI agents
--limit 10        # Quick overview
--limit 50        # Comprehensive analysis
```

### Output Formats
```bash
--jsonl           # Recommended: stream-friendly, one object per line
--json            # Single array (good for small results)
--no-snippet      # Metadata only (faster, smaller)
--snippet-length 150  # Custom snippet size
```

### Show Scores
```bash
--scores          # Display relevance scores with results
```

## Command Patterns

### Codebase Onboarding
```bash
ck --index .
ck --sem "main entry point" .
ck --sem "application initialization" .
ck --sem "authentication" .
ck --sem "database queries" .
```

### Code Search
```bash
# Semantic search with high confidence
ck --sem --threshold 0.7 --limit 20 "error handling" src/

# Lexical search for exact terms
ck --lex "TODO" .

# Hybrid search for balanced results
ck --hybrid --limit 20 "connection pooling" src/

# Get structured output
ck --jsonl --sem --threshold 0.7 "pattern" src/
```

### Code Review
```bash
# Find security patterns
ck --sem "authentication logic" src/
ck --sem "input validation" src/

# Find code quality issues
ck --lex "TODO" .
ck --lex "FIXME" .

# Find performance patterns
ck --sem "caching" src/
ck --sem "database connection" src/
```

### Refactoring
```bash
# Find all implementations of a pattern
ck --sem --threshold 0.8 "user validation" src/

# Find similar code
ck --sem "error response formatting" src/

# Find duplicate logic
ck --sem --threshold 0.8 "duplicate logic" src/
```

## Performance Guidelines

### Optimal Defaults for AI Agents
```bash
ck --sem --threshold 0.7 --limit 20 "query" src/
```

### Index Management
- **Index once** per session at project start: `ck --index .`
- Incremental updates happen automatically
- Only rebuild if index is stale: `ck --clean . && ck --index .`

### Speed Tips
```bash
# Use limit to control result count
ck --sem --limit 10 "query" src/

# Use no-snippet for faster parsing
ck --jsonl --no-snippet --sem "query" src/

# Check index before heavy operations
ck --status .
```

## Common Workflows

### Pattern 1: Understanding New Codebase
```bash
ck --index .
ck --sem "main entry point" .
ck --sem "configuration" .
ck --sem "authentication" .
ck --sem "database" .
ck --sem "API endpoints" .
```

### Pattern 2: Finding Specific Code
```bash
# Use semantic for concepts
ck --sem --threshold 0.7 "JWT authentication" src/

# Use lexical for exact names
ck --lex "authenticateUser" src/

# Use hybrid for balance
ck --hybrid "user session" src/
```

### Pattern 3: Structured Output for Parsing
```bash
# JSONL output (recommended)
ck --jsonl --sem --threshold 0.7 --limit 20 "pattern" src/

# Metadata only (no code snippets)
ck --jsonl --no-snippet --sem "pattern" src/
```

## Troubleshooting

### Irrelevant Results
```bash
# Increase threshold
ck --sem --threshold 0.8 "query" src/

# Switch to hybrid
ck --hybrid "query" src/

# Use lexical for exact terms
ck --lex "ExactFunctionName" src/
```

### Too Few Results
```bash
# Lower threshold
ck --sem --threshold 0.3 "query" src/

# Increase limit
ck --sem --limit 50 "query" src/

# Try hybrid mode
ck --hybrid "query" src/
```

### Slow Searches
```bash
# Limit results
ck --sem --limit 10 "query" src/

# Use metadata only
ck --jsonl --no-snippet --sem "query" src/

# Check index status
ck --status .
```

### Index Out of Date
```bash
ck --status .
ck --index .  # If stale
```

## Model Selection

Default model (`bge-small`) works well for most cases. Switch only if needed:

```bash
# Code-specialized model
ck --switch-model jina-code .

# Large context model
ck --switch-model nomic-v1.5 .
```

## Output Formats Explained

### Human-Readable (default)
```bash
ck --sem "pattern" src/
```

### JSONL (recommended for agents)
```bash
ck --jsonl --sem "pattern" src/
# Output: one JSON object per line
# {"file": "...", "line": 42, "content": "...", "score": 0.847}
```

### JSON
```bash
ck --json --sem "pattern" src/
# Output: single JSON array
# [{"file": "...", "line": 42, ...}, ...]
```

## Best Practices Checklist

✅ **Do**:
- Index once at project start: `ck --index .`
- Use `--threshold 0.7` as default
- Use `--limit 20` to manage context windows
- Use `--jsonl` for structured output
- Use semantic search for concepts
- Use lexical search for exact identifiers

❌ **Don’t**:
- Re-index unnecessarily (automatic incremental updates)
- Use threshold > 0.9 (too restrictive)
- Use threshold < 0.3 (too noisy)
- Skip `--limit` on exploratory searches (overwhelming output)

## Performance Benchmarks

| Operation | Time | Notes |
|-----------|------|-------|
| Initial index (100K LOC) | ~10s | One-time cost |
| Semantic search | <500ms | After initial index |
| Hybrid search | <300ms | Leverages both indices |
| Lexical search | <100ms | Full-text index only |
| Incremental update | <1s | Only changed files |

## MCP Integration

If using ck via MCP protocol (Claude Desktop, etc.):

```bash
# Start MCP server
ck --serve

# Configure in Claude Desktop
claude mcp add ck-search -s user -- ck --serve
```

MCP-specific notes:
- Use pagination for large results (automatic)
- Default page size of 25 is good for most cases
- Use threshold parameter to filter results
- Check index status before heavy searches

## Complete Example Session

```bash
# 1. Index the project
ck --index .

# 2. Check index status
ck --status .

# 3. Find authentication code
ck --sem --threshold 0.7 --limit 20 "authentication" src/

# 4. Get structured output for parsing
ck --jsonl --sem --threshold 0.7 --limit 20 "authentication" src/

# 5. Find all TODOs
ck --lex "TODO" .

# 6. Find error handling patterns
ck --sem --threshold 0.7 "error handling" src/

# 7. Get complete code sections
ck --sem --full-section "retry logic" src/
```

## Quick Command Reference

```bash
# Basic searches
ck --sem "concept" src/                    # Semantic
ck --lex "keyword" src/                    # Lexical
ck --hybrid "query" src/                   # Hybrid
ck --regex "pattern" src/                  # Regex

# With quality control
ck --sem --threshold 0.7 "concept" src/    # High confidence
ck --sem --limit 20 "concept" src/         # Limit results
ck --sem --scores "concept" src/           # Show scores

# Structured output
ck --jsonl --sem "concept" src/            # JSONL format
ck --json --sem "concept" src/             # JSON format
ck --jsonl --no-snippet --sem "concept" src/  # Metadata only

# Index management
ck --index .                               # Build index
ck --status .                              # Check status
ck --clean .                               # Remove index
ck --inspect src/file.rs                   # Inspect chunking

# Advanced
ck --sem --full-section "pattern" src/     # Complete code sections
ck --switch-model jina-code .              # Change embedding model
```

## Additional Resources

For complete documentation, visit: https://beaconbay.github.io/ck/
