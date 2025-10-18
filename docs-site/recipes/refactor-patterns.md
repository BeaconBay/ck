---
title: Refactoring Similar Patterns
description: Find and refactor duplicated code patterns using semantic search. Identify similar implementations, extract common patterns, and ensure consistency across your codebase.
---

# Refactoring Similar Patterns

**Goal**: Find similar code patterns across a codebase to refactor, deduplicate, or ensure consistency.

**Difficulty**: Intermediate

## Prerequisites

Index your project:

```bash
ck --index /path/to/project
```

## Why Use ck for Refactoring?

Traditional search finds **exact matches**. Semantic search finds **conceptually similar code**, even when:
- Variable names differ
- Languages differ (finding patterns across Python and JavaScript)
- Implementation details vary
- Structure is similar but syntax differs

## Step 1: Find a Pattern to Refactor

Start by locating one instance of the pattern:

```bash
ck --sem "database connection error handling" . --limit 5
```

**Example finding**:
```python
# src/users/db.py:45
try:
    conn = psycopg2.connect(DATABASE_URL)
    result = conn.execute(query)
    return result
except psycopg2.Error as e:
    logger.error(f"Database error: {e}")
    raise DatabaseException(str(e))
finally:
    if conn:
        conn.close()
```

## Step 2: Find Similar Implementations

Search for conceptually similar code:

```bash
ck --sem "try catch database connection cleanup error" . --limit 20 --threshold 0.6
```

**What you might find**:
- Same pattern in different modules
- Similar pattern with different error types
- Slightly different cleanup logic
- Missing error handling in some places

::: tip Threshold for Similarity
- **0.7+**: Very similar implementations
- **0.6-0.7**: Similar concepts, possibly different approaches
- **0.5-0.6**: Related patterns, may need manual review
:::

## Step 3: Compare and Analyze

Export results for detailed analysis:

```bash
ck --sem "database connection error handling" . \
  --json --limit 30 --scores > db-patterns.json
```

Review the scores to see how similar each match is:
- **High scores (0.8+)**: Nearly identical patterns—good refactoring candidates
- **Medium scores (0.6-0.8)**: Similar intent, different implementations
- **Lower scores (0.4-0.6)**: Related but may be intentionally different

## Real-World Example: API Error Handling

### Find inconsistent error responses:

```bash
# Find all error response patterns
ck --sem "http error response json format" . --limit 30 --scores --json > errors.json
```

**What you might discover**:

Different error formats across endpoints:

```typescript
// Pattern A: src/api/users.ts:67
return res.status(400).json({ error: "Invalid user ID" });

// Pattern B: src/api/posts.ts:42
return res.status(400).json({ message: "Invalid post ID", code: "INVALID_ID" });

// Pattern C: src/api/comments.ts:89
throw new BadRequestError("Invalid comment ID");
```

### Refactor to consistent pattern:

Create a standard error handler:

```typescript
// src/utils/errors.ts
export function errorResponse(statusCode: number, message: string, code?: string) {
  return {
    error: {
      message,
      code: code || 'UNKNOWN_ERROR',
      timestamp: new Date().toISOString()
    }
  };
}
```

Find and update all error responses:

```bash
# Find all error response locations
ck --sem "error response status json" . --limit 50 --json > refactor-targets.json
```

## Common Refactoring Patterns

### Pattern 1: Duplicate Error Handling

**Find duplicates**:
```bash
ck --sem "try catch error logging" . --limit 20
```

**Refactor to**: Centralized error handling middleware or decorator

### Pattern 2: Repeated Validation Logic

**Find similar validation**:
```bash
ck --sem "input validation email phone regex" . --limit 15
```

**Refactor to**: Shared validation utilities or schema validators

### Pattern 3: Inconsistent Null Checks

**Find null checking patterns**:
```bash
ck --sem "null undefined check validation" . --limit 20
```

**Refactor to**: Optional chaining, null coalescing, or guard clauses

### Pattern 4: Copy-Pasted CRUD Operations

**Find similar database operations**:
```bash
ck --sem "database create read update delete" . --limit 25
```

**Refactor to**: Generic repository pattern or ORM methods

## Step-by-Step Refactoring Workflow

### 1. Identify the Pattern

```bash
ck --sem "your pattern description" . --limit 10
```

### 2. Find All Instances

```bash
ck --sem "your pattern description" . --limit 50 --json > instances.json
```

### 3. Review for Variations

Look at the score distribution:
```bash
cat instances.json | jq '.[] | {file: .file, line: .line, score: .score}'
```

### 4. Group by Similarity

High-score group (0.75+): Direct refactoring candidates
Medium-score group (0.6-0.75): Review manually
Low-score group (<0.6): Possibly false positives

### 5. Create Abstraction

Design the common interface or utility function.

### 6. Refactor Incrementally

Update one instance, test, then continue.

## Advanced Technique: Cross-Language Patterns

Find similar patterns across different languages:

```bash
# Find retry logic in any language
ck --sem "retry exponential backoff sleep loop" . --limit 20
```

