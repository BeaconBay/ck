# NPM Distribution Guide

This document explains how ck is distributed via npm and how to publish new versions.

## Overview

ck is a Rust binary distributed via npm using a thin Node.js wrapper. This allows JavaScript/TypeScript developers to install ck without needing Rust installed locally.

## How It Works

1. **Installation**: When users run `npm install -g @beaconbay/ck-search`:
   - The `scripts/install.js` script runs
   - It detects the user's platform (OS + architecture)
   - Downloads prebuilt binary from GitHub releases
   - Falls back to building from source if Cargo is available

2. **Execution**: When users run `ck`:
   - The `cli/ck.js` wrapper spawns the actual Rust binary
   - All arguments are passed through unchanged
   - stdio is inherited for seamless CLI experience

## Supported Platforms

Prebuilt binaries are available for:
- **Linux**: x86_64 (glibc)
- **macOS**: x86_64 (Intel), aarch64 (Apple Silicon)
- **Windows**: x86_64

For other platforms, users need Cargo installed to build from source.

## Publishing to npm

### Prerequisites

1. npm account with access to `@beaconbay` scope
2. Logged in via `npm login`
3. GitHub release with binaries already published

### Steps

1. **Ensure version matches**:
   ```bash
   # package.json version must match Cargo.toml workspace version
   # This happens automatically during release
   ```

2. **Test locally**:
   ```bash
   # Build and test the package locally
   npm pack
   npm install -g beaconbay-ck-search-0.7.0.tgz
   ck --version
   npm uninstall -g @beaconbay/ck-search
   ```

3. **Publish to npm**:
   ```bash
   npm publish --access public
   ```

### Automation (Future)

Consider adding to `.github/workflows/release.yml`:

```yaml
- name: Publish to npm
  if: github.event_name == 'push' && startsWith(github.ref, 'refs/tags/')
  env:
    NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
  run: |
    echo "//registry.npmjs.org/:_authToken=${NODE_AUTH_TOKEN}" > ~/.npmrc
    npm publish --access public
```

## Testing Locally

### Test installation from GitHub release:

```bash
# Clear any previous dist
rm -rf dist/

# Run install script manually
node scripts/install.js

# Test the binary
node cli/ck.js --version
```

### Test fallback to source build:

```bash
# Clear dist and temporarily rename a release to force fallback
rm -rf dist/
# Edit package.json to invalid version temporarily
node scripts/install.js
```

## Troubleshooting

### "No prebuilt binary available"

- Check that GitHub release exists for the version in package.json
- Verify asset names match pattern: `ck-{version}-{target}.{tar.gz|zip}`
- Ensure download URL is publicly accessible

### "Binary not found after installation"

- Check that binary is executable: `ls -la dist/bin/ck`
- On Unix, ensure permissions: `chmod +x dist/bin/ck`
- On Windows, ensure .exe extension is correct

### "cargo build failed"

- User needs Rust installed: https://www.rust-lang.org/tools/install
- Or use a platform with prebuilt binaries

## File Structure

```
ck/
├── package.json          # npm package metadata
├── cli/
│   └── ck.js            # Thin wrapper that spawns binary
├── scripts/
│   ├── install.js       # Download or build binary
│   └── test.js          # Smoke test after install
├── dist/                # Created during install (gitignored)
│   ├── bin/
│   │   └── ck          # The actual Rust binary
│   └── tmp/            # Download cache
└── .npmignore          # What to exclude from npm package
```

## Version Management

**Important**: Keep versions in sync:
- `package.json` version
- `Cargo.toml` workspace version
- Git tag

The release workflow should handle this automatically, but always verify before publishing.

## npm vs Cargo

Users can install via either:

```bash
# Via cargo (Rust developers)
cargo install ck-search

# Via npm (JavaScript/TypeScript developers)
npm install -g @beaconbay/ck-search
```

Both install the same binary, just different distribution channels.
