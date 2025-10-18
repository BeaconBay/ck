---
title: Configuration
description: Configure ck's indexing behavior with .ckignore files, exclusion patterns, and project-specific settings for optimal semantic search.
---

# Configuration

Configure ck's indexing and exclusion behavior.

## .ckignore File

Control which files are excluded from indexing using `.ckignore` in your repository root.

### Syntax

Uses gitignore-style patterns:

```txt
# .ckignore syntax
# Comments start with #

# Exclude directories
node_modules/
target/
dist/
.git/

# Exclude file patterns
*.log
*.tmp
*.bak

# Exclude specific files
.env
config/secrets.yaml

# Negation (include despite parent exclusion)
!important.log

# Wildcards
temp_*.txt
**/*.test.js
```

### Default Exclusions

Auto-created `.ckignore` includes:

```txt
# Default .ckignore patterns
# Images
*.png
*.jpg
*.jpeg
*.gif
*.svg
*.ico
*.webp

# Videos
*.mp4
*.avi
*.mov
*.mkv
*.webm

# Audio
*.mp3
*.wav
*.flac
*.aac

# Archives
*.zip
*.tar
*.gz
*.rar
*.7z

# Binaries
*.exe
*.dll
*.so
*.dylib

# Config files
*.json
*.yaml
*.yml
*.toml

# Build artifacts
target/
dist/
build/
node_modules/
```

### Editing .ckignore

```bash
# Create or edit
vim .ckignore

# Add patterns
echo "logs/" >> .ckignore
echo "*.cache" >> .ckignore

# Rebuild index to apply changes
ck --clean .
ck --index .
```

## Exclusion Layers

ck combines multiple exclusion sources (all additive):

### 1. Default Exclusions

Built-in patterns (binaries, common artifacts).

### 2. .gitignore

Automatically respected unless `--no-ignore` is used.

### 3. .ckignore

Project-specific semantic search exclusions.

### 4. CLI Exclusions

Command-line `--exclude` flags.

### Examples

```bash
# All layers active (default)
ck --sem "pattern" .

# Skip .gitignore only
ck --no-ignore --sem "pattern" .

# Skip .ckignore only
ck --no-ckignore --sem "pattern" .

# Skip both ignore files
ck --no-ignore --no-ckignore --sem "pattern" .

# Add CLI exclusions
ck --exclude "temp/" --sem "pattern" .

# Multiple CLI exclusions
ck --exclude "*.test.js" --exclude "fixtures/" --sem "pattern" .
```

## Index Location

Indexes stored in `.ck/` directories:

```
project/
├── src/
├── .ck/                    # Index directory (safe to delete)
│   ├── embeddings.json     # Embedding vectors
│   ├── ann_index.bin       # Vector index
│   ├── tantivy_index/      # Keyword search index
│   └── manifest.json       # Index metadata
├── .ckignore               # Exclusion patterns
└── .gitignore
```

### Index Management

```bash
# Check index status
ck --status .

# View index details
ck --status src/

# Remove index
rm -rf .ck/

# Or use clean command
ck --clean .
```

## Index Metadata

Index manifest stores:
- Embedding model used
- Model dimensions
- Creation timestamp
- File hashes for delta indexing

View with:
```bash
cat .ck/manifest.json
```

## Configuration Best Practices

### What to Exclude

✅ **Exclude:**
- Generated files (build artifacts)
- Media files (images, videos, audio)
- Large data files
- Config files (JSON, YAML)
- Dependencies (node_modules, vendor)
- Test fixtures
- Documentation (if not relevant to code search)

❌ **Don’t exclude:**
- Source code
- Important configuration (if you search it)
- Tests (if you want to find them)
- Documentation (if you search it)

### Example .ckignore for Different Projects

**JavaScript/Node.js:**
```txt
# .ckignore for JavaScript/Node.js projects
node_modules/
dist/
build/
*.json
*.yaml
*.log
.next/
.nuxt/
coverage/
```

**Rust:**
```txt
# .ckignore for Rust projects
target/
Cargo.lock
*.json
*.toml
*.log
```

**Python:**
```txt
# .ckignore for Python projects
__pycache__/
*.pyc
venv/
.venv/
dist/
build/
*.egg-info/
.pytest_cache/
```

**Go:**
```txt
# .ckignore for Go projects
vendor/
*.mod
*.sum
bin/
```

## Performance Tuning

### Reduce Index Size

```bash
# Exclude documentation
echo "docs/" >> .ckignore
echo "*.md" >> .ckignore

# Exclude tests
echo "tests/" >> .ckignore
echo "*_test.rs" >> .ckignore

# Rebuild
ck --clean .
ck --index .
```

### Faster Indexing

```bash
# Exclude large directories first
echo "node_modules/" > .ckignore
echo "target/" >> .ckignore

# Use smaller model
ck --index --model bge-small .

# Index specific paths
ck --index src/ lib/
```

## Troubleshooting

### Files Not Indexed

```bash
# Check if file is excluded
cat .ckignore
cat .gitignore

# Try without ignore files
ck --no-ignore --no-ckignore --sem "pattern" .

# Check if file is binary
file path/to/file
```

### Too Many Files Indexed

```bash
# Check index status
ck --status .

# Add exclusions to .ckignore
vim .ckignore

# Rebuild
ck --clean .
ck --index .
```

### Index Too Large

```bash
# Check .ck/ directory size
du -sh .ck/

# Exclude unnecessary files
echo "docs/" >> .ckignore
echo "*.md" >> .ckignore

# Use smaller embedding model
ck --switch-model bge-small .
```

## Next Steps

- Learn about [embedding models](/reference/models)
- Check [CLI reference](/reference/cli)
- See [basic usage](/guide/basic-usage)
- Explore [semantic search](/features/semantic-search)
