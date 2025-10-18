---
title: Roadmap
description: Development roadmap for ck semantic code search. View completed features, in-progress work, and planned improvements for future releases.
---

# Roadmap

ck's development roadmap, completed features, and planned improvements.

## Current Version (v0.5.3)

### Completed Features ✅

#### Core Search
- **grep-compatible CLI** with semantic search capabilities
- **Semantic search** using local embedding models
- **Hybrid search** combining semantic + keyword with Reciprocal Rank Fusion
- **File listing flags** (-l, -L) for grep compatibility
- **Threshold filtering** for relevance control
- **Relevance scoring** with visual highlighting
- **Complete code section extraction** (--full-section flag)

#### AI Integration
- **MCP (Model Context Protocol) server** for AI agent integration
- **Built-in pagination** for large result sets
- **JSONL output format** for streaming workflows
- **Structured output** (JSON/JSONL) for automation

#### Indexing & Performance
- **Incremental index updates** with hash-based change detection
- **Automatic delta indexing** (only changed files)
- **Smart file filtering** with .gitignore and .ckignore support
- **Clean stdout/stderr separation** for reliable scripting
- **Interrupt handling** (safe Ctrl+C during indexing)

#### Language Support
- **Tree-sitter parsing** for 8 languages (Python, JS/TS, Rust, Go, Ruby, Haskell, C#, Zig)
- **Intelligent chunking** at function/class boundaries
- **Token-aware chunking** with HuggingFace tokenizers
- **Smart binary detection** (content-based, not extension-based)

#### Embedding Models
- **FastEmbed integration** with multiple models
- **BGE-Small** (default, fast, 400-token chunks)
- **Nomic V1.5** (8K context, 1024-token chunks)
- **Jina Code** (code-specialized, 8K context)
- **Model switching** with automatic rebuild detection

#### User Experience
- **File exclusion patterns** and glob support
- **.ckignore file** with automatic creation and sensible defaults
- **Published to crates.io** (`cargo install ck-search`)
- **Cross-platform support** (Linux, macOS, Windows)
- **Comprehensive documentation** with examples

## Next Release (v0.6)

### Planned Features 🚧

#### Configuration
- **Configuration file support** (.ck.toml or ck.config.json)
  — Default model selection
  — Default threshold and top-k values
  — Per-project preferences
  — Custom exclusion patterns

#### Distribution
- **Package manager distributions**
  — Homebrew formula for macOS
  — APT packages for Debian/Ubuntu
  — RPM packages for Fedora/RHEL
  — Chocolatey package for Windows
  — Pre-built binaries for all platforms

#### MCP Enhancements
- **Enhanced MCP tools**
  — File writing capabilities
  — Refactoring assistance
  — Code transformation tools
  — Bulk operations

#### Editor Integration
- **VS Code extension**
  — Inline semantic search
  — Code navigation
  — Quick actions
  — Status bar integration

- **JetBrains plugin**
  — IntelliJ IDEA support
  — PyCharm support
  — WebStorm support
  — Common features across IDEs

#### Language Support
- **Additional language chunkers**
  — Java (with tree-sitter-java)
  — PHP (with tree-sitter-php)
  — Swift (with tree-sitter-swift)
  — Kotlin (requested #21)
  — C/C++ (requested #21)

## Future Considerations

### Community Requests

Based on GitHub issues and discussions:

#### File Type Filtering (#28)
**Status**: Requested
**Description**: Add `--type` flag similar to ripgrep

```bash
# Proposed syntax
ck --type rust "pattern"
ck --type js,ts "pattern"
ck -T rust "pattern"  # Short form
```

**Use case**: Quickly filter searches to specific file types without glob patterns.

#### Git Diff-Based Re-indexing (#69)
**Status**: Under exploration
**Description**: Use git diff for incremental updates of large files

**Challenge**: Semantic chunking boundaries can change based on file content, making diff-based updates complex.

**Potential approach**:
- Implement for size-based chunking first
- Explore heuristics for semantic chunking
- May require trade-offs in accuracy vs speed

#### External Embedding API Support (#49)
**Status**: Under consideration
**Description**: Support external embedding services

**Requested APIs**:
- OpenAI embeddings
- HuggingFace Inference API
- Anthropic Claude embeddings
- Custom HTTP endpoints

**Trade-offs**:
- ✅ Access to latest/proprietary models
- ✅ No local model storage
- ❌ Requires internet connection
- ❌ Privacy/security considerations
- ❌ API costs

#### Bug Detection Category (#23)
**Status**: Research phase
**Description**: Specialized search for potential bugs

**Possible implementation**:
- Pre-trained model for code smell detection
- Pattern-based heuristics
- Integration with static analysis tools
- Custom queries for common bug patterns

## Long-Term Vision

### Performance
- GPU acceleration for embedding generation
- Distributed indexing for monorepos
- Caching strategies for frequently searched patterns
- Streaming search results

### Advanced Features
- Code similarity detection (find duplicate code)
- Dependency graph analysis
- Cross-repository search
- Code metrics and quality analysis
- Integration with code review tools

### AI Capabilities
- Context-aware code completion
- Automated documentation generation
- Code explanation and summarization
- Refactoring suggestions
- Test generation

### Enterprise Features
- Team collaboration features
- Shared index repositories
- Access control and permissions
- Audit logging
- On-premise deployment options

## Contributing to the Roadmap

### How to Influence Priority

1. **Vote on existing issues**: Use 👍 reactions on GitHub issues
2. **Open feature requests**: Describe your use case clearly
3. **Contribute code**: Submit PRs for features you need
4. **Sponsor development**: Support prioritization of specific features

### Feature Request Guidelines

When requesting features:
- ✅ Describe the problem, not just the solution
- ✅ Provide concrete use cases
- ✅ Consider implementation complexity
- ✅ Think about API design
- ✅ Check if workarounds exist

**Good example**:
> “As a developer working on microservices, I need to search across multiple repos simultaneously. Currently I have to cd into each repo and search separately, which is tedious for finding related implementations.”

**Less helpful**:
> “Add multi-repo search”

## Version Timeline

| Version | Focus | Status |
|---------|-------|--------|
| v0.1 | MVP, basic search | ✅ Released |
| v0.2 | Tree-sitter, chunking | ✅ Released |
| v0.3 | Incremental indexing | ✅ Released |
| v0.4 | Multiple models | ✅ Released |
| v0.5 | MCP integration | ✅ Released (current) |
| v0.6 | Config, distribution | 🚧 In planning |
| v0.7 | Editor integrations | 📋 Planned |
| v0.8 | Advanced features | 💭 Conceptual |

## Breaking Changes Policy

ck follows [Semantic Versioning](https://semver.org/):
- **MAJOR** (1.0.0): Breaking changes to CLI or API
- **MINOR** (0.X.0): New features, backward compatible
- **PATCH** (0.0.X): Bug fixes, backward compatible

### v1.0 Stability

Before reaching v1.0, ck will:
- Stabilize CLI interface
- Finalize MCP tool signatures
- Complete core feature set
- Achieve production maturity
- Document upgrade paths

## See Also

- [Known Limitations](/guide/limitations) — Current constraints
- [FAQ](/guide/faq) — Common questions
- [Contributing Guide](/contributing/development) — How to contribute
- [GitHub Issues](https://github.com/BeaconBay/ck/issues) — Feature requests and bugs
- [Changelog](https://github.com/BeaconBay/ck/blob/main/CHANGELOG.md) — Detailed release history
