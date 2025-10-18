---
title: Debugging Production Issues
description: Use semantic search to quickly locate bug sources, trace error paths, and understand production issues. Debug faster by finding relevant code without knowing exact names.
---

# Debugging Production Issues

**Goal**: Quickly locate and understand the source of production bugs using semantic search, even when you don't know the exact function or variable names.

**Difficulty**: Beginner to Intermediate

## Prerequisites

Index your codebase:

```bash
ck --index /path/to/project
```

## Why ck for Debugging?

### Traditional Debugging Workflow
1. See error message
2. Guess keywords from stack trace
3. `grep` for exact strings
4. Often miss related code
5. Repeat until you find it

### ck-Enhanced Workflow
1. See error message
2. Describe the **concept** semantically
3. Find all related code immediately
4. Understand context faster
5. Fix root cause, not just symptoms

## Debugging Workflows

### Workflow 1: From Error Message to Source

**Error in production logs**:
```
Error: Database connection timeout after 30s
  at /app/services/database.js:142
  at processTicksAndRejections (internal/process/task_queues.js:95)
```

#### Step 1: Find error handling code

```bash
ck --sem "database connection timeout error" . --limit 10
```

**What you'll find**:
- Connection initialization
- Timeout configuration
- Error handling
- Retry logic

#### Step 2: Understand connection management

```bash
ck --sem "database connection pool management" . --limit 15
```

**Look for**:
- Pool size configuration
- Connection lifecycle
- Resource cleanup
- Health checks

#### Step 3: Find timeout configuration

```bash
ck --sem "connection timeout config seconds" . --limit 10
```

**Discover**:
- Where timeout is set
- Default vs configured values
- Environment variables
- Configuration files

### Workflow 2: Tracing a Bug Through the Stack

**Bug report**: "Users sometimes see wrong data in their dashboard"

#### Step 1: Find dashboard data loading

```bash
ck --sem "dashboard data fetch user" . --limit 10
```

#### Step 2: Trace data source

```bash
ck --sem "user dashboard query database" . --limit 15
```

#### Step 3: Check caching layer

```bash
ck --sem "cache user data invalidation" . --limit 10
```

#### Step 4: Find race conditions

```bash
ck --sem "concurrent access shared state" . --limit 10
```

**Discovery**: Found cache invalidation happens **after** query, causing stale data race condition.

### Workflow 3: Understanding Null Pointer Errors

**Error**: `NullPointerException at UserService.java:234`

#### Step 1: Find the service method

```bash
ck --hybrid "UserService" . --limit 5
```

#### Step 2: Find null checks nearby

```bash
ck --sem "null check validation user object" . --limit 15 --threshold 0.65
```

#### Step 3: Find where object is created

```bash
ck --sem "user object creation initialization" . --limit 10
```

#### Step 4: Check object lifecycle

```bash
ck --sem "user object cleanup destroy null" . --limit 10
```

**Discovery**: Object accessed after being nullified in cleanup method.

## Real-World Debugging Examples

### Example 1: Memory Leak

**Symptom**: Application memory grows until OOM crash

#### Investigation with ck:

```bash
# Find memory allocation patterns
ck --sem "memory allocation large buffer" . --limit 20

# Find cleanup and disposal
ck --sem "cleanup dispose free memory" . --limit 15

# Check for event listener leaks
ck --sem "event listener registration removal" . --limit 15

# Look for circular references
ck --sem "circular reference retain cycle" . --limit 10
```

**Found**: Event listeners registered but never removed, causing memory leak.

**Fix location**:
```javascript
// src/components/Dashboard.jsx:89
// Missing cleanup:
useEffect(() => {
  eventBus.on('update', handleUpdate);
  // MISSING: return () => eventBus.off('update', handleUpdate);
}, []);
```

### Example 2: Race Condition

**Symptom**: Intermittent failures in payment processing

#### Investigation:

```bash
# Find payment processing code
ck --sem "payment process transaction" . --limit 10

# Look for concurrent operations
ck --sem "concurrent async parallel payment" . --limit 15

# Check locking mechanisms
ck --sem "lock mutex transaction isolation" . --limit 10

# Find state management
ck --sem "payment state status update" . --limit 15
```

**Found**: Two async operations updating payment status without locking.

**Fix**: Add transaction-level locking:
```python
# Before (race condition)
payment = get_payment(payment_id)
payment.status = "completed"
save_payment(payment)

# After (fixed)
with transaction_lock(payment_id):
    payment = get_payment(payment_id)
    payment.status = "completed"
    save_payment(payment)
```

