# Architecture Improvements for Reliability & Maintainability

## Overview

This document describes the architectural improvements made to address two critical issues identified by code analysis:

1. **Reliability & Setup**: Indexing and ONNX runtime downloads could fail silently or require manual intervention
2. **CLI Maintainability**: The monolithic command-line entry point was hard to extend or test

## Improvements Implemented

### 1. Enhanced Error Handling System

#### Context-Aware Error Types (`ck-cli/src/error.rs`)

We've introduced a comprehensive error handling system that provides:

- **Structured error types** with contextual information
- **User-friendly error messages** with suggestions for recovery
- **Clear failure reasons** instead of generic error strings

```rust
pub enum CkError {
    IndexingFailed {
        path: PathBuf,
        reason: String,
        suggestion: Option<String>,
    },
    ModelDownloadFailed {
        model: String,
        reason: String,
        offline_fallback: Option<String>,
    },
    // ... more specific error types
}
```

Benefits:
- Users get actionable error messages with recovery suggestions
- Automation tools can parse structured errors programmatically
- Debugging is easier with clear error chains

### 2. Robust Model Download System

#### Retry Logic with Exponential Backoff (`ck-embed/src/download.rs`)

The new `ModelDownloader` provides:

- **Automatic retry** with exponential backoff (default: 3 attempts)
- **Timeout protection** to prevent hanging downloads
- **Offline mode support** with cache validation
- **Progress callbacks** for user feedback

```rust
pub struct ModelDownloadConfig {
    pub max_retries: u32,        // Default: 3
    pub timeout: Duration,       // Default: 5 minutes
    pub cache_dir: PathBuf,      // Platform-specific cache
    pub offline_mode: bool,      // Work without network
    pub verbose: bool,           // Detailed output
}
```

Features:
- Downloads automatically retry on network failures
- Clear feedback during download progress
- Cached models are validated before use
- Offline mode with pre-download support: `ck --download-model <name>`

### 3. Modular Command Architecture

#### Command Pattern Implementation (`ck-cli/src/commands/`)

The monolithic CLI has been refactored into modular commands:

```
ck-cli/src/commands/
├── mod.rs         # Command trait and context
├── index.rs       # Indexing command
├── search.rs      # Search command
├── status.rs      # Status command
├── clean.rs       # Clean command
└── inspect.rs     # Inspect command
```

Each command:
- Implements the `Command` trait with `execute()` and `validate()`
- Has its own focused responsibility
- Can be tested in isolation
- Shares common context and error handling

#### Command Dispatcher (`ck-cli/src/dispatcher.rs`)

The dispatcher provides:
- **Central routing** of CLI arguments to commands
- **Consistent validation** before execution
- **Shared context** management (verbose, quiet, progress settings)

### 4. Offline Operation Support

New features for offline/automated environments:

```bash
# Pre-download models for offline use
ck --download-model BAAI/bge-small-en-v1.5
ck --download-model nomic-embed-text-v1.5

# Validate offline setup
ck --offline --status

# Use offline mode (no network access)
ck --offline --index .
ck --offline --sem "search query"
```

### 5. Better Progress and Feedback

Enhanced user feedback throughout operations:

- **Granular progress bars** for file and chunk processing
- **Clear status messages** at each stage
- **Spinner indicators** for long operations
- **Verbose mode** for debugging

## Usage Examples

### Reliable Indexing with Retry

```bash
# Index with automatic retry on failures
ck --index --retry-downloads .

# Index in offline mode (uses cached models only)
ck --offline --index .

# Verbose output for debugging
ck --verbose --index src/
```

### Pre-downloading Models for CI/CD

```bash
# In CI setup phase
ck --download-model BAAI/bge-small-en-v1.5
ck --download-model nomic-embed-text-v1.5

# In CI test phase (offline)
ck --offline --index .
ck --offline --sem "test query" src/
```

### Error Recovery Examples

When indexing fails, users now see:

```
❌ Indexing failed for /path/to/project: Permission denied
💡 Suggestion: Check file permissions or run with appropriate privileges

❌ Failed to download model 'nomic-embed-text-v1.5': Network timeout
💡 Offline fallback: Pre-download the model manually or use --offline mode with cached models
🔄 Retry with: ck --retry-downloads

❌ Model 'unknown-model' not found
📋 Available models:
  • BAAI/bge-small-en-v1.5
  • nomic-embed-text-v1.5
  • jina-embeddings-v2-base-code
```

## Testing the Improvements

The modular architecture enables better testing:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_index_command() {
        let cmd = IndexCommand::new(PathBuf::from("test_data"));
        assert!(cmd.validate().is_ok());
        // Test execution with mock embedder
    }

    #[tokio::test]
    async fn test_model_download_retry() {
        let config = ModelDownloadConfig {
            max_retries: 2,
            timeout: Duration::from_secs(10),
            ..Default::default()
        };
        // Test retry logic with network simulation
    }
}
```

## Migration Path

For existing users:

1. **No breaking changes**: All existing CLI commands work as before
2. **New optional features**: Retry and offline modes are opt-in
3. **Better defaults**: Automatic retry improves reliability without configuration

## Future Enhancements

Potential improvements building on this foundation:

1. **Plugin System**: External commands as plugins
2. **Configuration Files**: `.ckrc` for project-specific settings
3. **Parallel Downloads**: Download multiple models concurrently
4. **Model Verification**: Checksum validation for downloaded models
5. **Command Aliases**: User-defined shortcuts for common operations

## Summary

These architectural improvements address the core issues:

- **Reliability**: Automatic retry, offline support, and clear error recovery
- **Maintainability**: Modular commands, testable components, and clean separation of concerns
- **User Experience**: Better error messages, progress feedback, and recovery suggestions
- **Automation**: Structured errors, offline mode, and pre-download capabilities

The refactored architecture makes `ck` more robust for production use while maintaining backward compatibility and improving developer experience.