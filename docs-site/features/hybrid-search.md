---
title: Hybrid Search
description: Combine semantic understanding with keyword precision using Reciprocal Rank Fusion (RRF). Best of both worlds for accurate code search results.
---

# Hybrid Search

Combine semantic understanding with keyword precision using Reciprocal Rank Fusion.

## What is Hybrid Search?

Hybrid search merges semantic and keyword search results to give you the best of both worlds:
- **Semantic** – Finds conceptually related code
- **Keyword** – Ensures query terms are present
- **Fusion** – Ranks results by combined relevance

## Basic Usage

```bash
# Simple hybrid search
ck --hybrid "connection timeout" src/

# With relevance scores
ck --hybrid --scores "cache invalidation" src/

# Filter by confidence (note: RRF scores are typically 0.01-0.05)
ck --hybrid --threshold 0.02 "auth" src/
```

## ⚠️ Important: Understanding Hybrid Thresholds

### Hybrid search uses a different scoring scale than semantic search

Due to Reciprocal Rank Fusion (RRF) mathematics, hybrid scores typically range from **0.01 to 0.05**, not 0.0 to 1.0 like semantic search. This is a known counterintuitive aspect of RRF scoring.

### Threshold Quick Reference

| Search Mode | Score Range | Typical Threshold | Example |
|-------------|-------------|-------------------|---------|
| Semantic | 0.0 - 1.0 | 0.6 (default) | `--sem --threshold 0.7` |
| Hybrid (RRF) | ~0.01 - 0.05 | 0.016 - 0.025 | `--hybrid --threshold 0.02` |

**Why is this?** RRF combines rankings by summing reciprocal ranks: `1/(60 + rank)`. Since ranks start at 1, the maximum contribution per result is `1/61 ≈ 0.016`, making scores much smaller than normalized similarity scores.

## How It Works

1. **Dual search**: Runs semantic AND keyword search in parallel
2. **Ranking**: Each produces ranked results
3. **Fusion**: RRF (Reciprocal Rank Fusion) merges rankings
4. **Output**: Combined results sorted by fused relevance

### Reciprocal Rank Fusion Formula

```bash
RRF_score(doc) = Σ (1 / (k + rank_i))

where:
- k = 60 (constant)
- rank_i = position in each ranking
```

Results appearing high in both rankings get highest scores.

## When to Use Hybrid Search

✅ **Use hybrid when:**
- You need both concept AND keywords present
- Semantic search returns too many false positives
- Keyword search misses related code
- You want balanced precision and recall

### Examples

```bash
# Find timeout code with "connection" keyword
ck --hybrid "connection timeout" src/

# Authentication with specific terms
ck --hybrid "jwt authentication" src/

# Performance with keywords
ck --hybrid "cache optimization" src/

# Security patterns
ck --hybrid "sql injection prevention" src/
```

## Comparison with Other Search Modes

| Feature | Keyword | Semantic | Hybrid |
|---------|---------|----------|--------|
| Exact matches | ✅ | ❌ | ✅ |
| Conceptual matches | ❌ | ✅ | ✅ |
| Requires keywords | ✅ | ❌ | ✅ |
| False positives | Low | Medium | Low |
| Speed | Fastest | Fast | Fast |
| Threshold scale | N/A | 0.0-1.0 | ~0.01-0.05 |

```bash
# Keyword: Finds exact "error" text
ck "error" src/

# Semantic: Finds error handling concepts
ck --sem "error handling" src/

# Hybrid: Finds error handling concepts with "error" keyword
ck --hybrid "error handling" src/
```

## Advanced Options

### Threshold Filtering

```bash
# High confidence only (RRF scale)
ck --hybrid --threshold 0.025 "pattern" src/

# Exploratory (lower confidence, more results)
ck --hybrid --threshold 0.015 "pattern" src/

# Very strict (only top matches)
ck --hybrid --threshold 0.03 "pattern" src/
```

::: tip Choosing the Right Threshold
Start with `--threshold 0.02` for hybrid search. If you get too many results, increase to `0.025` or `0.03`. If too few, decrease to `0.015`.

Use `--scores` to see actual RRF values and calibrate your threshold accordingly.
:::

### Result Limiting

```bash
# Top 10 results
ck --hybrid --topk 10 "pattern" src/

# With threshold
ck --hybrid --topk 20 --threshold 0.02 "pattern" src/
```

### Complete Sections

```bash
# Get full functions
ck --hybrid --full-section "retry logic" src/
```

## Best Practices

### Query Formulation

✅ **Good hybrid queries:**
```bash
ck --hybrid "database connection pool"    # Concept + keywords
ck --hybrid "async timeout handler"       # Specific + descriptive
ck --hybrid "jwt token validation"        # Technology + function
```

❌ **Poor hybrid queries:**
```bash
ck --hybrid "code"                        # Too vague
ck --hybrid "function that does stuff"    # No specific keywords
```

### Performance Tips

```bash
# Limit results for speed
ck --hybrid --topk 10 "pattern" src/

# Use threshold to reduce computation
ck --hybrid --threshold 0.02 "pattern" src/

# Search specific paths
ck --hybrid "pattern" src/core/
```

## Use Cases

### Refactoring

```bash
# Find specific implementation patterns
ck --hybrid "singleton pattern" src/

# Find duplication
ck --hybrid "user validation logic" src/
```

### Security Audits

```bash
# SQL injection risks
ck --hybrid "sql injection" src/

# XSS vulnerabilities
ck --hybrid "xss prevention" src/

# Hardcoded secrets
ck --hybrid "api key|password" src/
```

### API Integration

```bash
# Find API calls
ck --hybrid "rest api client" src/

# Find specific endpoints
ck --hybrid "payment gateway" src/
```

## Troubleshooting

### Too Many Results

```bash
# Increase threshold (RRF scale)
ck --hybrid --threshold 0.03 "pattern" src/

# Limit count
ck --hybrid --topk 5 "pattern" src/

# Use more specific query
ck --hybrid "specific detailed pattern" src/
```

### Too Few Results

```bash
# Lower threshold (RRF scale)
ck --hybrid --threshold 0.015 "pattern" src/

# Try semantic-only
ck --sem "pattern" src/

# Try keyword-only
ck "pattern" src/
```

### Unexpected Results

```bash
# Check with scores to see RRF values
ck --hybrid --scores "pattern" src/

# Try semantic alone (0-1 scale)
ck --sem --scores "pattern" src/

# Try keyword alone
ck "pattern" src/
```

### “My threshold doesn’t seem to work”

Remember: hybrid search uses RRF scoring (~0.01-0.05), not semantic scoring (0-1).

```bash
# ❌ Wrong: Using semantic-style threshold
ck --hybrid --threshold 0.6 "pattern" src/  # Will return 0 results!

# ✅ Correct: Using RRF-scale threshold
ck --hybrid --threshold 0.02 "pattern" src/
```

Use `--scores` to see what range your results fall into, then adjust accordingly.

## Next Steps

- Learn about [semantic search](/features/semantic-search)
- Explore [grep compatibility](/features/grep-compatibility)
- Try [MCP integration](/features/mcp-integration)
- Check [CLI reference](/reference/cli)
