---
title: Architecture
description: Technical architecture of ck's modular Rust workspace. Understand crate organization, indexing pipeline, search algorithms, and design decisions.
---

# Architecture

Understanding ck's modular Rust workspace architecture.

## Workspace Structure

ck uses a Cargo workspace with specialized crates:

```
ck/
├── ck-cli/          # Command-line interface and MCP server
├── ck-core/         # Shared types, configuration, utilities
├── ck-engine/       # Search engine implementations
├── ck-index/        # File indexing and sidecar management
├── ck-embed/        # Text embedding providers
├── ck-ann/          # Approximate nearest neighbor indices
├── ck-chunk/        # Text segmentation and parsing
└── ck-models/       # Model registry and configuration
```

## Crate Responsibilities

### ck-cli

**Purpose**: User-facing CLI and MCP server

**Key components:**
- Argument parsing (clap)
- MCP JSON-RPC server
- Output formatting
- User interaction

**Dependencies:** All other crates

### ck-core

**Purpose**: Shared types and utilities

**Key components:**
- SearchResult types
- Configuration structures
- Error types (anyhow)
- Common utilities

**Dependencies:** None (foundation crate)

### ck-engine

**Purpose**: Search implementations

**Key components:**
- RegexEngine: Pattern matching
- SemanticEngine: Vector similarity search
- HybridEngine: Reciprocal Rank Fusion
- Result ranking and scoring

**Dependencies:** ck-core, ck-index, ck-embed, ck-ann

### ck-index

**Purpose**: File indexing and management

**Key components:**
- File discovery and traversal
- Hash-based change detection
- Incremental index updates
- Sidecar file management
- Exclusion pattern handling

**Dependencies:** ck-core, ck-chunk

### ck-embed

**Purpose**: Embedding generation

**Key components:**
- FastEmbed integration
- Multiple model support (BGE, Nomic, Jina)
- Token-aware chunking
- Embedding caching
- Model download management

**Dependencies:** ck-core, ck-models

### ck-ann

**Purpose**: Vector similarity search

**Key components:**
- Approximate Nearest Neighbor indices
- Cosine similarity scoring
- Index persistence
- Vector storage

**Dependencies:** ck-core

### ck-chunk

**Purpose**: Intelligent code chunking

**Key components:**
- Tree-sitter parsing (7+ languages)
- Semantic boundary detection
- Token counting (HuggingFace tokenizers)
- Content-based text detection
- Language detection

**Dependencies:** ck-core, ck-models

### ck-models

**Purpose**: Model configuration

**Key components:**
- Model registry (BGE, Nomic, Jina)
- Token limits and dimensions
- Model aliases
- Chunking configuration

**Dependencies:** ck-core

## Data Flow

### Indexing Flow

```
User Command (ck --index .)
    ↓
ck-cli: Parse arguments
    ↓
ck-index: Discover files
    ↓
ck-chunk: Parse and segment code
    ↓
ck-embed: Generate embeddings
    ↓
ck-ann: Build vector index
    ↓
ck-index: Save index to .ck/
```

### Search Flow (Semantic)

```
User Query (ck --sem "pattern" .)
    ↓
ck-cli: Parse arguments
    ↓
ck-embed: Embed query
    ↓
ck-ann: Find similar vectors
    ↓
ck-engine: Rank and score results
    ↓
ck-cli: Format and display output
```

### Search Flow (Hybrid)

```
User Query (ck --hybrid "pattern" .)
    ↓
ck-cli: Parse arguments
    ↓
[Parallel]
├─ ck-engine (SemanticEngine): Semantic search
└─ ck-engine (RegexEngine): Keyword search
    ↓
ck-engine (HybridEngine): RRF fusion
    ↓
ck-cli: Format and display output
```

## Key Design Patterns

### Error Handling

Uses `anyhow::Result` consistently:

```rust
use anyhow::Result;

pub fn search(query: &str) -> Result<Vec<SearchResult>> {
    // ...
}
```

### Async/Await

Tokio runtime for I/O operations:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // ...
}
```

### Parallel Processing

Rayon for CPU-intensive tasks:

```rust
use rayon::prelude::*;

files.par_iter()
    .map(|f| process_file(f))
    .collect()
```

### Memory-Mapped Files

Efficient large file access:

```rust
use memmap2::Mmap;

let mmap = unsafe { Mmap::map(&file)? };
// Access file contents without full load
```

## Storage Format

### Index Structure

```
.ck/
├── manifest.json          # Index metadata
│   └── { model, dimensions, timestamp, ... }
├── embeddings.json        # Vector embeddings
│   └── { file_path: [vectors...], ... }
├── ann_index.bin          # ANN index (binary)
└── tantivy_index/         # Keyword search index
    ├── meta.json
    └── *.seg files
```

### Sidecar Files

Each source file gets a sidecar:

```
src/
├── main.rs
└── .ck/
    └── main.rs.ck         # Sidecar with chunks and hashes
```

Sidecar contains:
- File hash (for change detection)
- Chunk boundaries
- Embedding IDs
- Metadata

## Performance Considerations

### Indexing Performance

- **Parallel file processing** – Rayon thread pool
- **Incremental updates** – Hash-based change detection
- **Efficient I/O** – Memory-mapped files
- **Smart exclusions** – Early filtering of non-code files

### Search Performance

- **Vector search** – O(log n) with ANN index
- **Keyword search** – Tantivy inverted index
- **Caching** – Embedding cache, model cache
- **Streaming results** – Generator patterns for large result sets

### Memory Management

- **Lazy loading** – Files loaded only when needed
- **Streaming processing** – Process files one at a time
- **Index compression** – Binary format for vectors
- **Model caching** – Reuse loaded models

## Testing Strategy

### Unit Tests

Each crate has unit tests:

```bash
cargo test --workspace
```

### Integration Tests

End-to-end testing in ck-cli:

```bash
cargo test --package ck-cli
```

### Feature Tests

Test each feature combination:

```bash
cargo hack test --each-feature --workspace
```

## Build Process

### Development

```bash
# Build all crates
cargo build --workspace

# Build release
cargo build --workspace --release

# Run tests
cargo test --workspace

# Lint
cargo clippy --workspace --all-features

# Format
cargo fmt --all
```

### Release

```bash
# Version bump (all crates)
# Update Cargo.toml in each crate

# Build and test
cargo test --workspace
cargo clippy --workspace --all-features
cargo fmt --all --check

# Publish to crates.io
cargo publish --package ck-core
cargo publish --package ck-models
# ... (publish in dependency order)
cargo publish --package ck-cli
```

## Extension Points

### Adding New Embedding Models

1. Add model config to `ck-models/src/registry.rs`
2. Implement embedding in `ck-embed`
3. Add CLI flag support in `ck-cli`

### Adding New Languages

1. Add tree-sitter grammar to `ck-chunk/Cargo.toml`
2. Implement parser in `ck-chunk/src/parsers/`
3. Register language in `ck-chunk/src/lib.rs`

### Adding New Search Modes

1. Implement engine in `ck-engine/src/`
2. Add CLI flag in `ck-cli/src/args.rs`
3. Wire up in `ck-cli/src/main.rs`

## Next Steps

- Read [contributing guide](/contributing/development)
- Check [CLI reference](/reference/cli)
- Explore [embedding models](/reference/models)
- See [configuration](/reference/configuration)
