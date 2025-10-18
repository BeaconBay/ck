---
title: Known Limitations
description: Current limitations and constraints of ck including large file handling, configuration options, and features not yet implemented. Workarounds provided.
---

# Known Limitations

Current limitations and constraints of `ck`. These are areas where `ck` has known constraints or where requested features haven't been implemented yet.

## Indexing Limitations

### Partial Directory Indexing

**Issue**: Cannot index only a specific subdirectory.

**Details**:
```bash
# This doesn't work as expected
ck --index ./docs  # Still indexes from repository root
```

Indexing always starts from the repository root, even when specifying a subdirectory path.

**Workaround**:
Use `.ckignore` to exclude everything except your target directory:
```txt
# Exclude everything
/*

# Include only docs
!/docs/
```

**Tracking**: GitHub issue [#50](https://github.com/BeaconBay/ck/issues/50)

### Full File Re-indexing for Large Files

**Issue**: Large files (26K+ LOC) are fully re-indexed on any change.

**Details**:
When a file changes, `ck` re-indexes the entire file rather than using git diff to index only changed sections. This is particularly noticeable with large files like database schema dumps.

**Why this happens**:
- Semantic chunking boundaries can change non-trivially based on file content
- A diff might split one semantic chunk into two
- Size-based chunking could theoretically use git diff, but semantic chunking cannot

**Workaround**:
1. Exclude very large files from indexing:
   ```txt
   # .ckignore
   schema.sql
   *.generated.sql
   ```

2. Use size-based chunking for large data files (semantic chunking not needed)

3. Split large files if possible

**Future consideration**: Git diff-based incremental indexing is being explored for size-based chunking.

**Tracking**: GitHub issue [#69](https://github.com/BeaconBay/ck/issues/69)

### Repository Root Requirement

**Issue**: Index operations require running from repository root.

**Details**:
The `.ck/` index directory is always created at the repository root, and all index operations assume this location.

**Impact**:
- Cannot have multiple independent indexes in subdirectories
- Must run `ck` commands from repository root for semantic search

**Workaround**:
Use `cd` to navigate to repository root before running semantic searches, or use git to determine root:
```bash
cd $(git rev-parse --show-toplevel)
ck --sem "pattern" .
```

## Search Limitations

### No File Type Filtering

**Issue**: No built-in `--type` flag like ripgrep for filtering by file type.

**Details**:
You cannot currently do:
```bash
ck --type rust "pattern"  # Not implemented
```

**Workaround**:
Use glob patterns or path specifications:
```bash
# Glob patterns
ck "pattern" **/*.rs
ck "pattern" **/*.{js,ts,jsx,tsx}

# Directory filtering
ck "pattern" src/

# Exclusions
ck --exclude "*.test.js" "pattern" src/
```

**Tracking**: GitHub issue [#28](https://github.com/BeaconBay/ck/issues/28)

### Single Embedding Model Per Index

**Issue**: Can only use one embedding model at a time for a repository.

**Details**:
Each index is tied to a specific embedding model. You cannot simultaneously search with multiple models.

**Impact**:
- Cannot compare models side-by-side without rebuilding
- Switching models requires full index rebuild

**Workaround**:
```bash
# Test model A
ck --switch-model bge-small .
ck --sem "pattern" src/

# Test model B (triggers rebuild)
ck --switch-model jina-code .
ck --sem "pattern" src/
```

Rebuilding takes time but is necessary when changing models.

## Model & Embedding Limitations

### Limited Embedding Model Options

**Issue**: Only models supported by fastembed-rs are available.

**Current models**:
- `bge-small` (default)
- `nomic-v1.5`
- `jina-code`

**Not supported**:
- Custom ONNX models
- External API-based models (OpenAI, Anthropic, HuggingFace Inference API)
- Proprietary embedding services

**Why**: `ck` uses fastembed-rs for fast local inference, which limits options to its supported models.

**Future consideration**: External embedding API support is being considered for users who want to use specific models.

**Tracking**: GitHub issue [#49](https://github.com/BeaconBay/ck/issues/49)

### No Custom Model Fine-tuning

**Issue**: Cannot fine-tune embedding models on your specific codebase.

**Details**:
Models are pre-trained and used as-is. Domain-specific fine-tuning is not supported.

**Impact**:
- May not perform optimally on domain-specific code (medical, financial, etc.)
- Cannot adapt to company-specific terminology
- No way to improve performance on specific codebases

**Workaround**:
- Try different available models to find best fit
- Use hybrid search to combine semantic with keyword matching
- Adjust threshold to filter irrelevant results

### HuggingFace Cache Location

**Issue**: Model cache location is determined by fastembed/hf-hub defaults.

**Details**:
While environment variables like `$HF_HOME` work, there’s no `ck`-specific configuration for cache location.

**Workaround**:
Use HuggingFace environment variables:
```bash
export HF_HOME=~/.cache/huggingface
export HF_HUB_CACHE=~/.cache/huggingface/hub
```

## Language Support Limitations

### Limited Language Parsers

**Current support**: 8 languages with tree-sitter parsing
- Python, JavaScript/TypeScript, Rust, Go, Ruby, Haskell, C#, Zig

**Not yet supported**:
- Java (#21)
- C/C++ (#21)
- Swift (#21)
- Kotlin (#21)
- PHP (roadmap)
- Scala
- Objective-C

**Impact**:
Unsupported languages fall back to simple text chunking without semantic boundaries.

**Workaround**:
Unsupported languages still work but with less intelligent chunking:
- Chunks based on token count rather than function boundaries
- May split functions/classes awkwardly
- Still provides semantic search, just with less precise boundaries

**Tracking**: GitHub issue [#21](https://github.com/BeaconBay/ck/issues/21)

### PDF Support (Experimental)

**Status**: Basic PDF text extraction is supported but has limitations.

**Details**:
ck can index and search PDF files by extracting text content, but uses a different code path than standard source code indexing.

**Limitations**:
- Text extraction only (no semantic PDF parsing)
- Doesn't always work perfectly with all PDF formats
- Not as sophisticated as cloud-based parsers like llamaparse
- May produce lower-quality chunks compared to source code

**Trade-off**:
This is a local/privacy-first approach vs cloud parsing quality. Your PDFs never leave your machine, but parsing quality is lower than cloud services like semtools.

**Workaround**:
```txt
# .ckignore
# Exclude problematic PDFs
docs/large-spec.pdf
*.scanned.pdf
```

For mission-critical PDF search, consider cloud-based tools like semtools.

## Feature Limitations

### No Configuration File

**Issue**: All configuration must be done via CLI flags or `.ckignore`.

**Missing**:
- No `.ck.toml` or `ck.config.json`
- Cannot set default model, threshold, topk
- Cannot configure per-project preferences

**Workaround**:
Use shell aliases for common patterns:
```bash
# .bashrc or .zshrc
alias cks='ck --sem --threshold 0.7 --topk 10'
alias ckh='ck --hybrid --scores'
```

**Future**: Configuration file support is on the roadmap (v0.6+).

### No Bug Detection Category

**Issue**: No specialized “bug” search category or pattern.

**Details**:
Cannot specifically search for potential bugs using semantic patterns.

**Example not supported**:
```bash
ck --bugs src/  # Not implemented
```

**Workaround**:
Use semantic queries describing bug patterns:
```bash
ck --sem "null pointer dereference" src/
ck --sem "unchecked array access" src/
ck --sem "resource leak" src/
ck --hybrid "TODO|FIXME|XXX" src/
```

**Tracking**: GitHub issue [#23](https://github.com/BeaconBay/ck/issues/23)

### No Refactoring Assistance

**Issue**: MCP server is read-only; cannot modify code.

**Details**:
Current MCP tools only search and read code. No tools for:
- Writing code
- Refactoring assistance
- Automated fixes
- Code generation

**Future**: Enhanced MCP tools with file writing capabilities are planned (v0.6+).

## Performance Limitations

### CPU-Only Inference

**Issue**: No GPU acceleration for embedding generation.

**Details**:
Embedding models run on CPU via ONNX. This is generally fast enough but could be faster with GPU support.

**Impact**:
- First-time indexing of very large codebases (>1M LOC) can take 5-10 minutes
- Delta updates are still fast (only changed files)

**Workaround**:
- Use smaller/faster model (bge-small) for large codebases
- Exclude unnecessary files with `.ckignore`
- Index overnight for initial build

### Memory Usage for Large Codebases

**Issue**: Memory usage scales with codebase size during indexing.

**Impact**:
Very large codebases (10M+ LOC) may require significant memory during initial indexing.

**Workaround**:
- Index in smaller batches by temporarily excluding directories
- Close other applications during indexing
- Upgrade system memory if regularly working with massive codebases

## Platform Limitations

### Windows-Specific Considerations

**Issue**: Some features have Windows-specific behavior differences.

**Details**:
- Path handling differences (backslashes vs forward slashes)
- File permissions handled differently
- Case sensitivity differences

**Impact**:
Generally minimal, but edge cases may behave differently on Windows vs Unix.

### No Pre-built Binaries (Yet)

**Issue**: Must install via cargo or build from source.

**Missing**:
- No `.deb` packages for apt
- No `.rpm` packages for yum/dnf
- No homebrew formula for macOS (yet)
- No chocolatey package for Windows

**Workaround**:
Install via cargo (available on all platforms):
```bash
cargo install ck-search
```

**Future**: Package manager distributions planned for v0.6+.

## MCP Server Limitations

### No File Writing

**Issue**: MCP server can only read/search code, not modify it.

**Current capabilities**:
- Search (semantic, regex, hybrid)
- Index management
- Status checking

**Not available**:
- Writing files
- Refactoring code
- Applying fixes
- Code generation

**Future**: Enhanced MCP tools planned for v0.6+.

### Pagination Required for Large Results

**Issue**: Large result sets must be paginated in MCP server.

**Details**:
Default page size is 25 results. Getting more requires pagination cursors.

**Impact**:
AI agents must implement pagination logic for comprehensive results.

**Mitigation**:
- Adjust `page_size` parameter (max recommended: 100)
- Use `top_k` to limit total results
- Use threshold filtering to reduce result count

## Workarounds & Alternatives

For most limitations, see the [FAQ](/guide/faq) for workarounds and alternative approaches.

## Reporting Limitations

Found a limitation not listed here?
- Check [existing issues](https://github.com/BeaconBay/ck/issues)
- Open a [new issue](https://github.com/BeaconBay/ck/issues/new)
- Discuss in [GitHub Discussions](https://github.com/BeaconBay/ck/discussions)

## See Also

- [Roadmap](/guide/roadmap) — Planned features and improvements
- [FAQ](/guide/faq) — Common questions and workarounds
- [Advanced Configuration](/reference/advanced) — Advanced usage patterns