**Might find**:
- Python: `for attempt in range(max_retries):`
- JavaScript: `async function retryWithBackoff()`
- Java: `@Retryable(maxAttempts = 3)`
- Go: `for i := 0; i < maxRetries; i++`

**Use case**: Ensuring consistent retry behavior across microservices written in different languages.

## Finding Anti-Patterns

Search for problematic patterns to refactor:

```bash
# Find potential SQL injection risks
ck --sem "sql query string concatenation format" . --limit 20

# Find resource leaks
ck --sem "file open read no close" . --limit 15

# Find synchronous operations that should be async
ck --sem "blocking synchronous http request" . --limit 15

# Find missing error handling
ck --sem "function no try catch no error check" . --limit 20 --threshold 0.5
```

::: warning Manual Review Required
Semantic search finds **potentially problematic code**. Always review each result—context matters for determining if code is actually problematic.
:::

## Refactoring Example: Logging Consistency

### Before: Inconsistent logging

```bash
ck --sem "logging error info debug" . --limit 30 --json > logging.json
```

**Found patterns**:
```python
# Pattern A
print(f"Error: {error}")

# Pattern B
logger.info("User created")

# Pattern C
sys.stderr.write(f"Failed: {reason}\n")

# Pattern D
logging.debug("Processing item", extra={"item_id": id})
```

### After: Standardized logging

Create standard logger:
```python
# utils/logger.py
import structlog

logger = structlog.get_logger()

def log_error(message: str, **context):
    logger.error(message, **context)

def log_info(message: str, **context):
    logger.info(message, **context)
```

Find and update all logging calls:
```bash
ck --sem "print log error info" . --limit 100 --json > logging-refactor.json
```

## Measuring Refactoring Impact

### Before refactoring:

```bash
# Count instances of the pattern
ck --sem "your pattern" . --limit 100 | wc -l
# Result: 47 instances
```

### After refactoring:

```bash
# Should find the new abstraction
ck --sem "centralized pattern handler" . --limit 10
# Result: 1 shared utility

# Old pattern should be mostly gone
ck --sem "old pattern" . --limit 100 | wc -l
# Result: 3 instances (in progress)
```

## Tips for Effective Pattern Refactoring

### 1. Start with High-Confidence Matches

```bash
ck --sem "pattern" . --threshold 0.75 --scores --limit 20
```

These are most likely true duplicates.

### 2. Use Hybrid Search for Known Names

After finding a pattern, find all references:

```bash
# Found function: handleDatabaseError
ck --hybrid "handleDatabaseError" . --limit 50
```

### 3. Combine with Traditional Tools

```bash
# Semantic search finds the pattern
ck --sem "database connection" . --json > db-code.json

# Grep finds exact function names
grep -r "connect_to_database" .

# Refactor tool applies changes
# (e.g., jscodeshift, comby, or manual IDE refactoring)
```

### 4. Document Your Findings

```bash
# Create a refactoring report
echo "# Refactoring Report: Database Connections" > report.md
echo "\n## Instances Found\n" >> report.md
ck --sem "database connection error handling" . --limit 30 >> report.md
```

## Language-Specific Patterns

### JavaScript/TypeScript

```bash
# Find promise chains to convert to async/await
ck --sem "promise then catch chain" . --limit 20

# Find var declarations to update to const/let
ck --sem "var declaration function scope" . --limit 20

# Find callbacks to convert to promises
ck --sem "callback function error first" . --limit 20
```

### Python

```bash
# Find string concatenation to convert to f-strings
ck --sem "string concatenation percent format" . --limit 20

# Find dict.get() patterns
ck --sem "dictionary get default value" . --limit 15

# Find list comprehensions that could be generator expressions
ck --sem "list comprehension memory" . --limit 15
```

### Java

```bash
# Find try-with-resources opportunities
ck --sem "try finally resource close" . --limit 20

# Find raw types to generify
ck --sem "list arraylist no generic type" . --limit 15

# Find string concatenation to convert to StringBuilder
ck --sem "string concatenation loop" . --limit 15
```

## AI-Assisted Refactoring

Export pattern instances to an AI agent for automated refactoring:

```bash
ck --sem "error handling pattern" . --json --limit 50 > patterns.json
```

Provide to AI:
```
Here are 50 instances of error handling in our codebase.
Please suggest a unified error handling pattern and show
how to refactor each instance.
```

The AI can:
- Identify common elements
- Propose abstraction
- Generate refactored code
- Create migration guide

## Next Steps

- **Security-focused refactoring**: [Security Code Review](/recipes/security-review)
- **Automate pattern detection**: [AI Agent Search Workflows](/recipes/ai-workflows)
- **Debug similar issues**: [Debugging Production Issues](/recipes/debug-production)

## Related Documentation

- [Semantic Search](/features/semantic-search) - Understanding semantic matching
- [Hybrid Search](/features/hybrid-search) - Combining semantic + exact matching
- [Output Formats](/reference/output-formats) - Working with JSON output
- [Advanced Usage](/guide/advanced-usage) - Threshold and filtering
