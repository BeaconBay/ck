# Changelog

All notable changes to this project will be documented in this file.

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

### Why JSONL for AI Agents?
- **Streaming friendly**: Process results as they arrive, no waiting for complete response
- **Memory efficient**: Parse one result at a time, not entire array into memory
- **Error resilient**: Malformed lines don't break entire response
- **Standard format**: Used by OpenAI, Anthropic, and modern ML pipelines

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
- **Performance**: Fast detection using only the first 8KB of file content, similar to ripgrep's approach

### Technical
- **Content-based detection**: `is_text_file()` function now reads file content instead of checking against a hardcoded extension allowlist
- **Test coverage**: Added comprehensive tests for binary detection with various file types and edge cases

## [0.3.6] - 2025-09-08

### Fixed
- **Exclude patterns functionality**: Fixed critical bug where `--exclude` patterns were completely ignored during indexing operations
- **Directory exclusion**: `--exclude "node_modules"` and similar patterns now work correctly to exclude directories and files
- **Pattern matching**: Added support for gitignore-style glob patterns using ripgrep's `OverrideBuilder` for consistent, performant exclusion
- **Multiple exclusions**: Fixed support for multiple `--exclude` flags (e.g., `--exclude "node_modules" --exclude "*.log"`)

### Technical
- **ripgrep alignment**: Leveraged the `ignore` crate's `OverrideBuilder` for exclude pattern matching, aligning with ripgrep's proven approach
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
- **File listing flags**: Added grep-compatible `-l/--files-with-matches` and `-L/--files-without-matches` flags for listing filenames only
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