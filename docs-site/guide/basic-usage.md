---
title: Basic Usage
description: Common patterns and workflows for everyday code search with ck. Learn semantic, keyword, and hybrid search with practical examples for developers.
---

# Basic Usage

Common patterns and workflows for everyday code search.

## Search Modes

ck supports three search modes that can be used independently or combined:

### Keyword Search (Default)

Traditional regex-based search, fully compatible with grep/ripgrep:

```bash
# Basic pattern matching
ck "function.*auth" src/

# Case-insensitive
ck -i "error" src/

# Whole word match
ck -w "test" src/

# Multiple patterns
ck "TODO|FIXME|XXX" src/
```

### Semantic Search

Find code by meaning:

```bash
# Basic semantic search
ck --sem "error handling" src/

# Complete code sections
ck --sem --full-section "authentication" src/

# Limit results
ck --sem --topk 5 "database query" src/

# Show relevance scores
ck --sem --scores "retry logic" src/

# Filter by confidence
ck --sem --threshold 0.7 "cache" src/
```

### Hybrid Search

Combines semantic understanding with keyword matching:

```bash
# Best of both worlds
ck --hybrid "timeout" src/

# With relevance scores
ck --hybrid --scores "connection pool" src/

# High-confidence results only (hybrid uses RRF scores ~0.01-0.05)
ck --hybrid --threshold 0.025 "auth" src/
```

## Using Different Interfaces

While the examples above show CLI usage, ck offers multiple interfaces for different workflows.

### Terminal User Interface (TUI)

For interactive exploration, use the TUI:

```bash
# Launch interactive mode
ck-tui
```

In TUI mode:
- **Live search**: Results update as you type
- **Visual navigation**: Use ↑/↓ to browse, → for preview
- **Mode switching**: Toggle between semantic/regex/hybrid with keyboard
- **Score visualization**: Color-coded heatmaps show relevance
- **Quick file opening**: Press Enter to open in editor

**Common TUI workflows**:

```bash
# Explore unfamiliar codebase
ck-tui
# Type: "authentication"
# Navigate results, preview code
# Press Enter to open interesting files

# Refine queries interactively
ck-tui
# Start broad: "api"
# Too many results? Refine: "api endpoint handler"
# Switch to hybrid for better precision
```

See [TUI Mode](/features/tui-mode) for complete keyboard shortcuts and features.

### Editor Extension (VSCode/Cursor)

Search without leaving your editor:

```bash
1. Press Cmd+Shift+; (Ctrl+Shift+; on Windows/Linux)
2. Type query: "database connection"
3. See live results with scores
4. Click result to jump to code
```

**Common editor workflows**:

- **Search selection**: Select code, press `Cmd+Shift+'` to find similar
- **Quick lookups**: Search while debugging or reviewing code
- **Pattern discovery**: Find all implementations of a pattern

See [Editor Integration](/features/editor-integration) for setup.

### MCP Server (AI Agents)

For AI-assisted code exploration with Claude Desktop:

```bash
# Start MCP server
ck --serve

# Then in Claude Desktop:
# "Find all error handling code and explain the patterns"
```

See [MCP Integration](/features/mcp-integration) for configuration.

### Choosing the Right Interface

- **CLI**: Best for scripts, pipelines, automation
- **TUI**: Best for exploration, discovery, query refinement
- **Editor**: Best for in-editor workflow, zero context switch
- **MCP**: Best for AI-assisted understanding

Read [Choosing an Interface](/guide/choosing-interface) for detailed comparison.

## Common Flags

### Output Control

```bash
# Line numbers
ck -n "pattern" src/

# Show N lines after match
ck -A 3 "error" src/

# Show N lines before match
ck -B 2 "error" src/

# Show context (before and after)
ck -C 2 "error" src/

# Only filenames with matches
ck -l "TODO" src/

# Only filenames without matches
ck -L "test" src/

# Hide filenames
ck --no-filename "pattern" file1 file2
```

### Search Scope

```bash
# Recursive search
ck -R "pattern" .

# Specific file types
ck "pattern" **/*.rs
ck "pattern" **/*.{js,ts}

# Exclude patterns
ck --exclude "*.test.js" "pattern" src/
ck --exclude "node_modules" --exclude "dist" "pattern" .

# Ignore gitignore rules
ck --no-ignore "pattern" .

# Ignore .ckignore rules
ck --no-ckignore "pattern" .
```

### Case Sensitivity

```bash
# Case-insensitive
ck -i "warning" src/

# Case-sensitive (default)
ck "Warning" src/

# Smart case (case-insensitive if pattern is all lowercase)
# Note: Use explicit -i for now
ck -i "error" src/  # matches Error, ERROR, error
```

