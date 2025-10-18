---
title: Output Formats
description: JSON and JSONL output format reference for ck. Complete schemas, field descriptions, processing examples for AI agents and automation.
---

# Output Formats

Complete reference for ck's structured output formats.

## Overview

ck supports three output modes:
- **Standard**: Human-readable terminal output (default)
- **JSON**: Single JSON array with all results
- **JSONL**: Newline-delimited JSON (one object per line)

## JSON Output

### Basic Usage

```bash
ck --json "pattern" src/
ck --json --sem "error handling" src/
ck --json --hybrid "auth" src/
```

### JSON Schema

Returns a JSON array of result objects:

```json
[
  {
    "file": "src/auth.rs",
    "line": 42,
    "content": "fn authenticate_user(token: &str) -> Result<User>",
    "score": 0.847
  },
  {
    "file": "src/db.rs",
    "line": 156,
    "content": "pub fn connect_pool(config: &Config) -> Pool",
    "score": 0.732
  }
]
```

### Field Reference

| Field | Type | Description | Present When |
|-------|------|-------------|--------------|
| `file` | string | Relative file path from search root | Always |
| `line` | integer | Line number in file (1-indexed) | Always |
| `content` | string | Content snippet or full line | Always (unless `--no-snippet`) |
| `score` | number | Relevance score | Semantic/hybrid search with `--scores` |

### Field Details

#### `file`
- **Type**: String
- **Format**: Relative path from current directory
- **Example**: `"src/auth/handler.rs"`, `"lib/utils.py"`
- **Always present**: Yes

#### `line`
- **Type**: Integer
- **Range**: 1 to file length
- **Note**: 1-indexed (first line is 1, not 0)
- **Always present**: Yes

#### `content`
- **Type**: String
- **Default length**: Full line (keyword search) or 200 characters (semantic/hybrid)
- **Customizable**: Use `--snippet-length NUM` to control size
- **Omitted with**: `--no-snippet` flag
- **Always present**: Yes (unless `--no-snippet`)

#### `score`
- **Type**: Float
- **Semantic search range**: 0.0 to 1.0 (0.6 default threshold)
  - `0.85+`: Excellent match
  - `0.70-0.85`: Strong match
  - `0.60-0.70`: Good match (default threshold)
  - `<0.60`: Weak match
- **Hybrid search range**: ~0.01 to 0.05 (RRF scoring)
  - `0.025+`: Strong match
  - `0.02-0.025`: Good match (typical threshold)
  - `0.015-0.02`: Fair match
  - `<0.015`: Weak match
- **Present**: Only with `--scores` flag on semantic/hybrid search
- **Note**: Keyword search does not produce scores

### Examples

#### Basic Keyword Search

```bash
$ ck --json "TODO" src/
```

```json
[
  {
    "file": "src/main.rs",
    "line": 42,
    "content": "// TODO: Implement error handling"
  },
  {
    "file": "src/utils.rs",
    "line": 15,
    "content": "// TODO: Add tests"
  }
]
```

#### Semantic Search with Scores

```bash
$ ck --json --sem --scores "authentication" src/
```

```json
[
  {
    "file": "src/auth.rs",
    "line": 42,
    "content": "fn authenticate_user(token: &str) -> Result<User> {",
    "score": 0.891
  },
  {
    "file": "src/middleware.rs",
    "line": 88,
    "content": "pub fn verify_session(req: &Request) -> bool {",
    "score": 0.742
  }
]
```

#### With Custom Snippet Length

```bash
$ ck --json --sem --snippet-length 100 "database" src/
```

```json
[
  {
    "file": "src/db.rs",
    "line": 24,
    "content": "pub async fn init_connection_pool(config: DatabaseConfig) -> Result<Pool, Error> {\n    let pool ="
  }
]
```

#### Metadata Only (No Snippets)

```bash
$ ck --json --no-snippet --sem "pattern" src/
```

```json
[
  {
    "file": "src/handler.rs",
    "line": 156
  },
  {
    "file": "src/utils.rs",
    "line": 89
  }
]
```

## JSONL Output

### Basic Usage

```bash
ck --jsonl "pattern" src/
ck --jsonl --sem "error handling" src/
ck --jsonl --hybrid "auth" src/
```

### JSONL Format

One JSON object per line (no wrapping array):

```jsonl
{"file":"src/auth.rs","line":42,"content":"fn authenticate_user(token: &str) -> Result<User>","score":0.847}
{"file":"src/db.rs","line":156,"content":"pub fn connect_pool(config: &Config) -> Pool","score":0.732}
```

### Schema

Each line is a JSON object with identical schema to JSON array elements (see JSON Schema above).

### When to Use JSONL

