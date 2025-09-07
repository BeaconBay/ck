# ck - Semantic Grep by Embedding

**ck (seek)** finds code by meaning, not just keywords. It's a drop-in replacement for `grep` that understands what you're looking for — search for "error handling" and find try/catch blocks, error returns, and exception handling code even when those exact words aren't present.

## Quick start
```bash
cargo install ck-search
```


```bash
# Find error handling patterns (finds try/catch, Result types, etc.)
ck --sem "error handling" src/

# Traditional grep-compatible search still works  
ck -n "TODO" *.rs

# Combine both: semantic relevance + keyword filtering
ck --hybrid "connection timeout" src/
```

## Why ck?

**For Developers:** Stop hunting through thousands of regex false positives. Find the code you actually need by describing what it does.

**For AI Agents:** Get structured, semantic search results in JSON format. Perfect for code analysis, documentation generation, and automated refactoring.

**For Teams:** Works exactly like `grep` with the same flags and behavior, but adds semantic intelligence when you need it.

## Quick Start

```bash
# Build from source
cargo build --release

# Index your project for semantic search
./target/debug/ck index src/

# Search by meaning
./target/debug/ck --sem "authentication logic" src/
./target/debug/ck --sem "database connection pooling" src/
./target/debug/ck --sem "retry mechanisms" src/

# Use all the grep features you know
./target/debug/ck -n -C 3 "error" src/
./target/debug/ck -r "TODO|FIXME" .
```

## Core Features

### 🔍 **Semantic Search**
Find code by concept, not keywords. Searches understand synonyms, related terms, and conceptual similarity.

```bash
# These find related code even without exact keywords:
ck --sem "retry logic"           # finds backoff, circuit breakers
ck --sem "user authentication"   # finds login, auth, credentials  
ck --sem "data validation"       # finds sanitization, type checking

# Get complete functions/classes containing matches (NEW!)
ck --sem --full-section "error handling"  # returns entire functions
ck --full-section "async def" src/        # works with regex too
```

### ⚡ **Drop-in grep Compatibility**
All your muscle memory works. Same flags, same behavior, same output format.

```bash
ck -i "warning" *.log              # Case-insensitive  
ck -n -A 3 -B 1 "error" src/       # Line numbers + context
ck --no-filename "TODO" src/        # Suppress filenames (grep -h equivalent)
ck -l "error" src/                  # List files with matches only (NEW!)
ck -L "TODO" src/                   # List files without matches (NEW!)
ck -r --exclude "*.test.js" "bug"  # Recursive with exclusions
ck "pattern" file1.txt file2.txt   # Multiple files
```

### 🎯 **Hybrid Search**  
Combine keyword precision with semantic understanding using Reciprocal Rank Fusion.

```bash
ck --hybrid "async timeout" src/    # Best of both worlds
ck --hybrid --scores "cache" src/   # Show relevance scores with color highlighting
ck --hybrid --threshold 0.02 query  # Filter by minimum relevance
ck -l --hybrid "database" src/      # List files using hybrid search
```

### 🤖 **Agent-Friendly Output**
Perfect JSON output for LLMs, scripts, and automation.

```bash
ck --json --sem "error handling" src/ | jq '.file'
ck --json --topk 5 "TODO" . | jq -r '.preview'
ck --json --full-section --sem "database" . | jq -r '.preview'  # Complete functions
```

### 📁 **Smart File Filtering**
Automatically excludes cache directories, build artifacts, and system files.

```bash
# These are excluded by default:
# .git, node_modules, target/, .fastembed_cache, __pycache__

# Override defaults:
ck --no-default-excludes "pattern" .     # Search everything
ck --exclude "dist" --exclude "logs" .   # Add custom exclusions
```

## How It Works

### 1. **Index Once, Search Many**
```bash
# Create semantic index (one-time setup)
ck index /path/to/project

# Now search instantly by meaning
ck --sem "database queries" .
ck --sem "error handling" .
ck --sem "authentication" .
```

### 2. **Three Search Modes**
- **`--regex`** (default): Classic grep behavior, no indexing required
- **`--sem`**: Pure semantic search using embeddings (requires index)
- **`--hybrid`**: Combines regex + semantic with intelligent ranking

### 3. **Relevance Scoring**
```bash
ck --sem --scores "machine learning" docs/
# [0.847] ./ai_guide.txt: Machine learning introduction...
# [0.732] ./statistics.txt: Statistical learning methods...
# [0.681] ./algorithms.txt: Classification algorithms...
```

## Advanced Usage

### Search Specific Files
```bash
# Glob patterns work
ck --sem "authentication" *.py *.js *.rs

# Multiple files
ck --sem "error handling" src/auth.rs src/db.rs

# Quoted patterns prevent shell expansion  
ck --sem "auth" "src/**/*.ts"
```

### Threshold Filtering
```bash
# Only high-confidence semantic matches
ck --sem --threshold 0.7 "query"

# Low-confidence hybrid matches (good for exploration)
ck --hybrid --threshold 0.01 "concept"

# Get complete code sections instead of snippets (NEW!)
ck --sem --full-section "database queries"
ck --full-section "class.*Error" src/     # Complete classes
```

### Top-K Results
```bash
# Limit results for focused analysis
ck --sem --topk 5 "authentication patterns"

# Great for AI agent consumption
ck --json --topk 10 "error handling" | process_results.py
```

### Directory Management
```bash
# Check index status
ck status .

# Clean up and rebuild
ck clean .
ck index .

# Add single file to index
ck add new_file.rs
```

## File Support

| Language | Indexing | Tree-sitter Parsing | Semantic Chunking |
|----------|----------|-------------------|------------------|
| Python | ✅ | ✅ | ✅ Functions, classes |
| JavaScript | ✅ | ✅ | ✅ Functions, classes, methods |
| TypeScript | ✅ | ✅ | ✅ Functions, classes, methods |
| Haskell | ✅ | ✅ | ✅ Functions, types, instances |

