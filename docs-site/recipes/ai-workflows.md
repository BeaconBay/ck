---
title: AI Agent Search Workflows
description: Integrate ck with AI agents for autonomous code analysis. MCP server setup, LLM tool integration, and workflow patterns for Claude, ChatGPT, and custom agents.
---

# AI Agent Search Workflows

**Goal**: Integrate ck with AI agents to enable autonomous code search, analysis, and understanding.

**Difficulty**: Intermediate

::: tip AI-First Design
ck was designed **primarily for AI agents**, not humans. AI agents naturally understand how to tune `--threshold`, `--topk`, and interpret semantic scores. The TUI exists to help humans gain confidence, but the CLI is where ck shines with AI.
:::

## Why ck for AI Agents?

### Traditional Tools

- **grep**: Finds exact matches only
- **cscope/ctags**: Symbol-based, requires exact names
- **AST parsers**: Complex, language-specific

### ck Advantages for AI

1. **Semantic understanding**: Find code by meaning, not just keywords
2. **JSON output**: Perfect for LLM consumption
3. **Adaptive search**: AI agents naturally tune parameters
4. **Near-miss hints**: Guides threshold adjustment
5. **grep compatibility**: Familiar tool AI models know well

## Integration Methods

### Method 1: Model Context Protocol (MCP)

**Best for**: Claude Desktop, Claude Code, or MCP-compatible tools

See the complete MCP setup guide: [MCP Integration](/features/mcp-integration)

**Quick start**:

```json
{
  "mcpServers": {
    "ck": {
      "command": "ck",
      "args": ["--mcp"],
      "cwd": "/path/to/your/project"
    }
  }
}
```

**Capabilities**:

- `search_semantic`: Semantic code search
- `search_hybrid`: Hybrid semantic + keyword
- `search_keyword`: Traditional grep
- `list_indexed_files`: Show indexed files
- `get_file_chunks`: Get semantic chunks from file

### Method 2: Direct CLI Integration

**Best for**: Custom agents, automation scripts, LangChain tools

**Tool definition** (JSON Schema):

```json
{
  "name": "search_code_semantic",
  "description": "Search codebase using semantic understanding. Finds code by meaning, not just exact matches. Returns file paths, line numbers, code snippets, and relevance scores.",
  "parameters": {
    "type": "object",
    "properties": {
      "query": {
        "type": "string",
        "description": "Natural language description of what to find. Examples: 'error handling', 'database connection', 'user authentication'"
      },
      "threshold": {
        "type": "number",
        "description": "Minimum relevance score (0.0-1.0). Default 0.6. Lower = more results, higher = more precise.",
        "default": 0.6
      },
      "limit": {
        "type": "number",
        "description": "Maximum number of results to return. Default 10, max 100.",
        "default": 10
      }
    },
    "required": ["query"]
  }
}
```

**Implementation**:

```python
import subprocess
import json

def search_code_semantic(
    query: str,
    threshold: float = 0.6,
    limit: int = 10
) -> list:
    """Search codebase semantically."""
    cmd = [
        "ck",
        "--sem", query,
        ".",
        "--json",
        "--scores",
        "--threshold", str(threshold),
        "--limit", str(limit)
    ]

    result = subprocess.run(cmd, capture_output=True, text=True)

    if result.returncode != 0:
        raise RuntimeError(f"Search failed: {result.stderr}")

    return json.loads(result.stdout)
```

### Method 3: LangChain Integration

**Best for**: LangChain-based applications

```python
from langchain.tools import Tool
from langchain.agents import initialize_agent, AgentType
from langchain.llms import OpenAI
import subprocess
import json

def ck_search(query: str) -> str:
    """Execute ck semantic search and format results."""
    cmd = ["ck", "--sem", query, ".", "--json", "--limit", "10", "--scores"]
    result = subprocess.run(cmd, capture_output=True, text=True)

    if result.returncode != 0:
        return f"Search failed: {result.stderr}"

    results = json.loads(result.stdout)

    # Format for LLM
    formatted = []
    for r in results:
        formatted.append(
            f"File: {r['file']}:{r['line']}\n"
            f"Score: {r['score']:.3f}\n"
            f"Code: {r['content']}\n"
        )

    return "\n---\n".join(formatted)

# Create LangChain tool
ck_tool = Tool(
    name="SearchCode",
    func=ck_search,
    description=(
        "Search codebase semantically by describing what you're looking for. "
        "Input should be a natural language description like 'error handling' "
        "or 'database connection logic'. Returns relevant code with file locations."
    )
)

# Initialize agent with ck tool
llm = OpenAI(temperature=0)
agent = initialize_agent(
    tools=[ck_tool],
    llm=llm,
    agent=AgentType.ZERO_SHOT_REACT_DESCRIPTION,
    verbose=True
)

# Agent can now search code
response = agent.run("Find the authentication logic in this codebase")
```

