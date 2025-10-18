---
title: Advanced Usage
description: Power-user features for ck including model selection, complete code sections, custom exclusions, JSON/JSONL output, and CI/CD integration patterns.
---

# Advanced Usage

Power-user features and advanced workflows for ck.

## Model Selection

Choose the right embedding model for your needs.

### Available Models

```bash
# BGE-Small (default) - Fast, precise, 400-token chunks
ck --index --model bge-small .

# Nomic V1.5 - Large contexts, 1024-token chunks, 8K capacity
ck --index --model nomic-v1.5 .

# Jina Code - Code-specialized, 1024-token chunks, 8K capacity
ck --index --model jina-code .
```

### Model Comparison

| Model | Chunk Size | Context Window | Best For |
|-------|------------|----------------|----------|
| `bge-small` | 400 tokens | 512 tokens | General code, fast indexing |
| `nomic-v1.5` | 1024 tokens | 8K tokens | Large functions, documentation |
| `jina-code` | 1024 tokens | 8K tokens | Code-specific understanding |

### Switching Models

```bash
# Switch to different model
ck --switch-model nomic-v1.5 .

# Force rebuild (if unsure about index state)
ck --switch-model jina-code --force .

# Check current model
ck --status .
```

## Complete Code Sections

Extract entire functions, classes, or modules:

```bash
# Get full functions containing matches
ck --sem --full-section "database query" src/

# Works with regex too
ck --full-section "class.*Handler" src/

# Combine with other flags
ck --sem --full-section --scores "authentication" src/
```

This uses tree-sitter parsing to return complete syntactic units.

## Advanced Filtering

### Relevance Thresholds

```bash
# Only high-confidence matches (0.7+)
ck --sem --threshold 0.7 "auth" src/

# Exploratory search (0.3+)
ck --sem --threshold 0.3 "pattern" src/

# Very strict (0.9+)
ck --sem --threshold 0.9 "exact concept" src/
```

Scores range from 0.0 to 1.0, with higher being more relevant.

### Result Limiting

```bash
# Top 5 results
ck --sem --topk 5 "pattern" src/

# Alternative flag name
ck --sem --limit 10 "pattern" src/

# Combine with threshold
ck --sem --topk 20 --threshold 0.5 "pattern" src/
```

### Pagination for MCP/Agents

```bash
# First page (25 results)
ck --sem --page-size 25 "pattern" src/

# Get specific page
ck --sem --page-size 25 --cursor "abc123" "pattern" src/
```

Used primarily by MCP server for large result sets.

## Chunking Inspection

Understand how ck processes your files:

```bash
# Inspect file chunking
ck --inspect src/main.rs

# See token counts per chunk
ck --inspect src/large_file.py

# Test different models
ck --inspect --model nomic-v1.5 src/main.rs
```

Output shows:
- Detected language
- Number of chunks
- Token count per chunk
- Chunk boundaries

## Custom Exclusions

### .ckignore Syntax

Uses gitignore-style patterns:

```txt
# Exclude directory and all contents
node_modules/
target/
dist/

# Exclude file patterns
*.log
*.tmp
*.bak

# Exclude specific files
config/secrets.yaml
.env*

# Negation (include despite parent exclusion)
!important.log
```

### Multiple Exclusion Layers

ck combines exclusions from multiple sources:

```bash
# Default exclusions + .gitignore + .ckignore + CLI
ck --exclude "temp/" "pattern" src/

# Skip .gitignore
ck --no-ignore --exclude "temp/" "pattern" src/

# Skip .ckignore
ck --no-ckignore --exclude "temp/" "pattern" src/

# Skip both (only CLI + defaults)
ck --no-ignore --no-ckignore --exclude "temp/" "pattern" src/
```

All exclusion layers are additive.

## Index Management

### Incremental Updates

ck automatically updates indexes incrementally:

```bash
# First search: full index
ck --sem "pattern" src/

# Subsequent searches: only changed files
ck --sem "another pattern" src/
```

Uses file hashing to detect changes.

### Manual Index Control

```bash
# Force full rebuild
ck --clean .
ck --index .

# Add single file to existing index
ck --add src/new_file.rs

# Check what needs updating
ck --status .
```

### Index Location

Indexes stored in `.ck/` directories:

```bash
project/
├── src/
├── .ck/              # Safe to delete anytime
│   ├── embeddings.json
│   ├── ann_index.bin
│   └── tantivy_index/
└── .ckignore
```

The `.ck/` directory is a cache and can be safely deleted.

## Structured Output for Automation

