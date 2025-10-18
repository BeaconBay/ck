---
title: Release Process
description: Create and publish ck releases. Version tagging, quality checks, changelog updates, and publishing workflow for maintainers.
---

# Release Process

How to create and publish new ck releases.

## Version Tagging Convention

**IMPORTANT**: Tags follow the format `X.Y.Z` (NO `v` prefix):

```bash
# Correct format
git tag 0.5.4
git tag 1.0.0

# Incorrect format (do not use)
git tag v0.5.4
```

Always check existing tags first:
```bash
git tag --sort=-version:refname
```

## Pre-Release Checklist

### 1. Quality Checks

Run all checks in order:

```bash
# 1. Lint (fix all warnings)
cargo clippy --workspace --all-features --all-targets -- -D warnings

# 2. Format
cargo fmt --all

# 3. Test
cargo test --workspace

# 4. Feature matrix
cargo hack test --each-feature --workspace

# 5. MSRV check
cargo hack check --each-feature --locked --rust-version --workspace
```

### 2. Update Documentation

- [ ] Update `--help` output if features changed
- [ ] Update README.md with new features
- [ ] Update docs-site if needed
- [ ] Check examples still work

## Version Bump Process

### 1. Update Workspace Version

Edit root `Cargo.toml`:

```toml
[workspace.package]
version = "0.5.4"  # Update this
```

### 2. Update All Crate Versions

```bash
# Find and replace across all Cargo.toml files
find . -name "Cargo.toml" -exec sed -i '' 's/version = "0.5.3"/version = "0.5.4"/g' {} \;

# Verify changes
git diff
```

### 3. Update CHANGELOG.md

Add comprehensive release notes following this format:

```markdown
## [0.5.4] - 2025-10-13

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

**Guidelines:**
- User-facing language in Added/Fixed
- Technical details in Technical section
- Include issue numbers: `(#123)`
- Credit contributors: `(@username)`

### 4. Update Version in Other Docs

```bash
# Check for version references
grep -r "0.5.3" . --exclude-dir=target --exclude-dir=.git

# Update as needed
# - PRD.txt
# - docs-site/package.json
# - Any hardcoded version strings
```

## Creating the Release

### 1. Commit Changes

```bash
# Stage changes
git add .

# Commit with clear message
git commit -m "chore: bump version to 0.5.4"
```

### 2. Create Tag

```bash
# Create annotated tag (NO 'v' prefix)
git tag -a 0.5.4 -m "Release 0.5.4"

# Verify tag
git tag --list | grep 0.5.4
```

### 3. Push to GitHub

```bash
# Push commits
git push origin main

# Push tag
git push origin 0.5.4
```

## Publishing to crates.io

### 1. Publish in Dependency Order

```bash
# Publish foundation crates first
cargo publish --package ck-core
cargo publish --package ck-models

# Then dependent crates
cargo publish --package ck-chunk
cargo publish --package ck-ann
cargo publish --package ck-embed
cargo publish --package ck-index
cargo publish --package ck-engine

# Finally, the CLI
cargo publish --package ck-cli
```

### 2. Verify Publication

```bash
# Check crates.io
open https://crates.io/crates/ck-search

# Test installation
cargo install ck-search --force
ck --version
```

## GitHub Release

### 1. Create Release on GitHub

1. Go to https://github.com/BeaconBay/ck/releases
2. Click “Draft a new release”
3. Select tag: `0.5.4`
4. Release title: `ck 0.5.4`
5. Copy CHANGELOG.md section for this version
6. Publish release

### 2. Attach Binaries (Optional)

If building platform-specific binaries:

```bash
# Build release binaries
cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target x86_64-apple-darwin
cargo build --release --target x86_64-pc-windows-gnu

# Attach to GitHub release
```

## Post-Release Tasks

### 1. Announce Release

- [ ] GitHub Discussions (if enabled)
- [ ] Discord/Slack (if applicable)
- [ ] Twitter/social media (if applicable)

### 2. Update Documentation Sites

- [ ] Deploy docs-site if changes made
- [ ] Update any external documentation links

### 3. Monitor Issues

- [ ] Watch for installation issues
- [ ] Check for regression reports
- [ ] Respond to questions

## Hotfix Process

For urgent bug fixes:

### 1. Create Hotfix Branch

```bash
git checkout -b hotfix/0.5.5 0.5.4
```

### 2. Fix Bug

```bash
# Make minimal changes to fix bug
# Add test for regression
cargo test

# Commit fix
git commit -m "fix: critical bug description (#issue)"
```

### 3. Release Hotfix

```bash
# Bump patch version
# Update CHANGELOG.md
# Follow normal release process

git tag 0.5.5
git push origin 0.5.5
cargo publish --package ck-cli
```

## Versioning Strategy

Follow [Semantic Versioning](https://semver.org/):

### MAJOR (X.0.0)

Breaking changes:
- CLI flag changes that break scripts
- Output format changes
- Index format changes requiring rebuild

### MINOR (0.X.0)

New features (backward compatible):
- New search modes
- New CLI flags
- New embedding models
- Performance improvements

### PATCH (0.0.X)

Bug fixes (backward compatible):
- Bug fixes
- Documentation updates
- Minor performance improvements

## Troubleshooting

### Publication Fails

```bash
# Check if already published
cargo search ck-search

# Verify credentials
cargo login

# Check network
ping crates.io
```

### Tag Conflicts

```bash
# Delete local tag
git tag -d 0.5.4

# Delete remote tag (careful!)
git push origin :refs/tags/0.5.4

# Recreate correctly
git tag 0.5.4
git push origin 0.5.4
```

### Version Mismatch

```bash
# Find all Cargo.toml files
find . -name "Cargo.toml" | xargs grep "version ="

# Verify consistency
# Re-run find/replace if needed
```

## Release Checklist

Use this checklist for each release:

- [ ] All quality checks pass (clippy, fmt, test)
- [ ] Version bumped in all Cargo.toml files
- [ ] CHANGELOG.md updated with release notes
- [ ] README.md updated if features changed
- [ ] `--help` output reflects changes
- [ ] Committed with clear message
- [ ] Tagged with correct format (X.Y.Z, no 'v')
- [ ] Pushed commits and tag to GitHub
- [ ] Published to crates.io (all crates in order)
- [ ] Verified installation works
- [ ] Created GitHub release with notes
- [ ] Announced release (if applicable)

## Next Steps

- Review [development guide](/contributing/development)
- Check [testing guidelines](/contributing/testing)
- See [architecture docs](/reference/architecture)