## AI Agent Workflow Patterns

### Pattern 1: Iterative Refinement

AI agents naturally refine searches based on results:

```python
# Initial broad search
results = search_code_semantic("authentication", threshold=0.6, limit=10)

if len(results) == 0:
    # No results - lower threshold
    results = search_code_semantic("authentication", threshold=0.4, limit=10)

elif len(results) > 50:
    # Too many results - increase threshold
    results = search_code_semantic("authentication", threshold=0.75, limit=10)

# Found key function, now find usages
if results:
    func_name = extract_function_name(results[0]['content'])
    # Switch to hybrid for exact matching
    usages = search_code_hybrid(func_name, threshold=0.02, limit=50)
```

### Pattern 2: Multi-Stage Analysis

Break complex tasks into stages:

```python
# Stage 1: Find entry point
entry_points = search_code_semantic("main application entry point", limit=5)

# Stage 2: Discover architecture
architecture = search_code_semantic("service layer dependency injection", limit=15)

# Stage 3: Find database code
db_code = search_code_semantic("database queries repository", limit=20)

# Stage 4: Locate error handling
error_handling = search_code_semantic("error handling custom errors", limit=15)

# Synthesize findings
report = synthesize_analysis(entry_points, architecture, db_code, error_handling)
```

### Pattern 3: Security Audit Workflow

```python
async def security_audit(project_path: str) -> dict:
    """Comprehensive AI-driven security audit."""

    findings = {}

    # 1. Authentication
    findings['auth'] = await analyze_security(
        search_code_semantic("authentication login password", limit=20),
        check_for=["weak_hashing", "hardcoded_secrets", "no_rate_limit"]
    )

    # 2. SQL Injection
    findings['sql_injection'] = await analyze_security(
        search_code_semantic("sql query user input", limit=30),
        check_for=["string_concatenation", "no_parameterization"]
    )

    # 3. XSS
    findings['xss'] = await analyze_security(
        search_code_semantic("html render user content", limit=25),
        check_for=["no_escaping", "dangerouslySetInnerHTML"]
    )

    # 4. Hardcoded secrets
    findings['secrets'] = await analyze_security(
        search_code_semantic("secret password api key hardcoded", limit=15),
        check_for=["credentials_in_code"]
    )

    return findings
```

### Pattern 4: Code Understanding Assistant

```python
def understand_codebase(question: str, context: str = "") -> str:
    """Answer questions about codebase using ck."""

    # Parse question to determine search strategy
    if is_asking_about_specific_function(question):
        # Use hybrid search for known names
        query = extract_function_name(question)
        results = search_code_hybrid(query, limit=10)

    elif is_asking_about_concept(question):
        # Use semantic search for concepts
        query = extract_concept(question)
        results = search_code_semantic(query, limit=15)

    else:
        # General exploration
        query = question
        results = search_code_semantic(query, limit=20)

    # Provide results to LLM for synthesis
    return synthesize_answer(question, results, context)
```

## Real-World Examples

### Example 1: Code Review Assistant

