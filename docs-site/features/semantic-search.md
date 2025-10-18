---
title: Semantic Search
description: Find code by meaning using AI embeddings. Semantic search understands concepts, finds patterns, and locates related code without exact keyword matches.
---

# Semantic Search

Find code by meaning, not just keywords. Semantic search understands concepts and finds related code even when exact terms don't match.

## What is Semantic Search?

Semantic search uses AI embeddings to understand the meaning of code. It finds functionally related code based on what it does, not just what it’s named.

### Traditional Search Limitations

```bash
# grep/ripgrep can only find exact text
grep "error handling" src/  # Misses try-catch, Result returns, etc.
```

### Semantic Search Advantages

```bash
# ck understands concepts
ck --sem "error handling" src/
# Finds: try/catch, Result<T, E>, error propagation, ?operator
# Finds: exception handling, error recovery, fallback logic
```

## How It Works

1. **Indexing**: Code is parsed into semantic chunks using tree-sitter
2. **Embedding**: Each chunk is converted to a vector representation
3. **Storage**: Vectors stored in local ANN (Approximate Nearest Neighbor) index
4. **Search**: Query converted to vector, similar vectors found via cosine similarity
5. **Results**: Ranked by semantic similarity score

## Basic Usage

### Simple Semantic Search

```bash
# Find error handling patterns
ck --sem "error handling" src/

# Find authentication code
ck --sem "user authentication" src/

# Find database operations
ck --sem "SQL queries" src/
```

### With Relevance Scores

```bash
# Show how relevant each match is
ck --sem --scores "caching" src/

# Output:
# [0.847] ./cache_manager.rs: LRU cache implementation
# [0.732] ./http_client.rs: Response caching with TTL
# [0.651] ./config.rs: Configuration caching
```

Scores range from 0.0 to 1.0, with higher scores indicating stronger semantic similarity.

### Complete Code Sections

```bash
# Get entire functions/classes containing matches
ck --sem --full-section "retry logic" src/

# Returns complete syntactic units, not just matching lines
```

## Real-World Examples

### Finding Design Patterns

```bash
# Singleton pattern
ck --sem "singleton pattern" src/

# Factory pattern
ck --sem "factory" src/

# Observer pattern
ck --sem "event subscription" src/
```

### Refactoring Support

```bash
# Find duplicate logic
ck --sem "user validation" src/

# Find candidates for extraction
ck --sem "complex conditional" src/

# Find related functionality
ck --sem "payment processing" src/
```

### Security Audits

```bash
# Find authentication code
ck --sem "authentication" src/

# Find input validation
ck --sem "input sanitization" src/

# Find cryptographic operations
ck --sem "encryption" src/
```

### Understanding New Codebases

```bash
# Find entry points
ck --sem "main entry point" src/

# Understand architecture
ck --sem "dependency injection" src/

# Find configuration
ck --sem "config loading" src/
```

## Advanced Features

### Threshold Filtering

Control match quality:

```bash
# Only high-confidence matches
ck --sem --threshold 0.7 "auth" src/

# Exploratory mode (find loosely related code)
ck --sem --threshold 0.3 "pattern" src/

# Very strict matching
ck --sem --threshold 0.9 "exact concept" src/
```

::: tip Threshold Guidelines
- **0.9+**: Very strict, nearly exact conceptual matches
- **0.7-0.9**: High confidence, clearly related
- **0.5-0.7**: Moderate, good general search (default: 0.6)
- **0.3-0.5**: Exploratory, finds loosely related code
- **<0.3**: Very broad, may include irrelevant results
:::

### Result Limiting

```bash
# Top 5 most relevant results
ck --sem --topk 5 "pattern" src/

# Alternative flag
ck --sem --limit 10 "pattern" src/

# Combine with threshold
ck --sem --topk 20 --threshold 0.5 "pattern" src/
```

### Model Selection

Different models for different needs:

```bash
# Default: Fast and precise
ck --index --model bge-small .
ck --sem "pattern" src/

# Large contexts: Better for big functions
ck --index --model nomic-v1.5 .
ck --sem "pattern" src/

# Code-specialized: Understands programming concepts
ck --index --model jina-code .
ck --sem "pattern" src/
```

