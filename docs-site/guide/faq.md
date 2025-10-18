---
title: FAQ
description: Frequently asked questions about ck semantic code search. Covers indexing, models, performance, troubleshooting, and common usage questions.
---

# FAQ

Frequently asked questions about ck.

## General

### How is ck different from grep/ripgrep/silver-searcher?

ck includes all the features of traditional search tools, but adds semantic understanding. Search for “error handling” and find relevant code even when those exact words aren’t used. You can use ck as a drop-in replacement for grep while optionally leveraging semantic capabilities.

### Does it work offline?

Yes, completely offline. The embedding model runs locally with no network calls. All indexing, searching, and model inference happens on your machine.

### Is it fast enough for large codebases?

Yes. Performance benchmarks:
- **Indexing** – ~1M LOC in under 2 minutes
- **Search** – Sub-500ms queries on typical codebases
- **Delta updates** – Only changed files are re-indexed

The first semantic search builds the index automatically; after that only changed files are reprocessed, keeping searches sub-second even on large projects.

### Can I use it in scripts/automation?

Absolutely. ck provides structured output formats:
- `--json`: Single JSON array (good for small result sets)
- `--jsonl`: One JSON object per line (recommended for streaming, AI agents)
- `--no-snippet`: Metadata only (minimal bandwidth)

Perfect for CI/CD pipelines, git hooks, and automated processing.

## Installation & Setup

### Where are embedding models downloaded and cached?

Models are cached in platform-specific directories:
- **Linux/macOS**: `~/.cache/ck/models/`
- **Windows**: `%LOCALAPPDATA%\ck\cache\models\`
- **Fallback**: `.ck_models/models/` in current directory

The model is downloaded once (~80-500MB depending on model) and reused thereafter.

### Can I reuse my existing HuggingFace cache?

Yes! ck uses `hf-hub` under the hood, so you can set environment variables to reuse your existing cache:

```bash
export HF_HOME=~/.cache/huggingface
# or
export HF_HUB_CACHE=~/.cache/huggingface/hub
```

This avoids downloading models multiple times if you already have them cached.

### How do I update ck?

```bash
# Update from crates.io
cargo install ck-search --force

