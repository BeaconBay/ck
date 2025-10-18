---
title: Choosing an Interface
description: Compare ck's four interfaces—CLI, TUI, Editor, and MCP—to find the best fit for your workflow. Detailed comparison of features, use cases, and strengths.
---

# Choosing an Interface

ck offers four different interfaces for code search. Choose the one that best fits your workflow and use case.

## Quick Decision Matrix

| Interface | Best For | Complexity | Integration |
|-----------|----------|------------|-------------|
| [CLI](#command-line-interface-cli) | Scripts, pipelines, grep replacement | Simple | Any terminal |
| [TUI](#terminal-user-interface-tui) | Interactive exploration, discovery | Medium | Terminal |
| [Editor](#editor-integration) | In-editor search, zero context switch | Medium | VSCode/Cursor |
| [MCP](#mcp-server) | AI agent integration, automation | Advanced | Claude, etc. |

## Command-Line Interface (CLI)

**Best for**: Scripting, pipelines, grep replacement, automation

### When to Use

✅ **Perfect when you**:
- Already use grep/ripgrep in scripts
- Need structured output (JSON/JSONL)
- Want to pipe results to other commands
- Prefer composable Unix-style tools
- Need reproducible searches
- Run batch operations

❌ **Not ideal when you**:
- Want to explore results interactively
- Need live preview of matches
- Prefer visual navigation
- Want to try multiple queries quickly

### Example Workflows

#### Simple Search
```bash
ck --sem "error handling" src/
```

#### Pipeline Integration
```bash
ck --sem "TODO" . | grep -v test | wc -l
```

#### Structured Output for Tools
```bash
ck --jsonl --sem "deprecated" src/ | jq '.file' | sort | uniq
```

#### Scripted Code Review
```bash
#!/bin/bash
# Find all authentication code for security review
ck --sem "authentication" src/ -l > auth_files.txt
ck --sem "password handling" src/ -l >> auth_files.txt
sort -u auth_files.txt
```

### Advantages

- **Fast**: No UI overhead, direct output
- **Scriptable**: Deterministic, reproducible results
- **Pipeable**: Works with Unix tools (`grep`, `awk`, `jq`)
- **Flexible**: All output formats available
- **Universal**: Works in any terminal, SSH, CI/CD

### Limitations

- No interactive preview
- One search at a time
- Results printed then gone
- Manual navigation to files

### Learn More

- [CLI Reference](/reference/cli) — Complete flag documentation
- [Basic Usage](/guide/basic-usage) — Common patterns
- [Advanced Usage](/guide/advanced-usage) — Power user techniques

## Terminal User Interface (TUI)

**Best for**: Interactive exploration, code discovery, query refinement

### When to Use

✅ **Perfect when you**:
- Want to explore search results visually
- Need live preview of matches
- Want to refine queries interactively
- Prefer keyboard-driven workflow
- Work primarily in terminal
- Need to see context for each result

❌ **Not ideal when you**:
- Need to automate searches
- Want to pipe output
- Prefer in-editor integration
- Need structured output

### Example Workflows

#### Interactive Discovery
```bash
# Launch TUI
ck-tui

# In TUI:
1. Type query: "database connection"
2. See results update live
3. Navigate with ↑/↓
4. Preview with →
5. Open in editor with Enter
```

#### Query Refinement
```bash
ck-tui
# Start broad: "auth"
# Refine: "authentication flow"
# Switch to hybrid: Ctrl+H
# Adjust threshold: increase for precision
```

#### Code Exploration Session
```bash
# Exploring new codebase
ck-tui
# Search: "main entry point"
# Search: "configuration"
# Search: "error handling"
# Toggle modes to find patterns
```

### Advantages

- **Interactive**: Live results as you type
- **Visual**: See context, scores, previews
- **Fast iteration**: Refine queries instantly
- **Keyboard-driven**: Efficient navigation
- **Multi-mode**: Switch between semantic/regex/hybrid
- **Preview modes**: Chunks, heatmap, full-file

### Limitations

- No scripting support
- Not pipeable
- Requires terminal
- One workspace at a time

### Learn More

- [TUI Mode](/features/tui-mode) — Complete interface guide
- [Keyboard Shortcuts](/features/tui-mode#keyboard-shortcuts) — Navigation reference

## Editor Integration

**Best for**: In-editor search, zero context switching, development workflow

### When to Use

✅ **Perfect when you**:
- Want search without leaving editor
- Prefer clicking to navigate results
- Use VSCode or Cursor primarily
- Want visual score indicators
- Need instant code preview
- Prefer mouse + keyboard workflow

❌ **Not ideal when you**:
- Use different editor (IntelliJ, Vim, Emacs)
- Prefer terminal-only workflow
- Need to automate searches
- Want to use in CI/CD

### Example Workflows

#### Quick Code Navigation
```bash
1. Press Cmd+Shift+; (search command)
2. Type: "handle user input"
3. See live results with scores
4. Click result → jump to line
5. Code opens with highlight
```

#### Search Selection
```bash
1. Select code: validateEmail
2. Press Cmd+Shift+' (search selection)
3. See all similar implementations
4. Review different approaches
5. Click to navigate
```

#### Understanding Unfamiliar Codebase
```bash
1. Open search panel
2. Search: "authentication"
   → See all auth-related code
3. Search: "database queries"
   → Understand data layer
4. Search: "API endpoints"
   → Map out interfaces
```

### Advantages

- **Zero context switch**: Search while coding
- **Visual results**: Scores, bars, previews
- **Instant navigation**: Click to jump
- **Integrated workflow**: Native VS Code feel
- **Live updates**: Results as you type
- **Status indicators**: Index freshness at a glance

### Limitations

- Requires specific editor
- Not scriptable
- Single workspace per window
- VSCode/Cursor only (JetBrains planned)

### Learn More

- [Editor Integration](/features/editor-integration) — Complete extension guide
- [Configuration](/features/editor-integration#configuration) — Settings reference

## MCP Server

**Best for**: AI agent integration, Claude Desktop, automation tools

::: tip AI Agent Configuration
See [AI Agent Setup](/guide/ai-agent-setup) for detailed configuration guidance for Claude Code and other AI coding assistants.
:::

### When to Use

✅ **Perfect when you**:
- Use Claude Desktop or other AI tools
- Want AI-assisted code exploration
- Need programmatic search access
- Build automation on top of ck
- Want persistent search sessions
- Need pagination for large results

❌ **Not ideal when you**:
- Don’t use AI assistants
- Prefer direct CLI interaction
- Want simple grep replacement
- Need human-facing output

### Example Workflows

#### AI-Assisted Code Review
```bash
In Claude Desktop:
> "Find all error handling code and summarize the patterns"

Claude uses MCP to:
1. Search semantic: "error handling"
2. Retrieve results via MCP
3. Analyze code snippets
4. Provide summary
```

#### Automated Code Analysis
```python
# Using MCP client
from mcp_client import MCPClient

client = MCPClient("ck")
results = client.search(
    query="deprecated APIs",
    mode="semantic",
    page_size=50
)

for result in results:
    analyze_deprecation(result)
```

#### Interactive Code Exploration
```bash
In Claude:
> "Show me how authentication works in this codebase"

Claude:
1. MCP search: "authentication"
2. MCP search: "login flow"
3. MCP search: "JWT handling"
4. Synthesizes explanation with code references
```

### Advantages

- **AI-enhanced**: Leverage language models for code understanding
- **Programmatic**: JSON-RPC API for automation
- **Persistent**: Reuse connection for multiple searches
- **Paginated**: Handle large result sets efficiently
- **Structured**: Clean data format for processing
- **Extensible**: Build custom tools on top

### Limitations

- Requires MCP-compatible client
- More complex setup
- Not for direct human interaction
- Learning curve for API

### Learn More

- [MCP Integration](/features/mcp-integration) — Complete MCP guide
- [MCP Tools](/features/mcp-integration#mcp-tools) — API reference

## Combining Interfaces

You can use multiple interfaces for different scenarios:

### Developer Workflow

```bash
# Morning: Explore codebase with TUI
ck-tui
# Search: "new features from last sprint"

# During coding: Use editor extension
# Cmd+Shift+; → search as you code

# Scripting: Use CLI for automation
ck --sem "TODO" . -l > todos.txt

# AI assistance: Use MCP via Claude Desktop
# "Explain the architecture of this module"
```

### Team Collaboration

```bash
# Individual exploration: TUI
ck-tui  # Interactive discovery

# Code review scripts: CLI
ck --sem "security vulnerabilities" src/ --json > review.json

# PR descriptions: AI + MCP
# Claude generates summaries via MCP

# IDE integration: Editor extension
# Quick searches during development
```

## Migration Path

### From grep/ripgrep

**Start with**: CLI mode

```bash
# Familiar grep syntax
ck "pattern" src/

# Add semantic search when needed
ck --sem "concept" src/

# Graduate to TUI for exploration
ck-tui
```

### New to ck

**Start with**: TUI mode

```bash
# Launch interactive interface
ck-tui

# Experiment with queries
# Learn what works

# Add CLI for scripts
ck --sem "pattern" . -l > files.txt

# Install editor extension when comfortable
# code --install-extension ck-search
```

### AI-First Users

**Start with**: MCP + Editor

```bash
# Install editor extension
code --install-extension ck-search

# Configure for AI agents (see AI Agent Setup guide)
# Enable MCP in Claude Desktop
# Use AI to explore codebase

# Add CLI for custom scripts
ck --jsonl --sem "pattern" . | custom-tool
```

See [AI Agent Setup](/guide/ai-agent-setup) for complete configuration details.

## Feature Comparison

| Feature | CLI | TUI | Editor | MCP |
|---------|-----|-----|--------|-----|
| **Search Modes** |
| Semantic search | ✅ | ✅ | ✅ | ✅ |
| Hybrid search | ✅ | ✅ | ✅ | ✅ |
| Regex search | ✅ | ✅ | ✅ | ✅ |
| **Output Formats** |
| Human-readable | ✅ | ✅ | ✅ | — |
| JSON | ✅ | — | — | ✅ |
| JSONL | ✅ | — | — | — |
| **Interaction** |
| Interactive | — | ✅ | ✅ | — |
| Scriptable | ✅ | — | — | ✅ |
| Live preview | — | ✅ | ✅ | — |
| **Navigation** |
| Direct file opening | — | ✅ | ✅ | — |
| Copy file paths | ✅ | ✅ | — | — |
| Click navigation | — | — | ✅ | — |
| **Advanced** |
| Pagination | — | ✅ | ✅ | ✅ |
| Threshold tuning | ✅ | ✅ | ✅ | ✅ |
| Context control | ✅ | ✅ | ⚠️ | ✅ |
| Reranking | ✅ | ✅ | ✅ | ✅ |

✅ = Fully supported
⚠️ = Partial support
— = Not applicable

## Performance Characteristics

| Interface | Startup | Search | Results | Memory |
|-----------|---------|--------|---------|--------|
| CLI | Instant | Fast | Immediate | Low |
| TUI | <1s | Fast | Streaming | Medium |
| Editor | ~2s | Fast | Buffered | Medium |
| MCP | ~3s | Fast | Paginated | Low-Medium |

## Recommendations by Role

### Backend Developer
1. **Primary**: CLI for scripts and pipelines
2. **Secondary**: TUI for exploration
3. **Optional**: MCP for AI-assisted code review

### Frontend Developer
1. **Primary**: Editor extension (VSCode/Cursor)
2. **Secondary**: TUI for terminal work
3. **Optional**: CLI for build scripts

### DevOps Engineer
1. **Primary**: CLI for automation
2. **Secondary**: TUI for debugging
3. **Optional**: MCP for AI-assisted troubleshooting

### Data Scientist
1. **Primary**: CLI for notebook integration
2. **Secondary**: MCP for AI workflows
3. **Optional**: Editor extension

### Security Researcher
1. **Primary**: CLI for batch analysis
2. **Secondary**: TUI for interactive exploration
3. **Essential**: MCP for AI-assisted vulnerability detection

## See Also

- [Getting Started](/guide/installation) — Installation and first steps
- [Basic Usage](/guide/basic-usage) — Common patterns
- [CLI Reference](/reference/cli) — Complete command reference
- [TUI Mode](/features/tui-mode) — Interactive terminal interface
- [Editor Integration](/features/editor-integration) — VSCode/Cursor extension
- [MCP Integration](/features/mcp-integration) — AI agent integration
