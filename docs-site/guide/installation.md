---
title: Installation
description: Install ck hybrid code search from NPM, crates.io, or source. Get started with your first search in minutes with automatic indexing and model downloads.
---

# Installation

Get up and running with ck in minutes.

## From NPM (Recommended)

```bash
npm install -g @beaconbay/ck-search
```

This installs the NPM package from [@beaconbay/ck-search](https://www.npmjs.com/package/@beaconbay/ck-search).

### Check for updates

```bash
# Check current version
npm list -g @beaconbay/ck-search

# Check if updates are available
npm outdated -g @beaconbay/ck-search

# Upgrade to the latest version
npm update -g @beaconbay/ck-search
```

## From crates.io

```bash
cargo install ck-search
```

This installs the latest stable release from [crates.io](https://crates.io/crates/ck-search).

## From Source

```bash
git clone https://github.com/BeaconBay/ck
cd ck
cargo install --path ck-cli
```

## Verify Installation

```bash
ck --version
```

## Your First Search

ck works just like grep — no configuration needed:

```bash
# Traditional keyword search
ck "TODO" src/

# Semantic search (automatically builds index on first run)
ck --sem "error handling" src/
```

::: tip First-Time Setup
The first semantic search will:
1. Detect your project structure
2. Download embedding model (one-time, ~80MB)
3. Index your codebase
4. Perform the search

Subsequent searches are fast — only changed files are re-indexed.
:::

## Quick Examples

### Semantic Search

Find code by concept:

```bash
# Find error handling patterns
ck --sem "error handling" src/

# Find authentication code
ck --sem "user authentication" src/

# Find database queries
ck --sem "SQL queries" src/

# Get complete functions
ck --sem --full-section "retry logic" src/
```

### grep-Compatible Search

All standard grep flags work:

```bash
# Case-insensitive search
ck -i "warning" src/

# Show line numbers and context
ck -n -A 3 -B 1 "error" src/

# List files with matches
ck -l "TODO" src/

# Recursive with pattern
ck -R "bug|fix" .
```

### Hybrid Search

Combine semantic and keyword search:

```bash
# Best of both worlds
ck --hybrid "connection timeout" src/

# Show relevance scores
ck --hybrid --scores "cache invalidation" src/

# Filter by confidence
ck --hybrid --threshold 0.02 "auth" src/
```

*Note: Hybrid search uses RRF (Reciprocal Rank Fusion) scores in the 0.01-0.05 range, unlike semantic search which uses 0.0-1.0.*

## Understanding the Output

### Standard Output

```bash
$ ck "error" src/main.rs
src/main.rs:42:    let result = risky_operation().map_err(|e| {
src/main.rs:43:        eprintln!("Error: {}", e);
```

### With Semantic Scores

```bash
$ ck --sem --scores "error handling" src/
[0.847] ./error_handler.rs: Comprehensive error handling with custom types
[0.732] ./main.rs: Main application with error propagation
[0.651] ./utils.rs: Utility functions with Result returns
```

Higher scores indicate stronger semantic similarity (0.0 — 1.0).

## Common Workflows

### Finding Related Code

```bash
# Find all authentication-related code
ck --sem "authentication" .

# Find test files for a feature
ck --sem "unit tests for auth" tests/

# Find configuration handling
ck --sem "config parsing" src/
```

### Code Review

```bash
# Find potential security issues
ck --hybrid "sql injection|xss" src/

# Find missing error handling
ck -L "Result|Option" src/*.rs

# Find TODOs from recent changes
git diff --name-only | xargs ck "TODO"
```

### Exploring Unfamiliar Codebases

```bash
# Understand project structure
ck --sem "main entry point" .

# Find similar functionality
ck --sem --full-section "http request handler" src/

# Locate business logic
ck --sem "payment processing" src/
```

## File Exclusions

ck automatically excludes:
- Binary files and build artifacts
- `.git` directories
- Files in `.gitignore`
- Media files (images, videos, audio)
- Common cache directories

### Custom Exclusions

```bash
# Exclude specific patterns
ck --exclude "*.test.js" --sem "api" src/

# Disable gitignore
ck --no-ignore "pattern" .

# Edit .ckignore file
vim .ckignore  # Uses gitignore syntax
```

## Index Management

ck automatically manages indexes, but you can control them:

```bash
# Check index status
ck --status .

# Force rebuild
ck --clean .

# Add single file
ck --add new_file.rs

# Inspect chunking strategy
ck --inspect src/main.rs
```

::: warning
`ck --clean` removes the entire index and requires a full rebuild. For most cases, use `ck --index` instead — it updates incrementally and is much faster.
:::

## Model Selection

Choose embedding models for different needs:

```bash
# Default: BGE-Small (fast, precise)
ck --index .

# Large contexts: Nomic V1.5
ck --index --model nomic-v1.5 .

# Code-specialized: Jina Code
ck --index --model jina-code .
```

See [Embedding Models](/reference/models) for detailed comparison.

## Choosing Your Interface

ck offers multiple interfaces for different workflows:

### Command-Line Interface (CLI)

The default mode you’ve been using — perfect for scripts and pipelines:

```bash
ck --sem "pattern" src/
```

### Terminal User Interface (TUI)

Interactive exploration with live results:

```bash
# Launch TUI mode
ck-tui

# Then:
# - Type queries and see live results
# - Navigate with ↑/↓
# - Preview with →
# - Open files with Enter
```

The TUI provides:
- Live search as you type
- Visual result previews
- Keyboard-driven navigation
- Score heatmaps
- Multiple preview modes

See [TUI Mode](/features/tui-mode) for complete documentation.

### Editor Integration (VSCode/Cursor)

Search without leaving your editor:

```bash
# Install extension (see docs for details)
code --install-extension ck-search

# Then use:
# - Cmd+Shift+; to open search
# - Cmd+Shift+' to search selection
```

See [Editor Integration](/features/editor-integration) for setup and usage.

### MCP Server (AI Agents)

Integrate with Claude Desktop and other AI tools:

```bash
# Start MCP server
ck --serve

# Then configure in Claude Desktop
```

See [MCP Integration](/features/mcp-integration) for complete setup, or [AI Agent Setup](/guide/ai-agent-setup) for configuration best practices with Claude Code and other AI coding assistants.

### Which Interface?

- **CLI**: Scripts, automation, grep replacement
- **TUI**: Interactive exploration, code discovery
- **Editor**: In-editor search, zero context switch
- **MCP**: AI-assisted code understanding

Read the full [Choosing an Interface](/guide/choosing-interface) guide for detailed comparison.

## Next Steps

- Learn [basic usage patterns](/guide/basic-usage)
- Try [TUI mode](/features/tui-mode) for interactive exploration
- Install [editor extension](/features/editor-integration) for in-editor search
- Configure [AI agent setup](/guide/ai-agent-setup) for Claude Code and AI assistants
- Explore [advanced features](/guide/advanced-usage)
- Set up [MCP integration](/features/mcp-integration)
- Check the [CLI reference](/reference/cli)

## Troubleshooting

### First Index Takes Long

First-time indexing downloads models and processes all files. Subsequent searches only process changed files.

### Model Download Fails

::: tip Model Cache Location
Models are cached in:
- Linux/macOS: `~/.cache/ck/models/`
- Windows: `%LOCALAPPDATA%\ck\cache\models\`

Ensure you have an active internet connection and ~500MB free disk space.
:::

### Search Results Seem Wrong

Try different search modes:
```bash
# Try hybrid instead of pure semantic
ck --hybrid "your query" .

# Adjust threshold
ck --sem --threshold 0.3 "query" .

# Use keyword search
ck "exact phrase" .
```

See [Configuration](/reference/configuration) for tuning options.