### JSON Output

Single JSON array:

```bash
# Basic JSON
ck --json --sem "pattern" src/

# Parse with jq
ck --json --sem "auth" src/ | jq -r '.[].file' | sort -u

# Filter by score
ck --json --sem --scores "pattern" src/ | jq '.[] | select(.score > 0.7)'

# Extract specific fields
ck --json --sem "pattern" src/ | jq '.[] | {file, line, score}'
```

### JSONL Output (Recommended for Agents)

One JSON object per line:

```bash
# JSONL format
ck --jsonl --sem "pattern" src/

# Stream processing
ck --jsonl --sem "pattern" src/ | while read -r line; do
  echo "$line" | jq '.file'
done

# Metadata only (no snippets, smaller output)
ck --jsonl --no-snippet --sem "pattern" src/

# Custom snippet length
ck --jsonl --snippet-length 150 --sem "pattern" src/
```

Why JSONL for AI agents:
- ✅ Stream-friendly: Process results as they arrive
- ✅ Memory-efficient: Parse one result at a time
- ✅ Error-resilient: One malformed line doesn’t break entire response
- ✅ Standard format: Used by OpenAI, Anthropic, modern ML pipelines

## Language Support

### Supported Languages

| Language | Tree-sitter | Semantic Chunking |
|----------|-------------|-------------------|
| Python | ✅ | Functions, classes |
| JavaScript/TypeScript | ✅ | Functions, classes, methods |
| Rust | ✅ | Functions, structs, traits |
| Go | ✅ | Functions, types, methods |
| Ruby | ✅ | Classes, methods, modules |
| Haskell | ✅ | Functions, types, instances |
| C# | ✅ | Classes, interfaces, methods |
| Zig | ✅ | Functions, structs |

Text formats (Markdown, JSON, YAML, TOML, XML, HTML, CSS, shell scripts, SQL) are also supported with content-based chunking.

### Binary Detection

ck uses ripgrep-style content analysis:
- Checks first 8KB of file for NUL bytes
- Automatically indexes text files regardless of extension
- Correctly excludes binary files

## CI/CD Integration

### Exit Codes

```bash
# Exit 0 if matches found, 1 if not
ck "pattern" src/
echo $?  # 0 = found, 1 = not found

# Use in scripts
if ck --hybrid "security issue" src/; then
  echo "Security issues found!"
  exit 1
fi
```

### Pre-commit Hooks

`.git/hooks/pre-commit`:
```bash
#!/bin/bash

# Find TODOs in staged files
if git diff --cached --name-only | xargs ck "TODO|FIXME|XXX"; then
  echo "Warning: Found TODOs in staged files"
  read -p "Commit anyway? (y/n) " -n 1 -r
  echo
  if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    exit 1
  fi
fi

# Check for secrets
if git diff --cached --name-only | xargs ck -i "api_key|password|secret"; then
  echo "Error: Potential secrets found!"
  exit 1
fi
```

### GitHub Actions

```yaml
name: Code Search
on: [pull_request]

jobs:
  search:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install ck
        run: cargo install ck-search

      - name: Search for security issues
        run: |
          ck --hybrid "sql injection|xss|csrf" src/ && exit 1 || true

      - name: Find missing tests
        run: |
          ck -L --sem "test" src/**/*.rs && exit 1 || true
```

## Performance Tuning

### Indexing Performance

```bash
# Use smaller model
ck --index --model bge-small .

# Exclude large directories
ck --exclude "node_modules" --exclude "target" --index .

# Index specific paths only
ck --index src/ lib/ tests/
```

### Search Performance

```bash
# Limit result count
ck --sem --topk 10 "pattern" src/

# Use threshold to reduce computation
ck --sem --threshold 0.5 "pattern" src/

# Search specific directories
ck --sem "pattern" src/core/
```

### Memory Optimization

```bash
# Smaller snippets in JSON output
ck --jsonl --snippet-length 100 --sem "pattern" src/

# Metadata only
ck --jsonl --no-snippet --sem "pattern" src/
```

## Interrupt Handling

All long-running operations can be safely interrupted:

```bash
# Start indexing
ck --index .

# Press Ctrl+C to stop

# Resume later (continues from where it stopped)
ck --index .
```

Partial indexes are saved, and subsequent operations resume from the checkpoint.

## Next Steps

- Deep dive into [semantic search](/features/semantic-search)
- Learn about [MCP integration](/features/mcp-integration)
- Explore [embedding models](/reference/models)
- Check [CLI reference](/reference/cli) for all options