```python
from anthropic import Anthropic
import json
import subprocess

client = Anthropic(api_key="...")

def get_code_context(query: str, limit: int = 10) -> str:
    """Get relevant code context using ck."""
    cmd = ["ck", "--sem", query, ".", "--json", "--limit", str(limit), "--scores"]
    result = subprocess.run(cmd, capture_output=True, text=True)
    results = json.loads(result.stdout)

    context = []
    for r in results:
        context.append(f"# {r['file']}:{r['line']} (score: {r['score']:.2f})\n{r['content']}")

    return "\n\n".join(context)

def review_pull_request(pr_description: str, changed_files: list) -> str:
    """AI code review using ck for context."""

    # Get relevant context from codebase
    context_queries = [
        "error handling patterns",
        "authentication authorization",
        "database queries",
        "input validation"
    ]

    context = ""
    for query in context_queries:
        context += f"\n## {query}\n"
        context += get_code_context(query, limit=5)

    # Ask Claude to review
    message = client.messages.create(
        model="claude-3-5-sonnet-20241022",
        max_tokens=4096,
        messages=[{
            "role": "user",
            "content": f"""Review this pull request for security and quality issues.

PR Description:
{pr_description}

Changed Files:
{json.dumps(changed_files, indent=2)}

Codebase Context (from semantic search):
{context}

Provide:
1. Security concerns
2. Code quality issues
3. Suggestions for improvement
"""
        }]
    )

    return message.content[0].text
```

### Example 2: Documentation Generator

```python
def generate_documentation(component: str) -> str:
    """Generate documentation by understanding code."""

    # Find the component
    component_code = search_code_hybrid(component, limit=5)

    if not component_code:
        return f"Component '{component}' not found"

    # Find related code
    related_queries = [
        f"{component} usage examples",
        f"{component} configuration",
        f"{component} error handling",
        f"{component} tests"
    ]

    related_code = {}
    for query in related_queries:
        related_code[query] = search_code_semantic(query, limit=5)

    # Generate documentation using LLM
    return generate_docs_from_code(component_code, related_code)
```

### Example 3: Refactoring Assistant

```python
def find_refactoring_opportunities(pattern: str) -> dict:
    """Find code duplication and refactoring opportunities."""

    # Find all instances of pattern
    instances = search_code_semantic(pattern, threshold=0.65, limit=50)

    # Group by similarity
    groups = cluster_by_similarity(instances)

    # For each group, suggest refactoring
    suggestions = []
    for group in groups:
        if len(group) >= 3:  # Pattern repeated 3+ times
            suggestion = {
                "pattern": pattern,
                "instances": len(group),
                "locations": [f"{r['file']}:{r['line']}" for r in group],
                "recommendation": suggest_refactoring(group)
            }
            suggestions.append(suggestion)

    return {
        "pattern": pattern,
        "total_instances": len(instances),
        "refactoring_suggestions": suggestions
    }
```

## Handling ck Output

### Parsing JSON Output

````python
import json

def parse_ck_results(output: str) -> list:
    """Parse ck JSON output safely."""
    try:
        results = json.loads(output)
        return results
    except json.JSONDecodeError as e:
        # Handle empty results or errors
        return []

def format_for_llm(results: list, max_length: int = 4000) -> str:
    """Format results for LLM consumption."""
    formatted = []

    for r in results:
        entry = f"""
File: {r['file']}:{r['line']}
{f"Score: {r['score']:.3f}" if 'score' in r else ""}
```
{r.get('content', '')}
```
"""
        formatted.append(entry)

    full_text = "\n---\n".join(formatted)

    # Truncate if too long
    if len(full_text) > max_length:
        full_text = full_text[:max_length] + "\n\n[... truncated ...]"

    return full_text
````

### Handling Near-Miss Feedback

ck provides near-miss hints when no results are found:

```text
No matches found above threshold 0.7
Near-miss: ./auth.rs:42 (score: 0.68)
Hint: Try lowering threshold to 0.6
```

**AI agent pattern**:

```python
def adaptive_search(query: str, threshold: float = 0.6) -> list:
    """Search with adaptive threshold based on results."""
    cmd = ["ck", "--sem", query, ".", "--json", "--threshold", str(threshold), "--limit", "10"]
    result = subprocess.run(cmd, capture_output=True, text=True)

    # Check for near-miss hint in stderr
    if "Near-miss" in result.stderr and "Try lowering threshold to" in result.stderr:
        # Extract suggested threshold
        suggested = extract_threshold_hint(result.stderr)
        # Retry with suggested threshold
        return adaptive_search(query, suggested)

    return json.loads(result.stdout) if result.stdout else []
```

## AI Agent Best Practices

### 1. Start with Semantic, Refine with Hybrid

```python
# 1. Semantic search to discover
results = search_code_semantic("authentication", limit=10)

# 2. Extract key identifiers
identifiers = extract_identifiers(results)  # e.g., "authenticateUser"

# 3. Hybrid search for exact usage
for ident in identifiers:
    usages = search_code_hybrid(ident, limit=30)
```

