---
title: Exploring New Codebases
description: Task-based guide for understanding unfamiliar codebases using ck semantic and hybrid search to discover patterns, architecture, and key components.
---

# Exploring New Codebases

**Goal**: Understand an unfamiliar codebase's structure, patterns, and architecture in 15 minutes.

**Difficulty**: Beginner

## Prerequisites

First, index the project:

```bash
cd /path/to/project
ck --index .
```

::: tip First-Time Indexing
The first run downloads the embedding model (~90MB). Subsequent indexing is fast—typically seconds for small projects, minutes for large ones.
:::

## Step 1: Discover the Entry Point

Find where the application starts:

```bash
ck --sem "application entry point main function" . --limit 5
```

**What to look for**:
- `main()` functions
- Application initialization
- Server setup code

**Example output**:
```
src/main.rs:42
fn main() -> Result<()> {
    let config = load_config()?;
    let server = Server::new(config);
    server.run()
}
```

## Step 2: Identify Core Abstractions

Find the main types and interfaces:

```bash
ck --sem "core domain model types interfaces" . --limit 10
```

**What to look for**:
- Struct/class definitions
- Interface/trait definitions
- Core business logic types

::: tip Refining Your Search
If you get too many results, increase threshold: `--threshold 0.7`

If you get too few, try a more specific query: "user authentication model"
:::

## Step 3: Understand the Architecture

Search for architectural patterns:

```bash
ck --sem "dependency injection service layer" . --limit 10
```

**Common patterns to search for**:
- "dependency injection container"
- "repository pattern database"
- "controller request handling"
- "middleware pipeline"
- "event bus messaging"

## Step 4: Find Configuration

Locate how the app is configured:

```bash
ck --sem "configuration settings environment variables" . --limit 5
```

**What to look for**:
- Config file loading
- Environment variable usage
- Default settings
- Configuration structs

## Step 5: Explore Error Handling

Understand how errors are managed:

```bash
ck --sem "error handling custom errors" . --limit 10
```

**What to look for**:
- Custom error types
- Error propagation patterns
- Logging and monitoring
- Error response formatting

## Step 6: Map External Dependencies

Find integrations with external services:

```bash
ck --sem "database client http client external api" . --limit 10
```

**What to look for**:
- Database connection setup
- HTTP client configuration
- Third-party API integrations
- Message queue connections

## Real-World Example

Exploring a Rust web service:

```bash
# 1. Entry point
ck --sem "main function server startup" . --limit 3
# Found: src/main.rs with Actix-web setup

# 2. Core models
ck --sem "user model struct" . --limit 5
# Found: src/models/user.rs with User struct

# 3. API routes
ck --sem "http routes endpoints" . --limit 10
# Found: src/routes/ directory with route handlers

# 4. Database layer
ck --sem "database queries repository" . --limit 10
# Found: src/db/ with Diesel ORM queries

# 5. Authentication
ck --sem "authentication jwt token validation" . --limit 5
# Found: src/auth.rs with JWT middleware
```

## What You've Learned

After these 6 steps, you should understand:

- **Architecture**: How the application is structured
- **Entry points**: Where execution begins
- **Core concepts**: Main types and abstractions
- **Patterns**: Architectural patterns in use
- **Configuration**: How the app is configured
- **Error handling**: How errors are managed
- **Dependencies**: External services and libraries

## Tips for Better Exploration

### Use Hybrid Search for Known Terms

Once you've discovered key types, use hybrid search:

```bash
ck --hybrid "UserRepository" . --limit 10
```

This combines semantic understanding with exact name matching.

### Adjust Threshold for Precision

- **High precision** (0.75+): Very related code only
- **Balanced** (0.6-0.7): Good mix of relevant results
- **Exploratory** (0.4-0.6): Broader discovery

### Export Results for Analysis

```bash
ck --sem "authentication" . --json > auth-code.json
```

Feed this to an AI agent for deeper analysis.

## Common Patterns by Language

### Python Projects
```bash
ck --sem "__init__ package initialization" . --limit 5
ck --sem "flask routes decorator" . --limit 10
ck --sem "django model class" . --limit 10
```

### JavaScript/TypeScript
```bash
ck --sem "express routes middleware" . --limit 10
ck --sem "react components hooks" . --limit 10
ck --sem "module exports" . --limit 10
```

### Java
```bash
ck --sem "spring boot configuration" . --limit 5
ck --sem "controller rest endpoint" . --limit 10
ck --sem "service layer business logic" . --limit 10
```

### Go
```bash
ck --sem "main package entry point" . --limit 3
ck --sem "http handler function" . --limit 10
ck --sem "struct method receiver" . --limit 10
```

## Next Steps

- **Deep dive into a subsystem**: [Finding Authentication Code](/recipes/find-auth)
- **Look for similar patterns**: [Refactoring Similar Patterns](/recipes/refactor-patterns)
- **Security review**: [Security Code Review](/recipes/security-review)
- **Automate with AI**: [AI Agent Search Workflows](/recipes/ai-workflows)

## Related Documentation

- [Semantic Search](/features/semantic-search) - How semantic search works
- [Hybrid Search](/features/hybrid-search) - Combining semantic + keyword
- [Advanced Usage](/guide/advanced-usage) - Threshold tuning and filters
