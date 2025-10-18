---
title: Testing Guidelines
description: Comprehensive testing practices for ck. Unit tests, integration tests, test organization, and quality standards for contributors.
---

# Testing Guidelines

Comprehensive testing practices for ck.

## Test Coverage Goals

- Maintain 65+ tests across workspace
- Test all public APIs
- Test error conditions
- Test edge cases
- Integration tests for CLI

Current coverage:
```bash
cargo test --workspace
# Shows test count and pass rate
```

## Test Organization

### Unit Tests

In same file as code:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function() {
        let result = function();
        assert_eq!(result, expected);
    }
}
```

### Integration Tests

In `tests/` directory:

```
ck-cli/
└── tests/
    ├── integration.rs
    ├── mcp_server.rs
    └── search_modes.rs
```

### Test Data

Store in `test_files/`:

```
test_files/
├── rust/
│   └── sample.rs
├── python/
│   └── sample.py
└── fixtures/
    └── data.json
```

## Running Tests

### All Tests

```bash
# Run all workspace tests
cargo test --workspace

# With output
cargo test --workspace -- --nocapture

# Single-threaded (for debugging)
cargo test --workspace -- --test-threads=1
```

### Specific Tests

```bash
# Specific crate
cargo test --package ck-core

# Specific test
cargo test test_semantic_search

# Pattern matching
cargo test semantic

# Integration tests only
cargo test --test integration
```

### Feature Matrix Testing

```bash
# Install cargo-hack
cargo install cargo-hack

# Test each feature combination
cargo hack test --each-feature --workspace

# Check (faster than test)
cargo hack check --each-feature --workspace
```

### Platform-Specific

```bash
# Skip Windows-specific tests on Unix
#[cfg_attr(not(target_os = "windows"), ignore)]
#[test]
fn windows_only_test() {
    // ...
}

# Run ignored tests
cargo test -- --ignored
```

## Writing Good Tests

### Test Structure

Follow Arrange-Act-Assert pattern:

```rust
#[test]
fn test_search_returns_results() {
    // Arrange
    let query = "error handling";
    let path = Path::new("test_files/");

    // Act
    let results = semantic_search(query, path).unwrap();

    // Assert
    assert!(!results.is_empty());
    assert!(results[0].score > 0.5);
}
```

### Test Naming

Use descriptive names:

```rust
#[test]
fn test_semantic_search_returns_relevant_results() { }

#[test]
fn test_semantic_search_with_threshold_filters_results() { }

#[test]
fn test_semantic_search_handles_empty_directory() { }
```

### Error Testing

Test error conditions:

```rust
#[test]
fn test_search_returns_error_for_invalid_path() {
    let result = search("pattern", Path::new("/nonexistent"));
    assert!(result.is_err());
}

#[test]
#[should_panic(expected = "index not found")]
fn test_search_panics_without_index() {
    search_without_index("pattern");
}
```

### Async Testing

For async functions:

```rust
#[tokio::test]
async fn test_async_operation() {
    let result = async_function().await;
    assert_eq!(result.unwrap(), expected);
}
```

## Test Data Management

### Minimal Test Data

Keep test files small and focused:

```rust
// test_files/rust/minimal.rs
fn simple_function() {
    println!("test");
}
```

### Fixtures

Use fixtures for complex setup:

```rust
fn setup_test_index() -> TempDir {
    let dir = TempDir::new().unwrap();
    // Create test index
    index_directory(dir.path()).unwrap();
    dir
}

#[test]
fn test_with_fixture() {
    let temp_dir = setup_test_index();
    // Test using temp_dir
}
```

### Temporary Files

Use `tempfile` crate:

```rust
use tempfile::TempDir;

#[test]
fn test_with_temp_dir() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path();

    // Test operations

    // Cleanup automatic on drop
}
```

## Snapshot Testing

For complex output (optional with `insta`):

```rust
use insta::assert_debug_snapshot;

#[test]
fn test_search_output() {
    let results = search("pattern", path).unwrap();
    assert_debug_snapshot!(results);
}
```

Update snapshots:
```bash
cargo insta review
```

## Performance Testing

### Benchmarks

Use Criterion (in `benches/`):

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_search(c: &mut Criterion) {
    c.bench_function("semantic_search", |b| {
        b.iter(|| semantic_search(black_box("pattern"), black_box(path)))
    });
}

criterion_group!(benches, benchmark_search);
criterion_main!(benches);
```

