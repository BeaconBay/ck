# ck - Semantic Code Search

**ck (seek)** finds code by meaning, not just keywords. It's grep that understands what you're looking for — search for "error handling" and find try/catch blocks, error returns, and exception handling code even when those exact words aren't present.

## 🚀 Quick Start

```bash
# Install from crates.io
cargo install ck-search

# Just search — ck builds and updates indexes automatically
ck --sem "error handling" src/
ck --sem "authentication logic" src/
ck --sem "database connection pooling" src/

# Traditional grep-compatible search still works
ck -n "TODO" *.rs
ck -R "TODO|FIXME" .

# Combine both: semantic relevance + keyword filtering
ck --hybrid "connection timeout" src/
```

## ✨ Headline Features

### 🤖 **AI Agent Integration (MCP Server)**
Connect ck directly to Claude Desktop, Cursor, or any MCP-compatible AI client for seamless code search integration:

```bash
# Start MCP server for AI agent integration
ck --serve
```

**Claude Desktop Setup:**

```bash
# Install via Claude Code CLI (recommended)
claude mcp add ck-search -s user -- ck --serve

# Note: You may need to restart Claude Code after installation
# Verify installation with:
claude mcp list  # or use /mcp in Claude Code
```

**Manual Configuration (alternative):**
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

**Tool Permissions:** When prompted by Claude Code, approve permissions for ck-search tools (semantic_search, regex_search, hybrid_search, etc.)

**Available MCP Tools:**
- `semantic_search` - Find code by meaning using embeddings
- `regex_search` - Traditional grep-style pattern matching
- `hybrid_search` - Combined semantic and keyword search
- `index_status` - Check indexing status and metadata
- `reindex` - Force rebuild of search index
- `health_check` - Server status and diagnostics

**Built-in Pagination:** Handles large result sets gracefully with page_size controls, cursors, and snippet length management.

### 🔍 **Semantic Search**
Find code by concept, not keywords. Understands synonyms, related terms, and conceptual similarity:

```bash
# These find related code even without exact keywords:
ck --sem "retry logic"           # finds backoff, circuit breakers
ck --sem "user authentication"   # finds login, auth, credentials
ck --sem "data validation"       # finds sanitization, type checking

# Get complete functions/classes containing matches
ck --sem --full-section "error handling"  # returns entire functions
```

### ⚡ **Drop-in grep Compatibility**
All your muscle memory works. Same flags, same behavior, same output format:

```bash
ck -i "warning" *.log              # Case-insensitive
ck -n -A 3 -B 1 "error" src/       # Line numbers + context
ck -l "error" src/                  # List files with matches only
ck -L "TODO" src/                   # List files without matches
ck -R --exclude "*.test.js" "bug"  # Recursive with exclusions
```

### 🎯 **Hybrid Search**
Combine keyword precision with semantic understanding using Reciprocal Rank Fusion:

```bash
ck --hybrid "async timeout" src/    # Best of both worlds
ck --hybrid --scores "cache" src/   # Show relevance scores with color highlighting
ck --hybrid --threshold 0.02 query  # Filter by minimum relevance
```

### ⚙️ **Automatic Delta Indexing**
Semantic and hybrid searches transparently create and refresh their indexes before running. The first search builds what it needs; subsequent searches only touch files that changed.

### 📁 **Smart File Filtering**
Automatically excludes cache directories, build artifacts, and respects `.gitignore` and `.ckignore` files:

```bash
# ck respects multiple exclusion layers (all are additive):
ck "pattern" .                           # Uses .gitignore + .ckignore + defaults
ck --no-ignore "pattern" .               # Skip .gitignore (still uses .ckignore)
ck --no-ckignore "pattern" .             # Skip .ckignore (still uses .gitignore)
ck --exclude "dist" --exclude "logs" .   # Add custom exclusions

# .ckignore file (created automatically on first index):
# - Excludes images, videos, audio, binaries, archives by default
# - Excludes JSON/YAML config files (issue #27)
# - Uses same syntax as .gitignore (glob patterns, ! for negation)
# - Persists across searches (issue #67)
# - Located at repository root, editable for custom patterns

# Exclusion patterns use .gitignore syntax:
ck --exclude "node_modules" .            # Exclude directory and all contents
ck --exclude "*.test.js" .                # Exclude files matching pattern
ck --exclude "build/" --exclude "*.log" . # Multiple exclusions
# Note: Patterns are relative to the search root
```

