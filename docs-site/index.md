---
layout: home

hero:
  name: ck <span style="font-weight:300;">("seek")</span>
  text: Hybrid Code Search
  tagline: ck’s hybrid search fuses lexical (BM25/grep) precision with embedding-based recall and re-ranks both, so you find the right code even when the exact keywords aren’t there.
  image:
    src: /logo.png
    alt: ck logo
  actions:
    - theme: brand
      text: Get Started
      link: /guide/installation
    - theme: alt
      text: View on GitHub
      link: https://github.com/BeaconBay/ck

features:
  - icon: ⚡
    title: Drop-in <code>grep</code> Replacement
    details: All your muscle memory works. Same flags, same behavior, same output format — plus semantic understanding when you need it
    link: /features/grep-compatibility

  - icon: 🔍
    title: Semantic Search
    details: Find code by concept, not keywords. Search for “retry logic” and find backoff, circuit breakers, and related patterns even without exact matches
    link: /features/semantic-search

  - icon: 🎯
    title: Hybrid Search
    details: Combine keyword precision with semantic understanding using Reciprocal Rank Fusion for best-of-both-worlds search results
    link: /features/hybrid-search

  - icon: 🤖
    title: AI Agent Integration
    details: Built-in MCP (Model Context Protocol) server for seamless integration with Claude Desktop, Cursor, and any MCP-compatible AI client
    link: /features/mcp-integration

  - icon: 💻
    title: Terminal User Interface
    details: Interactive search with live results, visual score heatmaps, and keyboard-driven navigation. Explore code with TUI mode for instant feedback
    link: /features/tui-mode

  - icon: 🔌
    title: Editor Integration
    details: Native VSCode and Cursor extension. Search without leaving your editor with inline results, instant navigation, and live updates
    link: /features/editor-integration

  - icon: 🚀
    title: Blazing Fast
    details: ~1M LOC indexed in under 2 minutes. Sub-500ms queries. Chunk-level incremental indexing only re-embeds what changed
    link: /guide/basic-usage

  - icon: 📦
    title: Completely Offline
    details: Everything runs locally. No code or queries sent to external services. Embedding model downloaded once and cached locally
    link: /reference/models
---

## Quick Start

```bash
# Install from NPM
npm install -g @beaconbay/ck-search

# CLI: Command-line search (grep-compatible)
ck --sem "error handling" src/
ck --hybrid "connection timeout" src/
ck -n "TODO" *.rs

# TUI: Interactive terminal UI
ck-tui
# Type queries, see live results, navigate with ↑/↓

# Editor: VSCode/Cursor extension
code --install-extension ck-search
# Press Cmd+Shift+; to search

# MCP: AI agent integration
ck --serve
# Configure in Claude Desktop for AI-assisted search
```

## Why ck?

**ck (seek)** finds code by meaning, not just keywords. It’s the grep you wish you had:

- 🎯 **Understand intent** – Search for “error handling” and find try/catch blocks, error returns, and exception handling even when those exact words aren’t present
- 🤖 **AI-first** – Built-in MCP server for direct integration with AI coding assistants
- ⚡ **Fast & efficient** – Automatic incremental indexing, sub-second queries
- 🔧 **Drop-in replacement** – Works exactly like grep/ripgrep with all the flags you know
- 🌐 **Multi-language** – Python, JavaScript/TypeScript, Rust, Go, Ruby, Haskell, C#, and more
- 🔒 **Privacy-first** – 100% offline, no telemetry, no external API calls

## Installation

### From NPM (recommended)
```bash
npm install -g @beaconbay/ck-search
```

### From crates.io
```bash
cargo install ck-search
```

### From source
```bash
git clone https://github.com/BeaconBay/ck
cd ck
cargo install --path ck-cli
```

## Next Steps

<div class="vp-doc">

- [**Getting Started Guide**](/guide/installation) — Installation and first search
- [**Choosing an Interface**](/guide/choosing-interface) — CLI, TUI, Editor, or MCP?
- [**TUI Mode**](/features/tui-mode) — Interactive terminal interface
- [**Editor Integration**](/features/editor-integration) — VSCode/Cursor extension
- [**MCP Integration**](/features/mcp-integration) — Connect with AI agents
- [**Basic Usage**](/guide/basic-usage) — Common patterns and workflows
- [**CLI Reference**](/reference/cli) — Complete command-line reference

</div>
