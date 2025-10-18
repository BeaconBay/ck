---
title: Changelog
description: Complete version history for ck with all notable changes, features, fixes, and technical improvements. Follows semantic versioning.
---

# Changelog

All notable changes to ck are documented here following [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- **VitePress documentation site**: Comprehensive documentation with improved navigation, search, and structure in `docs-site/` directory
- **Documentation features**: Guide pages, feature documentation, CLI reference, embedding model guide, architecture docs, and contributing guides
- **Local search**: Built-in search functionality in documentation site

### Technical
- **Self-contained docs**: All documentation tooling isolated in docs-site/ with independent build process using pnpm and VitePress
- **Node.js integration**: Documentation site uses Node.js 18+, pnpm 10+, and VitePress 1.6+ for modern documentation experience
- **GitHub integration**: Edit links and social links configured for easy contribution

## [0.5.3] - 2025-09-29

### Added
- **`.ckignore` file support**: Automatic creation of `.ckignore` file with sensible defaults for persistent exclusion patterns
- **Media file exclusions**: Images (png, jpg, gif, svg, etc.), videos (mp4, avi, mov, etc.), and audio files (mp3, wav, flac, etc.) excluded by default
- **Config file exclusions**: JSON and YAML files excluded from indexing by default to reduce noise in search results
- **`--no-ckignore` flag**: Option to bypass `.ckignore` patterns when needed
- **Persistent patterns**: Exclusion patterns persist across searches without needing command-line flags each time

### Fixed
- **Exclusion pattern persistence** ([#67](https://github.com/BeaconBay/ck/issues/67)): Patterns now persist in `.ckignore` instead of requiring `--exclude` flags on every search
- **Media file indexing** ([#66](https://github.com/BeaconBay/ck/issues/66)): Images, videos, and other binary files no longer indexed by default
- **Config file noise** ([#27](https://github.com/BeaconBay/ck/issues/27)): JSON/YAML config files excluded to focus search on actual code

### Technical
- **Additive pattern merging**: `.gitignore` + `.ckignore` + CLI + defaults all merge together (not mutually exclusive)
- **Auto-creation on first index**: `.ckignore` created automatically at repository root during first indexing
- **Glob pattern syntax**: Uses same pattern syntax as `.gitignore` for familiarity
- **Comprehensive test coverage**: 4 new tests covering creation, parsing, and exclusion logic

## [0.4.7] - 2025-09-19

### Added
- **Model switching command**: New `--switch-model` flag for seamless embedding model transitions with intelligent rebuild detection
- **Force rebuild option**: `--force` flag for explicit index rebuilding when switching models
- **Model resolution system**: Smart model management that respects existing index configurations and provides clear conflict guidance
- **Enhanced status display**: Index status now shows which embedding model and dimensions are in use
- **Search model validation**: Prevents mixing embedding models during search operations with actionable error messages

### Fixed
- **Windows atomic writes**: Fixed critical Windows compatibility issue where index files could become corrupted during writes
- **Embedding dimension mismatches**: Comprehensive validation preventing crashes from mixed embedding models with clear user guidance
- **Model consistency**: Enforced consistent embedding model usage across index lifecycle (build, search, update)
- **Clippy compliance**: Resolved all compiler warnings to meet strict CI requirements

### Technical
- **Atomic file operations**: Uses `tempfile::NamedTempFile` for cross-platform atomic writes with proper sync guarantees
- **Model registry integration**: Centralized model management with alias support and dimension tracking
- **Enhanced error messages**: User-friendly error messages with exact commands to resolve issues (e.g., "run `ck --clean .` then rebuild”)
- **Legacy code cleanup**: Removed 338 lines of unused ANN semantic search implementation
- **Interrupt handling**: Proper Ctrl+C handling during indexing with graceful cleanup

## [0.4.5] - 2025-09-13

### Added
- **Enhanced token-based chunking**: Implemented model-specific token-aware chunking using HuggingFace tokenizers for precise token counting instead of character estimation
- **Model-specific configurations**: Chunks now sized according to model capacity - 1024 tokens for large models (nomic/jina) vs 400 tokens for small models (bge-small)
- **Streamlined --inspect command**: Enhanced file inspection showing token counts per chunk, language detection, and clean visualization without visual noise
- **FastEmbed capacity utilization**: Configured FastEmbed to use full model capacity (8192 tokens for nomic/jina models vs previous 512 token truncation)
- **Indexing progress transparency**: Added model name and chunk configuration display during indexing operations

### Fixed
- **Token estimation accuracy**: Replaced rough character-based estimation with actual model tokenizers for precise chunking
- **Model capacity underutilization**: Fixed FastEmbed configuration to use full 8K context for large models instead of 512-token default
- **Clippy compliance**: Resolved all compiler warnings to meet CI/CD standards with `-D warnings` flag
- **Unused code cleanup**: Removed dead code and properly annotated intentional allowances for CI compliance

### Technical
- **HuggingFace tokenizer integration**: Added hf-hub and tokenizers dependencies for precise token counting
- **Model-aware chunking system**: `get_model_chunk_config()` function providing balanced precision vs context chunking strategy
- **Enhanced --inspect visualization**: Complete rewrite showing essential chunking information without progress bar clutter
- **Comprehensive quality checks**: All 88 tests passing with clippy compliance and code formatting standards

## [0.4.4] - 2025-09-13

### Fixed
- **`--add` command argument parsing**: Fixed issue where file paths were incorrectly parsed as pattern arguments, preventing single file additions to the index
- **Empty pattern behavior**: Empty regex patterns now match each line once (consistent with grep/ripgrep) instead of matching at every character position causing massive duplication

## [0.4.3] - 2025-09-13

### Added
- **Enhanced embedding models**: Added support for Nomic V1.5 (8192 tokens, 768 dimensions) and Jina Code (8192 tokens, code-specialized) models
- **Model selection**: New `--model` flag for choosing embedding model during indexing (`bge-small`, `nomic-v1.5`, `jina-code`)
- **Index-time model configuration**: Model selection is now properly configured at index creation time and stored in index manifest
- **Automatic model detection**: Search operations automatically use the model stored in the index manifest
- **Reranking support**: Added cross-encoder reranking with `--rerank` flag and `--rerank-model` option for improved search relevance
- **Striding for large chunks**: Implemented text striding with overlap for chunks exceeding model token limits
- **Token estimation**: Added token counting utilities to optimize chunk sizes for different models

### Fixed
- **Ctrl-C interrupt handling**: Fixed issue where indexing could not be properly cancelled - now uses `try_for_each` to stop all parallel workers immediately
- **Model compatibility checking**: Index operations now validate model compatibility and provide clear error messages for mismatches

### Technical
- **Model registry system**: New `ck-models` crate with centralized model configuration and limits
- **Index manifest enhancement**: Added `embedding_model` and `embedding_dimensions` fields to track model used for indexing
- **Backward compatibility**: Existing indexes without model metadata continue to work with default BGE model
- **Architecture fix**: Corrected design where model selection was incorrectly a search-time option instead of index-time configuration

### Documentation
- **README model guide**: Added comprehensive section explaining embedding model options and their trade-offs
- **CLI help improvements**: Enhanced help text with clear model selection examples and implications

## [0.4.2] - 2025-09-11

### Fixed
- **Hidden file indexing bug**: Fixed critical bug where hidden directories (especially `.git`) were being indexed despite exclusion patterns
- **Semantic search pollution**: Eliminated `.git` files appearing in semantic search results for unrelated queries
- **Index size reduction**: Significantly reduced index size by properly excluding hidden files and directories

### Technical
- **WalkBuilder configuration**: Changed `.hidden(false)` to `.hidden(true)` to respect hidden file conventions
- **Exclusion pattern enforcement**: Hidden file exclusion now takes precedence, preventing override patterns from being ignored
- **Performance improvement**: Reduced indexing time and storage by not processing `.git` and other hidden directories

## [0.4.1] - 2025-09-10

### Added
- **JSONL output format**: Stream-friendly `--jsonl` flag for AI agent workflows with structured output
- **No-snippet mode**: `--no-snippet` flag for metadata-only output to reduce bandwidth for agents
- **Agent documentation**: Comprehensive README section explaining JSONL benefits over traditional JSON
- **Agent examples**: Python code demonstrating stream processing patterns for AI workflows
- **UTF-8 warning suppression**: Eliminated noisy warnings for binary files in .git directories

### Technical
- **JsonlSearchResult struct**: New agent-friendly output format with conversion methods
- **Extended SearchResult**: Added chunk_hash and index_epoch fields for future agent features
- **Comprehensive test coverage**: 4 new integration tests validating JSONL functionality
- **Updated help text**: Dedicated JSONL section explaining streaming benefits for agents
- **Phase 1 PRD**: Complete specification for agent-ready code navigation features

::: tip Why JSONL for AI Agents?
- **Streaming friendly**: Process results as they arrive, no waiting for complete response
- **Memory efficient**: Parse one result at a time, not entire array into memory
- **Error resilient**: Malformed lines don’t break entire response
- **Standard format**: Used by OpenAI, Anthropic, and modern ML pipelines
:::

## [0.3.9] - 2025-09-10

### Added
- **Streaming producer-consumer indexing**: Implemented efficient streaming architecture for large-scale indexing operations
- **Memory-efficient processing**: Reduces memory footprint during indexing of large codebases
- **Performance optimization**: Better resource utilization through streaming data flow

### Technical
- **Producer-consumer pattern**: Separates file discovery from processing for better parallelization
- **Streaming integration**: Compatible with existing smart update and exclude pattern functionality

## [0.3.8] - 2025-09-09

### Added
- **Enhanced model caching documentation**: Updated README with comprehensive information about embedding model cache locations
- **Platform-specific cache paths**: Documented cache directories for Linux/macOS (`~/.cache/ck/models/`), Windows (`%LOCALAPPDATA%\ck\cache\models\`), and fallback locations
- **Model download transparency**: Clear documentation of where fastembed stores ONNX models when downloaded during indexing

### Fixed
- **Documentation accuracy**: Removed outdated `.fastembed_cache` references and provided correct cache path information
- **FAQ section**: Added frequently asked questions about embedding model storage and management

## [0.3.7] - 2025-09-08

### Improved
- **Smart binary detection**: Replaced restrictive extension-based file detection with ripgrep-style content analysis using NUL byte detection
- **Broader text file support**: Now automatically indexes log files (`.log`), config files (`.env`, `.conf`), and any other text format regardless of extension
- **Improved accuracy**: Files without extensions containing text content are now correctly detected and indexed
- **Binary file exclusion**: Files containing NUL bytes (executables, images, etc.) are correctly identified as binary and excluded from indexing
- **Performance**: Fast detection using only the first 8KB of file content, similar to ripgrep’s approach

### Technical
- **Content-based detection**: `is_text_file()` function now reads file content instead of checking against a hardcoded extension allowlist
- **Test coverage**: Added comprehensive tests for binary detection with various file types and edge cases

## [0.3.6] - 2025-09-08

### Fixed
- **Exclude patterns functionality**: Fixed critical bug where `--exclude` patterns were completely ignored during indexing operations
- **Directory exclusion**: `--exclude "node_modules"` and similar patterns now work correctly to exclude directories and files
- **Pattern matching**: Added support for gitignore-style glob patterns using ripgrep’s `OverrideBuilder` for consistent, performant exclusion
- **Multiple exclusions**: Fixed support for multiple `--exclude` flags (e.g., `--exclude "node_modules" --exclude "*.log"`)

### Technical
- **ripgrep alignment**: Leveraged the `ignore` crate’s `OverrideBuilder` for exclude pattern matching, aligning with ripgrep’s proven approach
- **Streaming integration**: Exclude patterns now work correctly with the new streaming indexing architecture
- **API consistency**: Updated all indexing functions (`index_directory`, `smart_update_index`, etc.) to support exclude patterns

## [0.3.5] - 2025-09-07

### Added
- **Git integration**: Added support for respecting `.gitignore` files during search and indexing operations
- **Ignore control flag**: Added `--no-ignore` flag to disable gitignore support when needed
- **Clean implementation**: Uses the `ignore` crate for proper gitignore parsing and directory traversal

### Fixed
- **UTF-8 boundary panic**: Fixed panic when truncating text containing emojis or multi-byte UTF-8 characters in preview display

## [0.3.1] - 2025-09-06

### Improved
- **Enhanced UX for semantic search**: Added intelligent defaults (topk=10, threshold=0.6) for semantic search to reduce cognitive load
- **Better CLI discoverability**: Added `--limit` as intuitive alias for `--topk` flag
- **Improved help documentation**: Clear signposting of relevant flags with aligned messaging across examples and descriptions
- **Informational output**: Semantic search now shows current parameters (e.g., "ℹ Semantic search: top 10 results, threshold ≥0.6")
- **Consistent flag documentation**: Help text now clearly shows defaults and relationships between flags

## [0.3.0] - 2025-09-06

### Fixed
- **Hybrid search indexing consistency**: Fixed hybrid search to use the same efficient v3 semantic indexing as semantic search mode, eliminating redundant index rebuilds and improving performance consistency
- **Directory validation**: Fixed issue where searching non-existent directories would silently fall back to parent directory indexes instead of showing clear error messages
- **Output stream separation**: All progress indicators and status messages now correctly output to stderr instead of stdout, ensuring clean output for piping and scripts
- **NaN sort handling**: Fixed edge cases with NaN values in similarity scoring that could cause inconsistent results

### Added
- **File listing flags**: Added grep-compatible `-l/--files-with-matches` and `-L/--files-without-match` flags for listing filenames only
- **Enhanced visual output**: Implemented sophisticated match highlighting with color-coded similarity heatmaps using RGB gradients
- **Better user experience**: Added "No matches found" message to stderr when no results are found, improving clarity for users
- **Improved error handling**: Enhanced directory traversal error handling and graceful degradation for individual file failures
- **Incremental indexing**: Smart hash-based index updates that only reprocess changed files, dramatically improving index update performance

### Improved
- **Indexing strategy optimization**: Smart embedding computation that only processes embeddings when needed for semantic/hybrid search, dramatically improving performance for regex-only workflows
- **Semantic search v3**: New implementation using pre-computed embeddings from sidecar files with span-based content extraction
- **Test infrastructure**: Enhanced integration tests with better binary path resolution and more resilient semantic search testing
- **Code quality**: Removed unused code, fixed compiler warnings, and improved error messaging throughout the codebase

## [0.2.0] - 2025-08-30

### Added
- Major improvements to CLI functionality
- Full-section feature implementation (`--full-section` flag)
- Comprehensive testing suite (40+ tests)
- Smart exclusion patterns for Python virtual environments and build artifacts
- Installation script with PATH setup (`install.sh`)

### Fixed
- CLI flag conflict: changed `-h` to `--no-filename` to avoid help conflict
- Proper handling of files with no filename
- File exclusion functionality during index creation
- Enhanced semantic search to return complete code sections

### Improved
- Updated documentation (README.md, PRD.txt) to reflect current implementation status
- Marked milestones M0-M5 as completed in project roadmap

## [0.1.0] - Initial Release

### Added
- Initial version of ck project with core functionality
- Drop-in grep compatibility with semantic search capabilities
- Basic regex, semantic, lexical, and hybrid search modes
- JSON output format for agent-friendly integration
- File indexing and sidecar management system

## Version Timeline

| Version | Focus Area | Release Date | Status |
|---------|-----------|--------------|--------|
| 0.1.0 | MVP, basic search | 2025-08-30 | ✅ Released |
| 0.2.0 | Tree-sitter, chunking | 2025-08-30 | ✅ Released |
| 0.3.x | Incremental indexing | 2025-09-06 | ✅ Released |
| 0.4.x | Multiple models | 2025-09-13 | ✅ Released |
| 0.5.x | MCP integration | 2025-09-29 | ✅ Released (current) |
| 0.6.0 | Config, distribution | TBD | 🚧 Planned |
| 0.7.0 | Editor integrations | TBD | 📋 Planned |
| 0.8.0 | Advanced features | TBD | 💭 Conceptual |
| 1.0.0 | Stability | TBD | 🎯 Goal |

## Breaking Changes Policy

ck follows [Semantic Versioning](https://semver.org/):

- **MAJOR** (1.0.0): Breaking changes to CLI or API
- **MINOR** (0.X.0): New features, backward compatible
- **PATCH** (0.0.X): Bug fixes, backward compatible

### Before v1.0

During the 0.x series, minor versions may introduce breaking changes, but we strive for backward compatibility where possible:

- **CLI compatibility**: We avoid changing existing flags and maintain grep-like behavior
- **Index format**: Indexes are regenerated automatically when format changes
- **Output format**: JSON/JSONL structure remains stable; new fields may be added

### v1.0 Stability Goals

Before reaching v1.0, ck will:
- ✅ Stabilize CLI interface
- ✅ Finalize MCP tool signatures
- ✅ Complete core feature set
- ✅ Achieve production maturity
- ✅ Document upgrade paths

## See Also

- [Roadmap](/guide/roadmap) — Planned features and timeline
- [GitHub Releases](https://github.com/BeaconBay/ck/releases) — Download releases
- [Contributing](/contributing/development) — Help build ck