# Or from source
cd ck
git pull
cargo install --path ck-cli --force
```

## Indexing

### How big are the indexes?

Typically 1-3x the size of your source code. The `.ck/` directory can be safely deleted to reclaim space and will be rebuilt on the next semantic search.

Index size depends on:
- Embedding model dimensions (384 for bge-small, 768 for nomic/jina)
- Number of chunks (more small functions = more chunks)
- Number of indexed files

### Why does indexing take so long the first time?

First-time indexing includes:
1. **Model download** (~80-500MB, one-time)
2. **File discovery** (traversing directory tree)
3. **Chunking** (parsing files with tree-sitter)
4. **Embedding generation** (AI model inference for each chunk)

Subsequent indexes only process changed files, making them much faster.

### Large files (26K+ LOC) re-index slowly on changes. Can this be optimized?

::: warning Performance Impact on Large Files
Currently, ck re-indexes the entire file when it detects changes. This can be slow for files over 26K LOC.

**Why this happens**:
- Semantic chunking boundaries could change based on file content
- Size-based chunking could potentially use git diff, but semantic chunking cannot

**Workarounds**:
1. Exclude very large files from indexing (add to `.ckignore`)
2. Use size-based chunking for large data files
3. Split large files if possible

**Future**: Git diff-based incremental indexing is being explored (#69).
:::

### Can I index only a specific subdirectory?

Not currently. Indexing always works from the repository root.

**Example that doesn’t work as expected**:
```bash
ck --index ./docs  # Still indexes entire repo from root
```

**Workaround**: Use `.ckignore` to exclude everything except your target directory:
```txt
# Exclude everything
/*

# Include only docs
!/docs/
```

This is a known limitation (#50).

## Search

### Search results seem irrelevant. How can I improve them?

Try these approaches:

**1. Adjust threshold**:
```bash
# Higher threshold = more strict
ck --sem --threshold 0.7 "error handling" src/

# Lower threshold = more exploratory
ck --sem --threshold 0.3 "pattern" src/
```

**2. Try different models**:
```bash
# Code-specialized model
ck --switch-model jina-code .

# Better for large functions
ck --switch-model nomic-v1.5 .
```

**3. Use hybrid search**:
```bash
# Combines semantic + keyword matching
ck --hybrid "connection timeout" src/
```

**4. Show relevance scores**:
```bash
# See why results were matched
ck --sem --scores "auth" src/
```

**5. Refine query**:
- Be more specific: “JWT authentication” vs “auth”
- Use technical terms: “connection pool” vs “database stuff”
- Describe what it does: “retry with exponential backoff” vs “retry”

### Why doesn’t semantic search find my specific function name?

Semantic search finds code by *meaning*, not exact text. For specific identifiers, use regular keyword search:

```bash
# Find function by name (keyword)
ck "mySpecificFunction" src/

# Find similar functionality (semantic)
ck --sem "database connection pooling" src/

# Best of both (hybrid)
ck --hybrid "mySpecificFunction" src/
```

### How do I search for a specific file type?

Currently, use file globbing or path specifications:

```bash
# Glob patterns
ck "pattern" **/*.rs
ck "pattern" **/*.{js,ts}

# Specific directory
ck "pattern" src/components/

# With exclusions
ck --exclude "*.test.js" "pattern" src/
```

**Note**: A ripgrep-style `--type` flag is planned for future versions (#28).

## Index Management

### How do I check if my code is indexed?

```bash
# Check index status
ck --status .

# Shows:
# - Whether index exists
# - Number of files indexed
# - Embedding model used
# - Last update time
```

### When should I rebuild the index?

::: tip Rebuilding Is Rarely Needed
ck automatically detects and re-indexes changed files. You only need to rebuild when:
- Switching embedding models
- Index seems corrupted
- Major codebase restructuring

```bash
# Clean and rebuild
ck --clean .
ck --index .

# Or switch model (rebuilds automatically)
ck --switch-model nomic-v1.5 .
```
:::

### Can I have multiple indexes for different models?

Not simultaneously. Each repository has one index using one model. To try different models:

```bash
# Switch and compare
ck --switch-model jina-code .
ck --sem "pattern" src/

ck --switch-model nomic-v1.5 .
ck --sem "pattern" src/
```

Switching models triggers a rebuild.

## File Filtering

### What files are excluded by default?

ck automatically excludes:
- **Binary files** (detected via content, not extension)
- **Hidden directories** (`.git/`, `.cache/`, etc.)
- **.gitignore patterns** (respects repository exclusions)
- **.ckignore patterns** (semantic search specific exclusions)

Default `.ckignore` includes:
- Images (png, jpg, gif, svg, etc.)
- Videos (mp4, avi, mov, etc.)
- Audio files (mp3, wav, flac, etc.)
- Archives (zip, tar, gz, etc.)
- Config files (*.json, *.yaml)
- Build artifacts

### How do I customize file exclusions?

Edit `.ckignore` in your repository root:

```txt
# Add custom patterns
logs/
*.log
temp_*.txt
fixtures/

# Use ! to include despite parent exclusion
!important.log
```

Uses gitignore syntax. Changes take effect on next index operation.

### Can I search files that are gitignored?

Yes, with flags:

```bash
# Skip .gitignore rules
ck --no-ignore "pattern" .

# Skip .ckignore rules
ck --no-ckignore "pattern" .

# Skip both
ck --no-ignore --no-ckignore "pattern" .
```

## Models & Embedding

### Which embedding model should I use?

| Model | Best For | Trade-off |
|-------|----------|-----------|
| `bge-small` | General use, fast | Smaller chunks (400 tokens) |
| `nomic-v1.5` | Large functions, docs | Larger download (~500MB) |
| `jina-code` | Code-specialized | Larger download (~500MB) |

See [Embedding Models](/reference/models) for detailed comparison.

### Can I use custom embedding models?

Currently limited to models supported by fastembed-rs:
- `bge-small` (default)
- `nomic-v1.5`
- `jina-code`

**Future**: External embedding API support (OpenAI, HuggingFace, etc.) is being considered (#49).

### Do I need GPU for embeddings?

No, ck uses ONNX models via fastembed which run efficiently on CPU. GPU support is not required.

## Privacy & Security

### What about privacy/security?

- ✅ **100% offline**: No code or queries sent to external services
- ✅ **No telemetry**: No tracking or analytics
- ✅ **No API keys**: No external service dependencies
- ✅ **Local models**: Downloaded once, run locally forever

Your code never leaves your machine.

### Is ck safe to use in enterprise environments?

Yes. Since everything runs locally:
- No data exfiltration risk
- No dependency on external services
- No proprietary code exposure
- Works in air-gapped environments (after initial model download)

## Troubleshooting

### “Index not found” error

Run a semantic search to build the index automatically:

```bash
ck --sem "pattern" src/
```

Or explicitly create index:

```bash
ck --index .
```

### Model download fails

**Check**:
1. Internet connection
2. Disk space (~500MB for models)
3. Cache directory writable

**Retry**:
```bash
# Clear cache and retry
rm -rf ~/.cache/ck/models/
ck --index .
```

### Search hangs or is very slow

**Possible causes**:
- First-time index build in progress
- Very large codebase
- Low memory

**Solutions**:
```bash
# Check index status
ck --status .

# Limit results
ck --sem --topk 10 "pattern" src/

# Exclude large directories
echo "vendor/" >> .ckignore
echo "node_modules/" >> .ckignore
ck --clean .
ck --index .
```

### “Embedding dimension mismatch” error

Happens when mixing embedding models. Solution:

```bash
ck --clean .
ck --index --model bge-small .
```

## Still Have Questions?

- Check [Known Limitations](/guide/limitations)
- See [Advanced Configuration](/reference/advanced)
- Open an issue on [GitHub](https://github.com/BeaconBay/ck/issues)
- Review [existing issues](https://github.com/BeaconBay/ck/issues?q=is%3Aissue)