Run benchmarks:
```bash
cargo bench
```

### Load Testing

Test with large datasets:

```rust
#[test]
#[ignore] // Skip in normal test runs
fn test_large_codebase() {
    let large_project = Path::new("../large_test_project");
    let results = index_directory(large_project);
    assert!(results.is_ok());
}
```

Run with:
```bash
cargo test -- --ignored large_codebase
```

## Mocking and Test Doubles

### Trait-Based Mocking

```rust
trait EmbedProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>>;
}

struct MockEmbedProvider;
impl EmbedProvider for MockEmbedProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        Ok(vec![0.1, 0.2, 0.3]) // Mock embedding
    }
}

#[test]
fn test_with_mock() {
    let mock = MockEmbedProvider;
    let result = function_using_embedder(&mock);
    assert!(result.is_ok());
}
```

## CI/CD Integration

### GitHub Actions

Tests run on:
- Ubuntu Latest
- Windows Latest
- macOS Latest

Test matrix includes:
- Stable Rust
- All feature combinations
- MSRV check

### Local CI Simulation

Run same checks as CI:

```bash
# Format check
cargo fmt --all --check

# Clippy
cargo clippy --workspace --all-features --all-targets -- -D warnings

# Tests
cargo test --workspace

# Feature matrix
cargo hack test --each-feature --workspace

# MSRV check
cargo hack check --each-feature --locked --rust-version --workspace
```

## Test-Driven Development

### TDD Workflow

1. **Write failing test**:
```rust
#[test]
fn test_new_feature() {
    let result = new_feature();
    assert_eq!(result, expected);
}
```

2. **Run test** (should fail):
```bash
cargo test test_new_feature
```

3. **Implement feature**:
```rust
pub fn new_feature() -> Result<T> {
    // Implementation
}
```

4. **Run test** (should pass):
```bash
cargo test test_new_feature
```

5. **Refactor** if needed

## Coverage Analysis

### Install Tarpaulin

```bash
cargo install cargo-tarpaulin
```

### Generate Coverage Report

```bash
# HTML report
cargo tarpaulin --out Html --output-dir coverage

# View report
open coverage/index.html

# Terminal output
cargo tarpaulin
```

### Coverage Goals

- Aim for >80% line coverage
- 100% coverage for critical paths
- Test all public APIs
- Test all error conditions

## Common Testing Patterns

### Testing Panics

```rust
#[test]
#[should_panic(expected = "message")]
fn test_panic() {
    panic_function();
}
```

### Testing Errors

```rust
#[test]
fn test_error_handling() {
    match error_function() {
        Ok(_) => panic!("Expected error"),
        Err(e) => assert!(e.to_string().contains("expected message")),
    }
}
```

### Parameterized Tests

```rust
#[test]
fn test_multiple_cases() {
    let cases = vec![
        ("input1", "output1"),
        ("input2", "output2"),
    ];

    for (input, expected) in cases {
        assert_eq!(function(input), expected);
    }
}
```

## Troubleshooting Tests

### Flaky Tests

```bash
# Run test multiple times
for i in {1..10}; do cargo test test_name || break; done

# Run with different thread counts
cargo test -- --test-threads=1
```

### Slow Tests

```bash
# Identify slow tests
cargo test -- --nocapture | grep "test result"

# Mark slow tests as ignored
#[ignore]
#[test]
fn slow_test() { }

# Run only fast tests
cargo test
```

### Test Isolation

Ensure tests don’t interfere:

```rust
#[test]
fn test_isolated() {
    let temp_dir = TempDir::new().unwrap();
    // Use temp_dir for all operations
    // Cleanup automatic
}
```

## Best Practices

✅ **Do:**
- Test public APIs
- Test error conditions
- Use descriptive test names
- Keep tests focused and simple
- Use fixtures for complex setup
- Clean up test data
- Run tests before committing

❌ **Don’t:**
- Test private implementation details
- Write tests that depend on external services
- Write tests that depend on other tests
- Commit failing tests
- Skip testing error paths
- Use hardcoded paths

## Next Steps

- Review [development guide](/contributing/development)
- Check [release process](/contributing/release-process)
- See [architecture docs](/reference/architecture)