### 2. Use Scores to Prioritize

```python
results = search_code_semantic("security vulnerability", limit=30)

# Sort by score (already sorted by ck)
high_confidence = [r for r in results if r['score'] > 0.8]
medium_confidence = [r for r in results if 0.6 <= r['score'] <= 0.8]

# Review high confidence first
analyze(high_confidence)
```

### 3. Combine Multiple Searches

```python
# Comprehensive understanding
auth_results = search_code_semantic("authentication login", limit=15)
session_results = search_code_semantic("session management", limit=10)
token_results = search_code_semantic("token jwt verification", limit=10)

# Combine and deduplicate
all_results = deduplicate_by_file_line(
    auth_results + session_results + token_results
)
```

### 4. Index Once Per Session

```python
import os
from pathlib import Path

def ensure_indexed(project_path: str) -> bool:
    """Check if index exists, create if needed."""
    ck_dir = Path(project_path) / ".ck"

    if not ck_dir.exists():
        # Index for first time
        cmd = ["ck", "--index", project_path]
        result = subprocess.run(cmd, capture_output=True)
        return result.returncode == 0

    return True  # Already indexed

# At start of AI session
ensure_indexed("/path/to/project")
```

### 5. Provide Context to LLM

```python
def query_with_context(llm_query: str) -> str:
    """Provide search results as context to LLM."""

    # Extract search intent from LLM query
    search_query = extract_search_query(llm_query)

    # Get relevant code
    code_results = search_code_semantic(search_query, limit=15)

    # Format as context
    context = format_for_llm(code_results)

    # Provide to LLM
    prompt = f"""
Based on this code from the project:

{context}

Answer the following question:
{llm_query}
"""

    return call_llm(prompt)
```

## Performance Considerations

### Token Efficiency

```python
# Instead of sending all code
results = search_code_semantic("auth", limit=50)  # 50 results * 200 chars = 10,000 chars

# Send summary + top results
summary = summarize_results(results)
top_results = results[:10]

context = f"{summary}\n\nTop results:\n{format_for_llm(top_results)}"
# Much more token-efficient
```

### Caching Results

```python
from functools import lru_cache
import hashlib

@lru_cache(maxsize=128)
def cached_search(query: str, threshold: float, limit: int) -> str:
    """Cache search results to avoid repeated calls."""
    cmd = ["ck", "--sem", query, ".", "--json", "--threshold", str(threshold), "--limit", str(limit)]
    result = subprocess.run(cmd, capture_output=True, text=True)
    return result.stdout

# Use cached search
results1 = json.loads(cached_search("authentication", 0.6, 10))  # Executes ck
results2 = json.loads(cached_search("authentication", 0.6, 10))  # Returns cached
```

## Error Handling

```python
def robust_search(query: str, **kwargs) -> list:
    """Search with comprehensive error handling."""
    try:
        cmd = ["ck", "--sem", query, ".", "--json"]

        for key, value in kwargs.items():
            cmd.extend([f"--{key}", str(value)])

        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=30  # 30 second timeout
        )

        if result.returncode != 0:
            error_msg = result.stderr
            if "not indexed" in error_msg:
                # Try to index
                subprocess.run(["ck", "--index", "."], check=True)
                # Retry search
                return robust_search(query, **kwargs)
            else:
                raise RuntimeError(f"Search failed: {error_msg}")

        return json.loads(result.stdout) if result.stdout else []

    except subprocess.TimeoutExpired:
        return []  # Search took too long, return empty
    except json.JSONDecodeError:
        return []  # Invalid JSON, return empty
    except Exception as e:
        print(f"Search error: {e}")
        return []
```

## Next Steps

- **Learn MCP integration**: [MCP Integration](/features/mcp-integration)
- **Understand semantic search**: [Semantic Search](/features/semantic-search)
- **Explore use cases**: [Exploring New Codebases](/recipes/explore-codebase)
- **Security workflows**: [Security Code Review](/recipes/security-review)

## Related Documentation

- [AI Agent Setup Guide](/guide/ai-agent-setup) - Quick setup for AI usage
- [Output Formats](/reference/output-formats) - JSON schema reference
- [CLI Reference](/reference/cli) - All command-line options
- [MCP Integration](/features/mcp-integration) - Model Context Protocol setup
