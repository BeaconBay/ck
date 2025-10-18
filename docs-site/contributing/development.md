---
title: Development Guide
description: Contribute to ck development. Setup guide, coding standards, pull request process, and development workflow for contributors.
---

# Development Guide

Contributing to ck development.

## Getting Started

### Prerequisites

- Rust 1.89+ (MSRV)
- Cargo
- Git

### Clone and Build

```bash
git clone https://github.com/BeaconBay/ck
cd ck
cargo build --workspace
cargo test --workspace
```

### Run from Source

```bash
# Build and run
cargo run --package ck-cli -- --sem "pattern" test_files/

# Or use the binary directly
./target/debug/ck --sem "pattern" test_files/
```

## Development Workflow

### Pre-Commit Checks

**ALWAYS** run these before committing:

```bash
# 1. Format code
cargo fmt --all

# 2. Run linter (must have no warnings)
cargo clippy --workspace --all-features --all-targets -- -D warnings

# 3. Run tests
cargo test --workspace
```

### Code Style

- Use `anyhow::Result` for error handling
- Follow Rust naming conventions
- Add doc comments for public APIs
- Keep functions focused and small
- Write tests for new features

### Common Patterns

```rust
// Error handling
use anyhow::{Result, Context};

pub fn process_file(path: &Path) -> Result<Data> {
    let content = fs::read_to_string(path)
        .context("Failed to read file")?;
    // ...
}

// Async operations
use tokio;

#[tokio::main]
async fn main() -> Result<()> {
    // async code
}

// Parallel processing
use rayon::prelude::*;

files.par_iter()
    .map(|f| process(f))
    .collect()
```

## Testing

### Run All Tests

```bash
# All tests
cargo test --workspace

# Specific crate
cargo test --package ck-core

# Specific test
cargo test test_semantic_search

# With output
cargo test -- --nocapture
```

### Test Each Feature

```bash
# Install cargo-hack
cargo install cargo-hack

# Test all feature combinations
cargo hack test --each-feature --workspace
```

### Integration Tests

```bash
# Run integration tests
cargo test --package ck-cli --test integration

# Test specific scenario
cargo test --test integration test_mcp_server
```

## Adding Features

### New Embedding Model

1. **Add model config** (`ck-models/src/registry.rs`):
```rust
pub const MY_MODEL: ModelConfig = ModelConfig {
    name: "my-model",
    dimensions: 768,
    max_tokens: 8192,
    // ...
};
```

2. **Implement embedding** (`ck-embed/src/`):
```rust
impl EmbedProvider for MyModel {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Implementation
    }
}
```

3. **Add CLI support** (`ck-cli/src/args.rs`):
```rust
#[derive(ValueEnum, Clone)]
pub enum Model {
    BgeSmall,
    NomicV15,
    MyModel, // Add here
}
```

4. **Write tests**:
```rust
#[test]
fn test_my_model() {
    // Test implementation
}
```

### New Language Support

1. **Add tree-sitter grammar** (`ck-chunk/Cargo.toml`):
```toml
[dependencies]
tree-sitter-mylang = "1.0"
```

2. **Implement parser** (`ck-chunk/src/parsers/mylang.rs`):
```rust
pub fn parse_mylang(source: &str) -> Result<Vec<Chunk>> {
    // Parser implementation
}
```

3. **Register language** (`ck-chunk/src/lib.rs`):
```rust
match extension {
    "ml" => parse_mylang(source)?,
    // ...
}
```

4. **Add tests** with sample files in `test_files/`

### New Search Mode

1. **Implement engine** (`ck-engine/src/my_engine.rs`):
```rust
pub struct MyEngine {
    // fields
}

impl SearchEngine for MyEngine {
    fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        // Implementation
    }
}
```

2. **Add CLI flag** (`ck-cli/src/args.rs`):
```rust
#[arg(long)]
pub my_search: bool,
```

3. **Wire up** (`ck-cli/src/main.rs`):
```rust
if args.my_search {
    let engine = MyEngine::new();
    engine.search(&query)?
}
```

## Debugging

### Enable Logging

```bash
# Set log level
RUST_LOG=debug cargo run -- --sem "pattern" src/

# Specific module
RUST_LOG=ck_engine=trace cargo run -- --sem "pattern" src/
```

### Debug Builds

```bash
# Debug build (with symbols)
cargo build

# Run with debugger
rust-gdb ./target/debug/ck
# or
rust-lldb ./target/debug/ck
```

### Performance Profiling

```bash
# Build with profiling
cargo build --release

# Profile with flamegraph
cargo install flamegraph
cargo flamegraph -- --index large_project/

# Profile with perf
perf record --call-graph dwarf ./target/release/ck --index .
perf report
```

## CI/CD

### GitHub Actions

CI runs on:
- Ubuntu Latest
- Windows Latest
- macOS Latest

CI checks:
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --workspace`
- `cargo hack check --each-feature`

### Local CI Simulation

```bash
# Run all CI checks locally
./scripts/ci-check.sh  # If available

# Or manually:
cargo fmt --all --check
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test --workspace
cargo hack check --each-feature --locked --rust-version --workspace
```

## Documentation

### Code Documentation

```rust
/// Brief description of function.
///
/// # Arguments
///
/// * `query` - The search query
/// * `path` - Path to search
///
/// # Returns
///
/// Vector of search results
///
/// # Errors
///
/// Returns error if path doesn't exist
///
/// # Examples
///
/// ```
/// let results = search("pattern", Path::new("src/"))?;
/// ```
pub fn search(query: &str, path: &Path) -> Result<Vec<SearchResult>> {
    // Implementation
}
```

### Generate Docs

```bash
# Build documentation
cargo doc --workspace --no-deps

# Open in browser
cargo doc --workspace --no-deps --open
```

## Common Tasks

### Update Dependencies

```bash
# Check for outdated deps
cargo outdated

# Update deps
cargo update

# Test after update
cargo test --workspace
```

### Benchmark Changes

```bash
# Run benchmarks
cargo bench

# Compare before/after
cargo bench --bench my_bench -- --save-baseline before
# make changes
cargo bench --bench my_bench -- --baseline before
```

### Cross-Platform Testing

```bash
# Install cross
cargo install cross

# Test on different platforms
cross test --target x86_64-unknown-linux-gnu
cross test --target x86_64-pc-windows-gnu
cross test --target aarch64-apple-darwin
```

## Troubleshooting

### Build Failures

```bash
# Clean and rebuild
cargo clean
cargo build --workspace

# Check specific crate
cargo check --package ck-core

# Verbose output
cargo build --verbose
```

### Test Failures

```bash
# Run specific test with output
cargo test test_name -- --nocapture --test-threads=1

# Update test snapshots (if using insta)
cargo insta review
```

### Clippy Issues

```bash
# See all warnings
cargo clippy --workspace --all-features --all-targets

# Auto-fix some issues
cargo clippy --fix --workspace

# Check specific lint
cargo clippy -- -W clippy::pedantic
```

## Next Steps

- Read [release process](/contributing/release-process)
- Check [testing guidelines](/contributing/testing)
- See [architecture docs](/reference/architecture)
- Review [CLI reference](/reference/cli)