**Why .ckignore?** While `.gitignore` handles version control exclusions, many files that *should* be in your repo aren't ideal for semantic search. Config files (`package.json`, `tsconfig.json`), images, videos, and data files add noise to search results and slow down indexing. `.ckignore` lets you focus semantic search on actual code while keeping everything else in git. Think of it as "what should I search" vs "what should I commit".

## 🛠 Advanced Usage

### AI Agent Integration

#### MCP Server (Recommended)
```python
# Example usage in AI agents
response = await client.call_tool("semantic_search", {
    "query": "authentication logic",
    "path": "/path/to/code",
    "page_size": 25,
    "top_k": 50,           # Limit total results (default: 100 for MCP)
    "snippet_length": 200
})

# Handle pagination
if response["pagination"]["next_cursor"]:
    next_response = await client.call_tool("semantic_search", {
        "query": "authentication logic",
        "path": "/path/to/code",
        "cursor": response["pagination"]["next_cursor"]
    })
```

#### JSONL Output (Custom Workflows)
Perfect structured output for LLMs, scripts, and automation:

```bash
# JSONL format - one JSON object per line (recommended for agents)
ck --jsonl --sem "error handling" src/
ck --jsonl --no-snippet "function" .        # Metadata only
ck --jsonl --topk 5 --threshold 0.7 "auth"  # High-confidence results

# Traditional JSON (single array)
ck --json --sem "error handling" src/ | jq '.file'
```

**Why JSONL for AI agents?**
- ✅ **Streaming friendly**: Process results as they arrive
- ✅ **Memory efficient**: Parse one result at a time
- ✅ **Error resilient**: One malformed line doesn't break entire response
- ✅ **Standard format**: Used by OpenAI API, Anthropic API, and modern ML pipelines

### Search & Filter Options

```bash
# Threshold filtering
ck --sem --threshold 0.7 "query"           # Only high-confidence matches
ck --hybrid --threshold 0.01 "concept"     # Low-confidence (exploration)

# Limit results
ck --sem --topk 5 "authentication patterns"

# Complete code sections
ck --sem --full-section "database queries"  # Complete functions
ck --full-section "class.*Error" src/       # Complete classes (works with regex too)

# Relevance scoring
ck --sem --scores "machine learning" docs/
# [0.847] ./ai_guide.txt: Machine learning introduction...
# [0.732] ./statistics.txt: Statistical learning methods...
```


### Language Coverage

