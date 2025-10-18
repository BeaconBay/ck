---
title: Introduction to ck
description: Hybrid code search tool that finds code by meaning, not keywords. Drop-in grep replacement with AI-powered understanding for developers and AI agents.
---

# Introduction

**ck (seek)** finds code by meaning, not just keywords. It's grep that understands what you're looking for — search for "error handling" and find try/catch blocks, error returns, and exception handling code even when those exact words aren't present.

## What is ck?

ck is a hybrid code search tool that combines the familiarity of grep with the intelligence of modern AI embeddings. It's designed for:

- **AI agents** that need reliable code search capabilities via MCP protocol
- **Developers** who want to find code by what it does, not what it's named
- **Teams** exploring large codebases and understanding unfamiliar code
- **Code reviewers** finding related code and patterns across files

## Key Capabilities

### 🔍 Semantic Search
Find code by concept, not keywords:
```bash
ck --sem "error handling" src/
ck --sem "authentication logic" src/
ck --sem "database connection pooling" src/
```

### ⚡ grep Compatibility
All your muscle memory works:
```bash
ck -n "TODO" *.rs
ck -R -i "fixme" .
ck -l "error" src/
```

### 🎯 Hybrid Search
Best of both worlds:
```bash
ck --hybrid "connection timeout" src/
```

### 🤖 AI Integration
Built-in MCP server for Claude Desktop, Cursor, and other AI tools:
```bash
ck --serve
```

## How It Works

1. **Indexing** – ck automatically creates and maintains semantic indexes of your code
2. **Embedding** – Uses local AI models (BGE, Nomic, Jina) to understand code semantics
3. **Search** – Finds semantically similar code chunks using vector similarity
4. **Results** – Returns familiar grep-style output with optional relevance scores

## Why ck?

### vs. grep/ripgrep
- ✅ Understands code meaning, not just text patterns
- ✅ Finds related code even with different terminology
- ✅ Maintains full grep compatibility for keyword search
- ✅ Automatic smart file filtering (.ckignore)

### vs. IDE search
- ✅ Works across entire codebase, not just open files
- ✅ Command-line friendly for scripts and automation
- ✅ Semantic understanding beyond symbol search
- ✅ AI agent integration via MCP

### vs. AI code search services
- ✅ 100% offline — no code leaves your machine
- ✅ No API keys or subscriptions required
- ✅ Fast local inference
- ✅ Privacy-first design

## Design Philosophy

- **Drop-in compatibility** – Works like grep, enhances where needed
- **Automatic everything** – Index management, updates, model downloads
- **Privacy-first** – Everything runs locally, no telemetry
- **Performance matters** – Fast indexing, sub-second queries
- **AI-native** – Built for both humans and AI agents

## Next Steps

- [Get started](/guide/installation) with installation
- Learn [basic usage](/guide/basic-usage) patterns
- Explore [MCP integration](/features/mcp-integration) for AI agents
- Read the [CLI reference](/reference/cli) for all options