See [Embedding Models](/reference/models) for detailed comparison.

## Understanding Results

### Score Interpretation

```bash
$ ck --sem --scores "database connection" src/

[0.891] ./db/pool.rs:15        # Very strong match
    Connection pooling with configurable size and timeout

[0.742] ./db/client.rs:42      # Strong match
    Database client initialization and connection handling

[0.623] ./config.rs:89         # Moderate match
    Database configuration settings

[0.489] ./logging.rs:12        # Weak match (mentions "connection" in logging)
    Connection logging and metrics
```

**Score Ranges:**
- `0.85+`: Excellent match, core functionality
- `0.70-0.85`: Strong match, clearly related
- `0.60-0.70`: Good match, relevant code
- `0.50-0.60`: Fair match, potentially related
- `<0.50`: Weak match, may not be relevant

### False Positives

::: tip Understanding Semantic Matches
Semantic search finds code by meaning, which can include related concepts. Searching for "database connection" might also return "connection string" code.

**If results are too broad:**
- Increase threshold: `--threshold 0.7`
- Use hybrid search: `--hybrid "database connection"`
- Make query more specific: "database connection pool" instead of "database connection"
:::

## Language Support

Semantic search works with all supported languages:

- **Strong support** – Python, JavaScript/TypeScript, Rust, Go, Ruby, C#, Haskell, Zig
- **Text formats** – Markdown, config files, documentation
- **Binary detection** – Automatically skips non-text files

Each language uses tree-sitter for intelligent chunking at function/class boundaries.

## Performance

### Indexing

```bash
# First semantic search builds index automatically
ck --sem "pattern" src/
# ~1M LOC indexed in under 2 minutes

# Subsequent searches only process changes
ck --sem "another pattern" src/
# Delta indexing: only changed files reprocessed
```

### Search Speed

- **Typical query** – <500ms
- **Large codebases** – <1s
- **Factors** – Result count, threshold, top-k limit

### Index Size

- **Typical** – 1-3x source code size
- **Location**: `.ck/` directory (safe to delete)
- **Compression** – Efficient binary format

## Best Practices

### Query Formulation

::: tip Writing Effective Queries
Describe **what the code does** with specific concepts.

✅ **Good queries:**
- "error handling" (clear concept)
- "authentication logic" (specific functionality)
- "database connection pool" (well-defined pattern)

❌ **Poor queries:**
- "code" (too vague)
- "x" (single letter, ambiguous)
- "the main thing" (unclear concept)
:::

### When to Use Semantic Search

✅ **Use semantic search for:**
- Finding related functionality
- Exploring unfamiliar codebases
- Locating design patterns
- Refactoring candidates
- Conceptual code review

❌ **Don’t use semantic search for:**
- Exact string matches → use `ck "pattern"`
- Variable name searches → use `ck "varName"`
- Finding specific symbols → use `ck "functionName"`

### Combining with Other Tools

```bash
# Semantic search + grep filtering
ck --sem "auth" src/ | grep "Token"

# Semantic search + file type
ck --sem "validation" src/**/*.rs

# Semantic search + jq for JSON
ck --json --sem "pattern" src/ | jq '.[] | select(.score > 0.7)'
```

## Troubleshooting

### No Results Found

```bash
# Try lower threshold
ck --sem --threshold 0.3 "pattern" src/

# Try different query phrasing
ck --sem "error handling" src/
ck --sem "exception management" src/

# Use keyword search
ck "error" src/
```

### Results Too Broad

```bash
# Increase threshold
ck --sem --threshold 0.7 "pattern" src/

# Use hybrid search
ck --hybrid "pattern" src/

# Add more specific terms
ck --sem "database connection pooling" src/
```

### Slow Indexing

```bash
# Use faster model
ck --index --model bge-small .

# Exclude large directories
ck --exclude "node_modules" --exclude "target" --index .

# Index specific paths
ck --index src/ lib/
```

## Next Steps

- Try [hybrid search](/features/hybrid-search) for combined keyword + semantic
- Explore [MCP integration](/features/mcp-integration) for AI agents
- Learn about [embedding models](/reference/models)
- Check [configuration options](/reference/configuration)
