---
title: Embedding Models
description: Compare BGE-Small, Nomic V1.5, and Jina Code embedding models for ck. Understand chunk sizes, context windows, and performance trade-offs.
---

# Embedding Models

Choose the right embedding model for your semantic search needs.

## Available Models

### BGE-Small (Default)

```bash
ck --index --model bge-small .
```

**Specifications:**
- Chunk size: 400 tokens
- Model capacity: 512 tokens
- Dimensions: 384
- Size: ~80MB

**Best for:**
- General code search
- Fast indexing
- Smaller codebases
- Quick iteration

**Pros:**
- Fastest indexing
- Smallest model download
- Good general understanding
- Low memory usage

**Cons:**
- Smaller chunks may split large functions
- Lower context window

### Nomic V1.5

```bash
ck --index --model nomic-v1.5 .
```

**Specifications:**
- Chunk size: 1024 tokens
- Model capacity: 8192 tokens
- Dimensions: 768
- Size: ~500MB

**Best for:**
- Large functions
- Documentation-heavy code
- Complex code structures
- Long-context understanding

**Pros:**
- Large context window (8K tokens)
- Better for big functions
- Handles documentation well
- Strong semantic understanding

**Cons:**
- Slower indexing
- Larger model download
- Higher memory usage

### Jina Code

```bash
ck --index --model jina-code .
```

**Specifications:**
- Chunk size: 1024 tokens
- Model capacity: 8192 tokens
- Dimensions: 768
- Size: ~500MB

**Best for:**
- Code-specific searches
- Programming language understanding
- API/function signatures
- Code structure awareness

**Pros:**
- Specialized for code
- Understands programming concepts
- Large context window
- Strong for refactoring

**Cons:**
- Slower indexing
- Larger model download
- May be overkill for simple searches

## Comparison Table

| Feature | BGE-Small | Nomic V1.5 | Jina Code |
|---------|-----------|------------|-----------|
| Chunk Size | 400 tokens | 1024 tokens | 1024 tokens |
| Context Window | 512 tokens | 8K tokens | 8K tokens |
| Dimensions | 384 | 768 | 768 |
| Download Size | ~80MB | ~500MB | ~500MB |
| Index Speed | ⚡⚡⚡ | ⚡⚡ | ⚡⚡ |
| Memory Usage | Low | Medium | Medium |
| Code Understanding | Good | Good | Excellent |
| Large Functions | Fair | Excellent | Excellent |

## Model Selection Guide

### By Project Size

**Small projects (<10K LOC):**
```bash
ck --index --model bge-small .
# Fast, sufficient for small codebases
```

**Medium projects (10K-100K LOC):**
```bash
ck --index --model bge-small .      # Fast iteration
# or
ck --index --model jina-code .      # Better understanding
```

**Large projects (>100K LOC):**
```bash
ck --index --model nomic-v1.5 .     # Large contexts
# or
ck --index --model jina-code .      # Code-specialized
```

### By Code Characteristics

**Many small functions:**
```bash
ck --index --model bge-small .
# 400-token chunks handle small functions well
```

**Large functions/classes:**
```bash
ck --index --model nomic-v1.5 .
# 1024-token chunks avoid splitting
```

**Documentation-heavy:**
```bash
ck --index --model nomic-v1.5 .
# Better for docs and comments
```

**Pure code focus:**
```bash
ck --index --model jina-code .
# Code-specialized understanding
```

## Switching Models

### Check Current Model

```bash
ck --status .
# Shows current model and dimensions
```

### Switch to Different Model

```bash
# Smart switch (rebuilds if needed)
ck --switch-model nomic-v1.5 .

# Force rebuild
ck --switch-model jina-code --force .
```

### Manual Rebuild

```bash
# Remove old index
ck --clean .

# Build with new model
ck --index --model jina-code .
```

## Model Cache Location

Models are downloaded once and cached:

- **Linux/macOS** – `~/.cache/ck/models/`
- **Windows** – `%LOCALAPPDATA%\ck\cache\models\`
- **Fallback** – `.ck_models/models/` in current directory

```bash
# Check cache
ls ~/.cache/ck/models/

# Clear cache (will re-download)
rm -rf ~/.cache/ck/models/
```

## Performance Impact

### Indexing Time

For 1M LOC codebase:

| Model | Time | Notes |
|-------|------|-------|
| bge-small | ~2 min | Fastest |
| nomic-v1.5 | ~4 min | Larger chunks |
| jina-code | ~4 min | Code-specific processing |

### Search Speed

All models have similar search speed (~400-600ms). Differences are in indexing, not search.

### Disk Usage

Index size (typical 1M LOC):

| Model | Size | Notes |
|-------|------|-------|
| bge-small | ~200MB | 384 dimensions |
| nomic-v1.5 | ~400MB | 768 dimensions |
| jina-code | ~400MB | 768 dimensions |

## Best Practices

### Start Simple

```bash
# Begin with default
ck --index .
ck --sem "pattern" src/

# If results aren't great, try specialized model
ck --switch-model jina-code .
```

### Test Different Models

```bash
# Inspect chunking without rebuilding
ck --inspect --model bge-small src/large_file.py
ck --inspect --model nomic-v1.5 src/large_file.py

# Compare results
```

### Consider Trade-offs

- **Fast iteration** – Use `bge-small`
- **Best quality** – Use `jina-code`
- **Balanced** – Use `nomic-v1.5`

## Troubleshooting

### Model Download Fails

```bash
# Check network connection
ping huggingface.co

# Check disk space
df -h ~/.cache/ck/

# Manual retry
rm -rf ~/.cache/ck/models/
ck --index --model bge-small .
```

### Index Size Too Large

```bash
# Use smaller model
ck --switch-model bge-small .

# Exclude unnecessary files
echo "*.md" >> .ckignore
echo "docs/" >> .ckignore
ck --clean .
ck --index .
```

### Results Not Good

```bash
# Try code-specialized model
ck --switch-model jina-code .

# Adjust threshold
ck --sem --threshold 0.5 "pattern" src/

# Use hybrid search
ck --hybrid "pattern" src/
```

## Next Steps

- Learn about [semantic search](/features/semantic-search)
- Check [configuration options](/reference/configuration)
- See [CLI reference](/reference/cli)
- Read [basic usage](/guide/basic-usage)