### Example 3: Performance Regression

**Symptom**: API response time increased from 100ms to 5s after deployment

#### Investigation:

```bash
# Find the slow endpoint
ck --sem "api endpoint handler user list" . --limit 5

# Check for N+1 queries
ck --sem "loop database query select" . --limit 20

# Find caching code
ck --sem "cache memoize performance" . --limit 15

# Check recent changes to queries
ck --sem "database query join user" . --limit 10
```

**Found**: Added related data fetch inside loop (N+1 problem).

**Before regression**:
```javascript
const users = await User.findAll();
// Sends 1 query
```

**After regression** (caused slowdown):
```javascript
const users = await User.findAll();
for (const user of users) {
  user.profile = await Profile.findByUserId(user.id);  // N queries!
}
```

**Fix**: Use eager loading:
```javascript
const users = await User.findAll({
  include: [{ model: Profile }]  // 1 query with JOIN
});
```

### Example 4: Security Vulnerability

**Report**: "SQL injection possible in search feature"

#### Investigation:

```bash
# Find search implementation
ck --sem "search query user input" . --limit 10

# Check for string concatenation
ck --sem "sql query string concatenation" . --limit 15

# Find parameterized queries (good pattern)
ck --sem "prepared statement parameterized query" . --limit 10

# Check input sanitization
ck --sem "input sanitization escape sql" . --limit 10
```

**Found**: Search uses string interpolation instead of parameterized query.

**Vulnerable code**:
```python
# src/search.py:45 - VULNERABLE
query = f"SELECT * FROM products WHERE name LIKE '%{search_term}%'"
cursor.execute(query)
```

**Fixed**:
```python
query = "SELECT * FROM products WHERE name LIKE %s"
cursor.execute(query, (f'%{search_term}%',))
```

## Debugging Patterns by Error Type

### Null/Undefined Errors

```bash
# Find where value is used
ck --sem "variable access null undefined" . --limit 15

# Find where it's supposed to be set
ck --sem "variable initialization assignment" . --limit 10

# Check validation
ck --sem "null check validation guard" . --limit 15
```

### Type Errors

```bash
# Find type conversions
ck --sem "type conversion cast string number" . --limit 15

# Find validation logic
ck --sem "type validation check instanceof" . --limit 10

# Find where wrong type is passed
ck --sem "function call parameter type" . --limit 15
```

### Timeout Errors

```bash
# Find timeout configuration
ck --sem "timeout milliseconds config" . --limit 10

# Find slow operations
ck --sem "slow operation blocking synchronous" . --limit 15

# Check retry logic
ck --sem "retry backoff attempt" . --limit 10
```

### Authentication Errors

```bash
# Find auth verification
ck --sem "authentication token verification" . --limit 10

# Check token expiration
ck --sem "token expiration expired check" . --limit 10

# Find session management
ck --sem "session validation active" . --limit 10
```

### Permission Errors

```bash
# Find authorization checks
ck --sem "permission authorization role check" . --limit 15

# Find where check is missing
ck --sem "endpoint handler without auth" . --limit 20 --threshold 0.55

# Check permission configuration
ck --sem "role permission configuration" . --limit 10
```

## Advanced Debugging Techniques

### Technique 1: Compare Similar Code Paths

Find working code to compare with broken code:

```bash
# Find the broken feature
ck --sem "broken feature implementation" . --limit 5

# Find similar working feature
ck --sem "similar working feature implementation" . --limit 5

# Compare to find the difference
diff broken.js working.js
```

### Technique 2: Trace Data Flow

Follow data through the application:

```bash
# 1. Find data entry point
ck --sem "api endpoint receive data" . --limit 5

# 2. Find validation
ck --sem "input validation schema" . --limit 10

# 3. Find processing
ck --sem "data processing transformation" . --limit 10

# 4. Find storage
ck --sem "save database persist" . --limit 10

# 5. Find retrieval
ck --sem "fetch read database query" . --limit 10
```

### Technique 3: Find All Error Paths

Understand how errors can occur:

```bash
# Find try-catch blocks
ck --sem "try catch error handling" . --limit 30

# Find error throwing
ck --sem "throw error exception" . --limit 20

# Find error logging
ck --sem "log error exception" . --limit 25

# Export for analysis
ck --sem "error handling" . --json --limit 50 > error-paths.json
```

### Technique 4: Hybrid Search for Stack Traces

When you have specific function names from stack trace:

```bash
# Semantic search for general area
ck --sem "payment processing error handling" . --limit 10

# Then hybrid search for specific function
ck --hybrid "processPayment" . --limit 20

# Find all callers
ck --sem "call processPayment function" . --limit 30
```