✅ **Use JSONL for:**
- **Streaming processing**: Process results as they arrive
- **Memory efficiency**: Parse one result at a time
- **AI agents**: Standard format for LLM workflows
- **Large result sets**: Avoid loading entire array into memory
- **Error resilience**: One malformed line doesn't break parsing

❌ **Use JSON array for:**
- **Simple scripts**: When loading all results is acceptable
- **Small result sets**: <100 results
- **Tools requiring arrays**: Some JSON processors expect arrays

### Processing Examples

#### Stream Processing (Bash)

```bash
# Process each result immediately
ck --jsonl --sem "pattern" src/ | while read -r line; do
  file=$(echo "$line" | jq -r '.file')
  echo "Match in: $file"
done
```

#### With jq

```bash
# Extract just filenames
ck --jsonl --sem "auth" src/ | jq -r '.file'

# Filter by score
ck --jsonl --sem --scores "pattern" src/ | jq 'select(.score > 0.7)'

# Aggregate results
ck --jsonl --sem "pattern" src/ | jq -s 'group_by(.file) | map({file: .[0].file, count: length})'
```

#### Python Processing

```python
import json
import subprocess

# Stream JSONL results
proc = subprocess.Popen(
    ['ck', '--jsonl', '--sem', 'authentication', 'src/'],
    stdout=subprocess.PIPE,
    text=True
)

for line in proc.stdout:
    result = json.loads(line)
    print(f"{result['file']}:{result['line']}")
```

#### Node.js Processing

```javascript
import { spawn } from 'child_process';
import { createInterface } from 'readline';

const ck = spawn('ck', ['--jsonl', '--sem', 'error handling', 'src/']);
const rl = createInterface({ input: ck.stdout });

for await (const line of rl) {
  const result = JSON.parse(line);
  console.log(`${result.file}:${result.line}`);
}
```

## Output Flags

### Controlling Content

| Flag | Effect |
|------|--------|
| `--no-snippet` | Omit `content` field (metadata only) |
| `--snippet-length NUM` | Set `content` length to NUM characters |
| `--scores` | Include `score` field (semantic/hybrid only) |

### Combining Flags

```bash
# Metadata + scores only
ck --jsonl --no-snippet --scores --sem "pattern" src/

# Custom snippet size with scores
ck --json --snippet-length 150 --scores --sem "auth" src/

# Minimal output for large-scale processing
ck --jsonl --no-snippet --sem "pattern" src/
```

## Use Cases

### AI Agent Integration

JSONL is ideal for AI coding assistants:

```bash
# Claude Desktop via MCP uses JSONL internally
# Direct CLI usage:
ck --jsonl --sem "error handling" src/ | process_with_llm
```

### Data Analysis

```bash
# Count matches per file
ck --jsonl --sem "database" src/ | jq -r '.file' | sort | uniq -c

# Find high-scoring results
ck --jsonl --sem --scores "auth" src/ | jq 'select(.score > 0.8)'

# Export for spreadsheet
ck --jsonl --sem --scores "pattern" src/ | jq -r '[.file, .line, .score] | @csv'
```

### CI/CD Pipelines

```bash
# GitHub Actions: Check for security patterns
results=$(ck --json --hybrid "sql injection" src/)
count=$(echo "$results" | jq 'length')
if [ "$count" -gt 0 ]; then
  echo "Security issues found: $count"
  exit 1
fi
```

### Custom Tooling

```bash
# Build custom search wrapper
#!/bin/bash
ck --jsonl --sem "$1" src/ | \
  jq --arg query "$1" '{query: $query, file: .file, line: .line}' | \
  post_to_dashboard
```

## Performance Considerations

### JSON vs JSONL

| Aspect | JSON Array | JSONL |
|--------|-----------|-------|
| Memory | Loads all results | Streams line-by-line |
| Processing | Parse once | Parse per line |
| Large results | May exhaust memory | Constant memory |
| Compatibility | Universal | Line-oriented tools |

### Optimizing Output Size

```bash
# Smallest output
ck --jsonl --no-snippet --sem "pattern" src/

# Reduce snippet size
ck --jsonl --snippet-length 50 --sem "pattern" src/

# Limit results
ck --jsonl --sem --topk 10 "pattern" src/
```

## Error Handling

### Invalid JSON Lines

JSONL streams may be interrupted (Ctrl+C, errors). Handle gracefully:

```bash
ck --jsonl --sem "pattern" src/ | while read -r line; do
  if ! echo "$line" | jq . >/dev/null 2>&1; then
    echo "Skipping invalid line: $line" >&2
    continue
  fi
  # Process valid line
done
```

### Empty Results

```bash
# JSON: Empty array
[]

# JSONL: No output (empty stream)
```

## Next Steps

- Learn [basic usage](/guide/basic-usage) patterns
- Try [semantic search](/features/semantic-search)
- Explore [MCP integration](/features/mcp-integration) for AI agents
- Check [CLI reference](/reference/cli) for all flags