## Finding Code Patterns

### Error Handling

```bash
# Find error handling code
ck --sem "error handling" src/

# Find try-catch blocks
ck --sem "exception handling" src/

# Find error propagation
ck "?" src/**/*.rs  # Rust question mark operator
ck --sem "error return" src/
```

### Authentication & Authorization

```bash
# Find auth code
ck --sem "authentication" src/

# Find permission checks
ck --sem "authorization" src/

# Find session management
ck --sem "session handling" src/

# Combined keyword + semantic
ck --hybrid "jwt|token" src/
```

### Database Operations

```bash
# Find database queries
ck --sem "database query" src/

# Find connection handling
ck --sem "database connection" src/

# Find migrations
ck --sem "schema migration" migrations/

# Specific SQL operations
ck --hybrid "INSERT|UPDATE|DELETE" src/
```

### Testing

```bash
# Find test files
ck -l --sem "test" tests/

# Find unit tests
ck --sem "unit test" tests/

# Find integration tests
ck --sem "integration test" tests/

# Find tests for specific feature
ck --sem "authentication test" tests/
```

## Structured Output

### JSON Output

For scripting and automation:

```bash
# Single JSON array
ck --json --sem "pattern" src/ | jq '.file'

# Get unique files
ck --json --sem "auth" src/ | jq -r '.[].file' | sort -u

# Filter by score
ck --json --sem --scores "auth" src/ | jq '.[] | select(.score > 0.7)'
```

### JSONL Output

For streaming and AI agents:

```bash
# One JSON object per line
ck --jsonl --sem "pattern" src/

# Stream processing
ck --jsonl --sem "auth" src/ | while read -r line; do
  echo "$line" | jq '.file'
done

# Metadata only (no snippets)
ck --jsonl --no-snippet --sem "pattern" src/
```

## File Management

### Smart Exclusions

ck automatically excludes via `.ckignore`:

```bash
# View current exclusions
cat .ckignore

# Edit exclusions
vim .ckignore
```

Example `.ckignore`:
```txt
# Images and media
*.png
*.jpg
*.mp4
*.mp3

# Config files
*.json
*.yaml

# Build artifacts
target/
dist/
build/
node_modules/

# Custom patterns
temp/
*.bak
```

### Index Operations

```bash
# Check index status
ck --status .

# View index info
ck --status src/

# Add single file
ck --add new_file.rs

# Rebuild index
ck --clean .
ck --index .

# Switch embedding model
ck --switch-model nomic-v1.5 .

# Force rebuild with new model
ck --switch-model jina-code --force .

# Inspect file chunking
ck --inspect src/main.rs
```

## Performance Tips

### Faster Indexing

```bash
# Use faster model for large codebases
ck --index --model bge-small .

# Index specific directories
ck --index src/ lib/

# Exclude large directories
ck --exclude "node_modules" --exclude "target" --index .
```

### Faster Searches

```bash
# Limit results
ck --sem --topk 10 "pattern" src/

# Search specific paths
ck --sem "pattern" src/core/

# Use threshold to filter weak matches
ck --sem --threshold 0.5 "pattern" src/
```

### Memory Usage

```bash
# Smaller result snippets
ck --jsonl --snippet-length 100 --sem "pattern" src/

# Metadata only
ck --jsonl --no-snippet --sem "pattern" src/
```

## Integration Examples

### Git Hooks

```bash
# Pre-commit: Find TODOs in changed files
git diff --cached --name-only | xargs ck "TODO|FIXME"

# Find security issues in changes
git diff --name-only | xargs ck --hybrid "password|secret|key"
```

### CI/CD

```bash
# Find security keywords
ck --hybrid "api_key|password|secret" src/ && exit 1

# Verify test coverage
ck -L --sem "test" src/**/*.rs && echo "Missing tests!"

# Generate documentation
ck --jsonl --sem "public API" src/ | generate_docs.py
```

### Editor Integration

```bash
# Find definition
ck --sem "function definition $(xsel -b)" src/

# Find usage
ck "$(xsel -b)" src/

# Find similar code
ck --sem --full-section "$(xsel -b)" src/
```

## Next Steps

- Try [TUI mode](/features/tui-mode) for interactive exploration
- Install [editor extension](/features/editor-integration) for in-editor search
- Explore [advanced features](/guide/advanced-usage)
- Learn about [semantic search](/features/semantic-search) in depth
- Try [MCP integration](/features/mcp-integration) with AI agents
- Check [CLI reference](/reference/cli) for all options
- Read [Choosing an Interface](/guide/choosing-interface) for workflow guidance