| Language | Indexing | Chunking | AST-aware | Notes |
|----------|----------|----------|-----------|-------|
| Zig | ✅ | ✅ | ✅ | contributed by [@Nevon](https://github.com/Nevon) (PR #72) |

### Model Selection

Choose the right embedding model for your needs:

```bash
# Default: BGE-Small (fast, precise chunking)
ck --index .

# Enhanced: Nomic V1.5 (8K context, optimal for large functions)
ck --index --model nomic-v1.5 .

# Code-specialized: Jina Code (optimized for programming languages)
ck --index --model jina-code .
```

**Model Comparison:**
- **`bge-small`** (default): 400-token chunks, fast indexing, good for most code
- **`nomic-v1.5`**: 1024-token chunks with 8K model capacity, better for large functions
- **`jina-code`**: 1024-token chunks with 8K model capacity, specialized for code understanding

### Index Management

```bash
# Check index status
ck --status .

# Clean up and rebuild / switch models
ck --clean .
ck --switch-model nomic-v1.5 .
ck --switch-model nomic-v1.5 --force .     # Force rebuild

# Add single file to index
ck --add new_file.rs

# File inspection (analyze chunking and token usage)
ck --inspect src/main.rs
ck --inspect --model bge-small src/main.rs  # Test different models
```

**Interrupting Operations:** Indexing can be safely interrupted with Ctrl+C. The partial index is saved, and the next operation will resume from where it stopped, only processing new or changed files.

## 📚 Language Support

| Language | Indexing | Tree-sitter Parsing | Semantic Chunking |
|----------|----------|-------------------|------------------|
| Python | ✅ | ✅ | ✅ Functions, classes |
| JavaScript/TypeScript | ✅ | ✅ | ✅ Functions, classes, methods |
| Rust | ✅ | ✅ | ✅ Functions, structs, traits |
| Go | ✅ | ✅ | ✅ Functions, types, methods |
| Ruby | ✅ | ✅ | ✅ Classes, methods, modules |
| Haskell | ✅ | ✅ | ✅ Functions, types, instances |
| C# | ✅ | ✅ | ✅ Classes, interfaces, methods |

**Text Formats:** Markdown, JSON, YAML, TOML, XML, HTML, CSS, shell scripts, SQL, log files, config files, and any other text format.

**Smart Binary Detection:** Uses ripgrep-style content analysis, automatically indexing any text file while correctly excluding binary files.

**Unsupported File Types:** Text files with unrecognized extensions (like `.org`, `.adoc`, etc.) are automatically indexed as plain text. ck detects text vs binary based on file contents, not extensions.

## 🏗 Installation

### From crates.io
```bash
cargo install ck-search
```

### From Source
```bash
git clone https://github.com/BeaconBay/ck
cd ck
cargo install --path ck-cli
```

### Package Managers
```bash
# Currently available:
cargo install ck-search    # ✅ Available now via crates.io

# Coming soon:
brew install ck-search     # 🚧 In development (use cargo for now)
apt install ck-search      # 🚧 In development
```

## 💡 Examples

### Finding Code Patterns
```bash
# Find authentication/authorization code
ck --sem "user permissions" src/
ck --sem "access control" src/
ck --sem "login validation" src/

# Find error handling strategies
ck --sem "exception handling" src/
ck --sem "error recovery" src/
ck --sem "fallback mechanisms" src/

# Find performance-related code
ck --sem "caching strategies" src/
ck --sem "database optimization" src/
ck --sem "memory management" src/
```

### Team Workflows
```bash
# Find related test files
ck --sem "unit tests for authentication" tests/
ck -l --sem "test" tests/           # List test files by semantic content

# Identify refactoring candidates
ck --sem "duplicate logic" src/
ck --sem "code complexity" src/
ck -L "test" src/                   # Find source files without tests

# Security audit
ck --hybrid "password|credential|secret" src/
ck --sem "input validation" src/
```

### Integration Examples
```bash
# Git hooks
git diff --name-only | xargs ck --sem "TODO"

# CI/CD pipeline
ck --json --sem "security vulnerability" . | security_scanner.py

# Code review prep
ck --hybrid --scores "performance" src/ > review_notes.txt

# Documentation generation
ck --json --sem "public API" src/ | generate_docs.py
```

## ⚡ Performance

**Field-tested on real codebases:**

- **Indexing:** ~1M LOC in under 2 minutes
- **Search:** Sub-500ms queries on typical codebases
- **Index size:** ~2x source code size with compression
- **Memory:** Efficient streaming for large repositories
- **Token precision:** HuggingFace tokenizers for exact model-specific token counting

## 🔧 Architecture

ck uses a modular Rust workspace:

- **`ck-cli`** - Command-line interface and MCP server
- **`ck-core`** - Shared types, configuration, and utilities
- **`ck-engine`** - Search engine implementations (regex, semantic, hybrid)
- **`ck-index`** - File indexing, hashing, and sidecar management
- **`ck-embed`** - Text embedding providers (FastEmbed, API backends)
- **`ck-ann`** - Approximate nearest neighbor search indices
- **`ck-chunk`** - Text segmentation and language-aware parsing
- **`ck-models`** - Model registry and configuration management

### Index Storage

Indexes are stored in `.ck/` directories alongside your code:

```
project/
├── src/
├── docs/
└── .ck/           # Semantic index (can be safely deleted)
    ├── embeddings.json
    ├── ann_index.bin
    └── tantivy_index/
```

The `.ck/` directory is a cache — safe to delete and rebuild anytime.

## 🧪 Testing

```bash
# Run the full test suite
cargo test --workspace

# Test with each feature combination
cargo hack test --each-feature --workspace
```

## 🤝 Contributing

ck is actively developed and welcomes contributions:

1. **Issues:** Report bugs, request features
2. **Code:** Submit PRs for bug fixes, new features
3. **Documentation:** Improve examples, guides, tutorials
4. **Testing:** Help test on different codebases and languages

### Development Setup
```bash
git clone https://github.com/BeaconBay/ck
cd ck
cargo build --workspace
cargo test --workspace
./target/debug/ck --index test_files/
./target/debug/ck --sem "test query" test_files/
```

### CI Requirements
Before submitting a PR, ensure your code passes all CI checks:

```bash
# Format code (required)
cargo fmt --all

# Run clippy linter (required - must have no warnings)
cargo clippy --workspace --all-features --all-targets -- -D warnings

# Run tests (required)
cargo test --workspace

# Check minimum supported Rust version (MSRV)
cargo hack check --each-feature --locked --rust-version --workspace
```

The CI pipeline runs on Ubuntu, Windows, and macOS to ensure cross-platform compatibility.

## 🗺 Roadmap

### Current (v0.5+)
- ✅ MCP (Model Context Protocol) server for AI agent integration
- ✅ grep-compatible CLI with semantic search and file listing flags
- ✅ FastEmbed integration with BGE models and enhanced model selection
- ✅ File exclusion patterns and glob support
- ✅ Threshold filtering and relevance scoring with visual highlighting
- ✅ Tree-sitter parsing and intelligent chunking for 7+ languages
- ✅ Complete code section extraction (`--full-section`)
- ✅ Clean stdout/stderr separation for reliable scripting
- ✅ Incremental index updates with hash-based change detection
- ✅ Token-aware chunking with HuggingFace tokenizers
- ✅ Published to crates.io (`cargo install ck-search`)

### Next (v0.6+)
- 🚧 Configuration file support
- 🚧 Package manager distributions (brew, apt)
- 🚧 Enhanced MCP tools (file writing, refactoring assistance)
- 🚧 VS Code extension
- 🚧 JetBrains plugin
- 🚧 Additional language chunkers (Java, PHP, Swift)

## ❓ FAQ

**Q: How is this different from grep/ripgrep/silver-searcher?**
A: ck includes all the features of traditional search tools, but adds semantic understanding. Search for "error handling" and find relevant code even when those exact words aren't used.

**Q: Does it work offline?**
A: Yes, completely offline. The embedding model runs locally with no network calls.

**Q: How big are the indexes?**
A: Typically 1-3x the size of your source code. The `.ck/` directory can be safely deleted to reclaim space.

**Q: Is it fast enough for large codebases?**
A: Yes. The first semantic search builds the index automatically; after that only changed files are reprocessed, keeping searches sub-second even on large projects.

**Q: Can I use it in scripts/automation?**
A: Absolutely. The `--json` and `--jsonl` flags provide structured output perfect for automated processing and AI agent integration.

**Q: What about privacy/security?**
A: Everything runs locally. No code or queries are sent to external services. The embedding model is downloaded once and cached locally.

**Q: Where are the embedding models cached?**
A: Models are cached in platform-specific directories:
- Linux/macOS: `~/.cache/ck/models/`
- Windows: `%LOCALAPPDATA%\ck\cache\models\`
- Fallback: `.ck_models/models/` in current directory

## 📄 License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

## 🙏 Credits

Built with:
- [Rust](https://rust-lang.org) - Systems programming language
- [FastEmbed](https://github.com/Anush008/fastembed-rs) - Fast text embeddings
- [Tantivy](https://github.com/quickwit-oss/tantivy) - Full-text search engine
- [clap](https://github.com/clap-rs/clap) - Command line argument parsing

Inspired by the need for better code search tools in the age of AI-assisted development.

---

**Start finding code by what it does, not what it says.**

```bash
cargo install ck-search
ck --sem "the code you're looking for"
```