# Claude Development Guide

This file contains project-specific instructions for Claude and other AI agents working on the ck codebase.

## Release Process

### Version Tagging Convention

**IMPORTANT**: Tags follow the format `X.Y.Z` (NO `v` prefix) to match current standard:

```bash
# Correct format (current standard since 0.3.8+)
git tag 0.4.1
git tag 0.3.9

# Old format (deprecated, do not use)
git tag v0.3.4
```

Always check existing tags first: `git tag --sort=-version:refname`

### Pre-Commit Quality Checks

**ALWAYS** run these commands in order before any commit:

1. **Linting**: `cargo clippy` - Fix all warnings
2. **Formatting**: `cargo fmt` - Format all code  
3. **Testing**: `cargo test` - Ensure all tests pass

### Version Bump Process

When bumping versions:

1. **Update workspace version**: `Cargo.toml` (workspace level)
2. **Update ALL crate versions**: Use find/replace across all `Cargo.toml` files
   ```bash
   find . -name "Cargo.toml" -exec sed -i '' 's/version = "OLD"/version = "NEW"/g' {} \;
   ```
3. **Update documentation versions**: Check `PRD.txt` and other docs
4. **Update CHANGELOG.md**: Add comprehensive release notes (see format below)

### CHANGELOG.md Format

Always update CHANGELOG.md with new releases. Follow this structure:

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added
- **Feature name**: Clear user-facing description
- **Technical capability**: What it enables

### Fixed  
- **Bug description**: What was broken and how it's fixed
- **Performance issue**: Specific improvements made

### Technical
- **Implementation details**: For maintainers and contributors
- **Dependencies**: New dependencies added
```

### Development Notes

- **Test coverage**: Maintain comprehensive test coverage (currently 65+ tests)
- **Cross-platform**: Ensure features work on Windows, macOS, and Linux
- **Performance**: Consider impact on indexing and search performance
- **User experience**: Maintain grep compatibility and intuitive CLI design

### Common Patterns in this Codebase

- **Error handling**: Use `anyhow::Result` consistently
- **Async/await**: Tokio runtime for async operations  
- **Parallel processing**: Rayon for CPU-intensive tasks
- **File I/O**: Memory-mapped files for large data access
- **Configuration**: Workspace-level dependency management

### Quality Standards

- All clippy warnings must be resolved
- Code must be formatted with `cargo fmt`
- All tests must pass
- New features require comprehensive test coverage
- Breaking changes require major version bump
- --help reflects any new features
- README incorporates any new user features (e.g. flags etc)