**Text Formats:** Markdown, JSON, YAML, TOML, XML, HTML, CSS, shell scripts, SQL, and plain text.

**Smart Exclusions:** Automatically skips `.git`, `node_modules`, `target/`, `build/`, `dist/`, `__pycache__/`, `.fastembed_cache`, `.venv`, `venv`, and other common build/cache/virtual environment directories.

## Installation

### From Source
```bash
git clone https://github.com/BeaconBay/ck
cd ck
cargo install --path ck-cli
```

### Package Managers (Planned)
```bash
# Coming soon:
brew install ck-search
apt install ck-search
```

## Architecture

ck uses a modular Rust workspace:

- **`ck-cli`** - Command-line interface and argument parsing
- **`ck-core`** - Shared types, configuration, and utilities  
- **`ck-search`** - Search engine implementations (regex, BM25, semantic)
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

## Examples

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
ck -l --hybrid --scores "security" src/  # Files with security-related code
```

## Configuration

### Default Exclusions
```bash
# View current exclusion patterns
ck --help | grep -A 20 exclude

# These directories are excluded by default:
# .git, .svn, .hg                    # Version control
# node_modules, target, build        # Build artifacts  
# .cache, __pycache__, .fastembed_cache  # Caches
# .vscode, .idea                     # IDE files
```

### Custom Configuration (Planned)
```toml
# .ck/config.toml
[search]
default_mode = "hybrid"
default_threshold = 0.05

[indexing]  
exclude_patterns = ["*.log", "temp/"]
chunk_size = 512
overlap = 64

[models]
embedding_model = "BAAI/bge-small-en-v1.5"
```

## Performance

- **Indexing:** ~1M LOC in under 2 minutes (with smart exclusions and optimized embedding computation)
- **Search:** Sub-500ms queries on typical codebases  
- **Index size:** ~2x source code size with compression
- **Memory:** Efficient streaming for large repositories with span-based content extraction
- **File filtering:** Automatic exclusion of virtual environments and build artifacts
- **Output:** Clean stdout/stderr separation for reliable piping and scripting

## Testing

Run the comprehensive test suite:
```bash
# Full test suite (40+ tests)
./test_ck.sh

# Quick smoke test (14 core tests)
./test_ck_simple.sh
```

Tests cover grep compatibility, semantic search, index management, file filtering, and more.

## Contributing

ck is actively developed and welcomes contributions:

1. **Issues:** Report bugs, request features
2. **Code:** Submit PRs for bug fixes, new features  
3. **Documentation:** Improve examples, guides, tutorials
4. **Testing:** Help test on different codebases and languages

### Development Setup
```bash
git clone https://github.com/your-org/ck
cd ck
cargo build
cargo test
./target/debug/ck index test_files/
./target/debug/ck --sem "test query" test_files/
```

## Roadmap

### Current (v0.3+)
- ✅ grep-compatible CLI with semantic search and file listing flags (`-l`, `-L`)
- ✅ FastEmbed integration with BGE models
- ✅ File exclusion patterns and glob support
- ✅ Threshold filtering and relevance scoring with visual highlighting
- ✅ Tree-sitter parsing and intelligent chunking (Python, TypeScript, JavaScript, Haskell)
- ✅ Complete code section extraction (`--full-section`)
- ✅ Enhanced indexing strategy with v3 semantic search optimization
- ✅ Clean stdout/stderr separation for reliable scripting
- ✅ Incremental index updates with hash-based change detection

### Near-term (v0.4-0.5)  
- 🚧 Configuration file support
- 🚧 Package manager distributions

### Medium-term (v0.4-0.6)
- 🔮 Multiple embedding model support
- 🔮 Advanced ranking algorithms
- 🔮 Plugin architecture for custom chunkers
- 🔮 Distributed/remote index support

### Long-term (v1.0+)
- 🔮 IDE integrations (VS Code, IntelliJ, etc.)
- 🔮 Git integration (semantic diffs, blame)
- 🔮 Web interface for team usage
- 🔮 Multi-language semantic understanding

## FAQ

**Q: How is this different from grep/ripgrep/silver-searcher?**  
A: ck includes all the features of traditional search tools, but adds semantic understanding. Search for "error handling" and find relevant code even when those exact words aren't used.

**Q: Does it work offline?**  
A: Yes, completely offline. The embedding model runs locally with no network calls.

**Q: How big are the indexes?**  
A: Typically 1-3x the size of your source code, depending on content. The `.ck/` directory can be safely deleted to reclaim space.

**Q: Is it fast enough for large codebases?**  
A: Yes. Indexing is a one-time cost, and searches are sub-second even on large projects. Regex searches require no indexing and are as fast as grep.

**Q: Can I use it in scripts/automation?**  
A: Absolutely. The `--json` flag provides structured output perfect for automated processing. Use `--full-section` to get complete functions for AI analysis.

**Q: What about privacy/security?**  
A: Everything runs locally. No code or queries are sent to external services. The embedding model is downloaded once and cached locally.

## License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Credits

Built with:
- [Rust](https://rust-lang.org) - Systems programming language
- [FastEmbed](https://github.com/Anush008/fastembed-rs) - Fast text embeddings
- [Tantivy](https://github.com/quickwit-oss/tantivy) - Full-text search engine
- [clap](https://github.com/clap-rs/clap) - Command line argument parsing

Inspired by the need for better code search tools in the age of AI-assisted development.

---

**Start finding code by what it does, not what it says.**

```bash
cargo build --release
./target/release/ck index .
./target/release/ck --sem "the code you're looking for"
```
