# Claude Development Guide

This file contains project-specific instructions for Claude and other AI agents working on the ck codebase.
Whenever you actually use ck and it does something unexpected, jot it down in a file could UNEXPECTED.md - supply what you ran, what you expected to happen, what happened instead.


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

All nine crates ship in lockstep. To bump from OLD to NEW:

1. **Workspace `Cargo.toml`** — update two places:
   - `[workspace.package] version = "NEW"`
   - The eight `ck-* = { path = "...", version = "NEW", ... }` lines under `[workspace.dependencies]`
2. **Update `CHANGELOG.md`** with release notes (format below)
3. **Tag** as `X.Y.Z` (no `v` prefix) — `release.yml` does the rest

That's it. Individual crate `Cargo.toml` files inherit the workspace version
via `version.workspace = true` and depend on siblings via `{ workspace = true }`.

**You do NOT need to bump `package.json`.** The `publish-npm` job in
`release.yml` reads `Cargo.toml`'s `[workspace.package] version` at publish
time and stamps `package.json` to match before `npm publish`. The committed
`package.json` version is informational — only the tag + Cargo.toml are
the gate.

### What `release.yml` does on tag push

When tag `X.Y.Z` is pushed:

1. **Create GitHub release** (draft) and verify tag matches `Cargo.toml`
2. **Build binaries** for 5 targets (linux x86_64, macos x86_64+arm64, windows x86_64+arm64), upload as `.tar.gz`/`.zip` assets
3. **Publish 9 crates to crates.io** in dep order, with retry-on-"already-published" and verify-via-API (User-Agent required)
4. **Publish to npm as `@beaconbay/ck-search`** — postinstall script downloads the platform binary from the GitHub release at user install time
5. **Finalize GitHub release** (out of draft)

Required repo secrets: `CARGO_REGISTRY_TOKEN`, `NPM_TOKEN`, `GITHUB_TOKEN` (auto).

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