## Debugging with AI Agents

Combine ck with AI for faster debugging:

```python
def debug_with_ai(error_message: str, stack_trace: str) -> str:
    """Use AI + ck to debug production issues."""

    # Extract keywords from error
    keywords = extract_keywords(error_message)

    # Search for relevant code
    related_code = []
    for keyword in keywords:
        results = search_code_semantic(keyword, limit=10)
        related_code.extend(results)

    # Format context for AI
    context = format_for_llm(related_code)

    # Ask AI to analyze
    prompt = f"""
Debug this production error:

Error: {error_message}

Stack Trace:
{stack_trace}

Relevant Code (from semantic search):
{context}

Provide:
1. Root cause analysis
2. Suggested fix
3. Prevention strategies
"""

    return call_llm(prompt)
```

## Debugging Checklist

Use ck to verify each item:

### ✅ Input Validation
```bash
ck --sem "input validation sanitization" . --limit 20
```

### ✅ Error Handling
```bash
ck --sem "try catch error handling" . --limit 30
```

### ✅ Resource Cleanup
```bash
ck --sem "cleanup dispose close connection" . --limit 20
```

### ✅ Null Checks
```bash
ck --sem "null undefined check validation" . --limit 25
```

### ✅ Concurrent Access
```bash
ck --sem "lock mutex concurrent access" . --limit 15
```

### ✅ Logging
```bash
ck --sem "error logging exception" . --limit 20
```

## Post-Mortem Analysis

After fixing a bug, prevent future occurrences:

```bash
# Find similar patterns
ck --sem "similar bug pattern" . --limit 20 --threshold 0.65

# Document the issue
echo "# Bug Post-Mortem: $BUG_ID" > postmortem.md
echo "\n## Affected Code\n" >> postmortem.md
ck --sem "bug related code" . --limit 10 >> postmortem.md

# Find code that needs the same fix
ck --sem "vulnerable pattern needs fix" . --limit 30 --json > needs-fix.json
```

## Tips for Effective Debugging

### 1. Start Broad, Then Narrow

```bash
# Broad semantic search
ck --sem "payment processing" . --limit 20 --threshold 0.6

# Found key function: processPaymentTransaction
# Narrow with hybrid search
ck --hybrid "processPaymentTransaction" . --limit 30
```

### 2. Use Multiple Query Variations

```bash
# Try different phrasings
ck --sem "database connection error" . --limit 10
ck --sem "db connect fail timeout" . --limit 10
ck --sem "connection pool exhausted" . --limit 10
```

### 3. Adjust Threshold Based on Results

```bash
# Too few results? Lower threshold
ck --sem "rare pattern" . --threshold 0.5

# Too many results? Raise threshold
ck --sem "common pattern" . --threshold 0.75
```

### 4. Export for Documentation

```bash
# Document your investigation
ck --sem "bug related code" . --json > investigation.json

# Share with team
ck --sem "bug fix needed" . > bug-locations.txt
```

## Language-Specific Debugging

### Python

```bash
# Find exception handling
ck --sem "try except exception handling" . --limit 20

# Find imports (dependency issues)
ck --sem "import module dependency" . --limit 30

# Find decorators (middleware issues)
ck --sem "decorator route handler" . --limit 15
```

### JavaScript/Node.js

```bash
# Find promise chains (async issues)
ck --sem "promise then catch async" . --limit 20

# Find callback hell
ck --sem "callback nested error first" . --limit 15

# Find event emitters (memory leaks)
ck --sem "event emitter listener on" . --limit 20
```

### Java

```bash
# Find exception handling
ck --sem "try catch finally exception" . --limit 25

# Find resource leaks
ck --sem "stream connection close finally" . --limit 20

# Find thread issues
ck --sem "thread synchronize concurrent" . --limit 15
```

## Next Steps

- **Prevent bugs**: [Security Code Review](/recipes/security-review)
- **Fix patterns**: [Refactoring Similar Patterns](/recipes/refactor-patterns)
- **Automate debugging**: [AI Agent Search Workflows](/recipes/ai-workflows)
- **Understand architecture**: [Exploring New Codebases](/recipes/explore-codebase)

## Related Documentation

- [Semantic Search](/features/semantic-search) - Understanding semantic queries
- [Hybrid Search](/features/hybrid-search) - Combining semantic + exact matching
- [Output Formats](/reference/output-formats) - JSON output for analysis
- [Advanced Usage](/guide/advanced-usage) - Threshold tuning
