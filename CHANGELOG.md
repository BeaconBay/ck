# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added
- **C language support**: Full tree-sitter based semantic chunking for C files (.c, .h) - functions, structs, enums, unions, typedefs, macros
- **C++ language support**: Full tree-sitter based semantic chunking for C++ files (.cpp, .cc, .cxx, .hpp, .h) - classes, structs, namespaces, templates, enums, unions
## [0.7.6] - 2026-05-23

### Fixed
- **Scoped semantic search returned zero results** (#111). `semantic_search_v3` was computing the top-k from all chunks in the index *before* applying the requested path filter. With a whole-codebase index plus a narrow `path=` query, the global top-k could be entirely consumed by chunks outside the requested scope, leaving the path filter with nothing to keep. New `PathScope` enum filters at sidecar-collection time (also faster — no embedding loaded for out-of-scope chunks). Hoists the `canonicalize()` of the target path out of the per-result loop where it was running N times.
- **`oneshot` use-after-free race in `Receiver` drop** (#100, dependabot bump 0.1.11 → 0.1.13). Affects the in-process control-flow channels used internally.

### Security
- **MCP tool handlers escaped their working directory** (#112). Every MCP tool handler (semantic, lexical, regex, hybrid, index_status, reindex) accepted `request.path` verbatim with only an existence check. An agent connected over MCP could ask ck to index or semantically search any path readable on the host — `/etc`, another user's home, anywhere. `McpContext` stored a `cwd` but nothing enforced it. New `allowed_roots: Vec<PathBuf>` (canonical), defaults to `[canonical(cwd)]`, extensible via `CK_MCP_ALLOWED_ROOTS` env var (colon-separated). All 6 handlers now route `request.path` through `McpContext::resolve_request_path`, which canonicalizes and rejects paths outside the sandbox. Symlink-based escape is rejected post-canonicalization.

### Changed
- **MCP tool schemas compatible with Google Gemini API** (#106 by @gabriel-ecegi). Gemini's strict JSON Schema validator rejects union types like `{"type":["array","null"]}` in tool function declarations. The four request structs' `Option<Vec<String>>` fields (`include_patterns`, `exclude_patterns`) now use `#[schemars(with = "Vec<String>")]` so the published schema emits `{"type":"array"}`. Runtime behavior unchanged — serde still accepts missing/null.

### Technical
- **Release pipeline made fail-loud** (in 0.7.5's tail commits but worth repeating). `publish_crate`'s `cargo publish ... | tee` previously masked cargo's exit code via the pipeline; auth failures printed "Successfully published" and burned 10 min on a verify loop for a crate that never uploaded. Now uses `set -o pipefail` + upfront `CARGO_REGISTRY_TOKEN` presence check. `verify_crate` sends a `User-Agent` header (crates.io requires one; absence returned 403 and silently broke every prior release).
- **Dev-dep bumps** (#99): jws 3.2.2 → 3.2.3, qs 6.14.0 → 6.14.1, mdast-util-to-hast 13.2.0 → 13.2.1, preact (patch). Docs site + VSCode extension only.

## [0.7.5] - 2026-05-23

### Fixed
- **CI Test job (broken since 0.7.3)**: `cargo hack test --each-feature --workspace` now passes. Two `ck-engine` semantic `.ckignore` tests (`test_subdirectory_search_uses_parent_ckignore`, `test_multiple_ckignore_files_merge_correctly`) assumed the `fastembed` embedder was compiled in. Under permutations that disabled `fastembed`, `ck-embed::create_embedder_for_config` silently returned `DummyEmbedder` (zero vectors), every cosine similarity fell below the 0.1 threshold, and the tests panicked. Tests are now `#[cfg(feature = "fastembed")]`-gated.

### Technical
- **Consolidated release plumbing**: removed the duplicated `publish-crates` job from `ci.yaml` so `release.yml` is the sole publisher on tag push (no more race between two workflows competing for `cargo publish`).
- **Simplified version bumps**: intra-workspace dependencies (`ck-core`, `ck-models`, `ck-embed`, `ck-chunk`, `ck-ann`, `ck-index`, `ck-engine`, `ck-tui`) moved to `[workspace.dependencies]`. `ck-cli` now uses `version.workspace = true` like its peers. A version bump now touches one file (`Cargo.toml`) instead of nine.
- **Restored full crate publish set**: `ck-tui` was missing from `release.yml`'s `CRATES` list and was never published as part of a tagged release. Added.
- **CLAUDE.md**: updated release process docs to reflect the simpler bump workflow.

### Notes
- 0.7.3 and 0.7.4 are skipped on crates.io. 0.7.4 was partially published (only `ck-core` landed) due to the issues fixed here. 0.7.5 is the first clean release on top of the consolidated pipeline.

## [0.7.2] - 2026-01-24

### Added
- **Mixedbread model support**: First-class support for Mixedbread embedding and reranking models (PR #89 by @regenrek)
  - Embedding model: `mxbai-xsmall` (`mixedbread-ai/mxbai-embed-xsmall-v1`) - 384 dimensions, 4K context window
  - Reranker: `mxbai` (`mixedbread-ai/mxbai-rerank-xsmall-v1`) - Neural cross-encoder reranker
  - Fully local inference using ONNX Runtime with quantized models
  - Provider abstraction for clean model selection and routing
  - CLI support: `--model mxbai-xsmall` and `--rerank-model mxbai`
  - MCP server support for Mixedbread models in semantic/hybrid search tools
- **Dart language support**: Full tree-sitter based parsing for Dart files (PR #96 by @aboo)
  - Classes, methods, functions, getters/setters, constructors
  - Extension methods and mixins
- **Elixir language support**: Comprehensive Elixir/OTP parsing (PR #91 by @CamonZ)
  - Modules (`defmodule`), functions (`def`, `defp`), macros (`defmacro`, `defmacrop`)
  - Protocols, implementations, structs, exceptions
  - Module attributes (`@moduledoc`, `@doc`, `@spec`, `@type`, `@behaviour`)
  - Guards (`defguard`, `defguardp`)
- **VSCode extension improvements**: Better robustness and error handling (PR #74 by @runonthespot)
  - `--status-json` flag for structured CLI status output
  - `--` separator to prevent dash-prefixed queries from being parsed as flags
  - Dynamic .ckignore template fetched from backend
- **VitePress documentation site**: Comprehensive docs with navigation, search, and structure

### Fixed
- **Rust doc comments**: Doc comments (`///` and `/** */`) now correctly attach to their associated items instead of being separate chunks (PR #92 by @EduBalbino)
- **--no-ignore flag**: Now properly disables `.git/info/exclude` and global gitignore in addition to `.gitignore` (PR #93 by @lukeod)
- **regex_search ckignore**: `regex_search` now respects the `use_ckignore` option instead of always using .ckignore (PR #86 by @RX14)
- **Documentation link**: Fixed broken link to query-based chunking documentation (PR #78 by @transitive-bullshit)
- **Windows checksum format**: Fixed checksum file format for package manager compatibility on Windows (PR #98 by @fdr)

### Security
- **js-yaml update**: Bumped js-yaml from 4.1.0 to 4.1.1 to fix prototype pollution vulnerability (PR #88)

### Technical
- **ort 2.0.0-rc.11 compatibility**: Updated Mixedbread implementation for latest ONNX Runtime API
- **ndarray 0.17**: Upgraded ndarray for ort compatibility
- **SWE-bench framework**: Added benchmarks directory with SWE-bench evaluation framework

### Contributors
Thanks to the following contributors for this release:
- @regenrek (Kevin Kern) - Mixedbread model support
- @CamonZ (Rafael Simon Garcia) - Elixir language support
- @aboo - Dart language support
- @runonthespot - VSCode extension improvements
- @EduBalbino - Rust doc comments fix
- @lukeod - --no-ignore fix
- @RX14 (Stephanie Wilde-Hobbs) - regex_search ckignore fix
- @transitive-bullshit - Documentation link fix
- @fdr - Windows checksum fix

## [0.7.1] - 2025-11-05

### Fixed
- **Hierarchical .ckignore support**: Fixed MCP daemon subdirectory search failures and indexing performance issues by implementing proper hierarchical .ckignore file loading using WalkBuilder's custom ignore filename support (PR #84)
- **Subdirectory search**: MCP searches in subdirectories now correctly respect parent directories' .ckignore files, matching .gitignore behavior
- **Indexing performance**: Eliminated repeated indexing of ignored files, significantly improving indexing speed in large repositories

### Technical
- **FileCollectionOptions refactor**: Replaced parameter threading anti-pattern with unified config struct for cleaner architecture
- **WalkBuilder integration**: Configured with `.add_custom_ignore_filename(".ckignore")` for hierarchical ignore file support
- **CLI flag addition**: Added `--no-ckignore` flag to disable .ckignore support when needed
- **Test coverage**: Added regression test `test_subdirectory_search_uses_parent_ckignore` to prevent future regressions
- **Dependencies**: Bumped vite from 5.4.20 to 5.4.21 in docs-site (PR #82)

## [0.7.0] - 2025-10-13

### Added
- **Chunk-level incremental indexing**: Smart caching system that reuses embeddings for unchanged chunks, dramatically improving reindexing performance (80-90% cache hit rate for typical code changes)
- **Content-aware cache invalidation**: Hash-based invalidation using blake3(chunk_text + leading_trivia + trailing_trivia) ensures doc comment and whitespace changes properly invalidate cache
- **Model compatibility enforcement**: Prevents silent embedding corruption by validating model consistency across indexing operations with clear error messages and recovery guidance
- **Chunk hash versioning**: Manifest tracking of hash scheme version (v2) for future compatibility and reliable version detection

### Performance
- **Selective re-embedding**: Only changed chunks are re-embedded; unchanged chunks reuse cached embeddings from previous index
- **Cache hit validation**: Both hash match AND dimension match required before reusing cached embeddings
- **Typical performance**: For code changes affecting 10-20% of file chunks, 80-90% of embeddings are reused from cache

### Technical
- **Blake3 hashing**: Fast cryptographic hashing of chunk content including all trivia for reliable change detection
- **Sidecar-based cache**: Old sidecars loaded into memory to build chunk_hash → embedding cache for efficient lookup
- **Model validation**: Index operations validate embedding model matches existing index and return actionable errors on mismatch
- **Backward compatibility**: Existing manifests auto-upgrade to chunk_hash_version v2 on load
- **Comprehensive testing**: All 181 tests passing with full coverage of cache invalidation, model validation, and version tracking

## [0.6.0] - 2025-10-12

### Added
- **MCP Server**: Full Model Context Protocol implementation with stdio transport for AI agent integration
- **Pagination Support**: Cursor-based pagination for all search modes (page_size: 1-200, default: 50)
- **Session Management**: TTL-based session cleanup for paginated results with automatic expiration (60s)
- **MCP Search Tools**: `semantic_search`, `regex_search`, `hybrid_search`, `lexical_search` with unified interface
- **MCP Index Tools**: `index_status`, `reindex`, `health_check` for index management
- **CLI Heatmap Visualization**: Color-coded similarity scores with RGB gradient highlighting (red→yellow→green)
- **Enhanced Visual Output**: Unicode box drawing and sophisticated match highlighting in CLI
- **Near-Miss Tracking**: Track closest result below threshold for better search feedback
- **Fallback Strategies**: Automatic fallback from semantic to lexical search when embeddings unavailable
- **Graceful Error Handling**: Skip stale index entries when files no longer exist

### Fixed
- **Mixed Line Endings**: Proper handling of Unix (\n), Windows (\r\n), and Mac (\r) line endings
- **Span Validation**: Prevent invalid spans with zero line numbers using `Span::new()` validation
- **Streaming File Operations**: Memory-efficient line extraction without loading entire files
- **PDF Content Resolution**: Better content path resolution and caching for PDF files

### Technical
- **MCP Protocol**: Full implementation with tool discovery, validation, and error handling
- **Session Cleanup**: LRU-based eviction with periodic cleanup task (every 30s)
- **Streaming Reads**: Optimized `extract_lines_from_file()` for minimal memory footprint
- **SearchResults Enhancement**: Added `closest_below_threshold` field for improved UX
- **Comprehensive Testing**: 7 new MCP integration tests covering pagination, validation, and edge cases
- **Line Ending Support**: `split_lines_with_endings()` tracks exact byte lengths per line
- **TUI Refactoring**: Extracted TUI functionality into dedicated `ck-tui` crate (3,084 lines)
- **Modular Architecture**: Clean separation of TUI components with public API
- **Config Persistence**: TUI preferences saved to `~/.config/ck/tui.json`

### Breaking Changes
- **Span Construction**: Use `Span::new()` for validated construction instead of struct literals (backward compatible via `Span::new_unchecked()`)

## [0.5.3] - 2025-09-29

### Added
- **`.ckignore` file support**: Automatic creation of `.ckignore` file with sensible defaults for persistent exclusion patterns
- **Media file exclusions**: Images (png, jpg, gif, svg, etc.), videos (mp4, avi, mov, etc.), and audio files (mp3, wav, flac, etc.) excluded by default
- **Config file exclusions**: JSON and YAML files excluded from indexing by default to reduce noise in search results
- **`--no-ckignore` flag**: Option to bypass `.ckignore` patterns when needed
- **Persistent patterns**: Exclusion patterns persist across searches without needing command-line flags each time

### Fixed
- **Exclusion pattern persistence** (issue #67): Patterns now persist in `.ckignore` instead of requiring `--exclude` flags on every search
- **Media file indexing** (issue #66): Images, videos, and other binary files no longer indexed by default
- **Config file noise** (issue #27): JSON/YAML config files excluded to focus search on actual code

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
- **Enhanced error messages**: User-friendly error messages with exact commands to resolve issues (e.g., "run `ck --clean .` then rebuild")
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