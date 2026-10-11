# ck - Semantic Code Search for VS Code & Cursor

Brings the power of `ck` semantic code search directly into Visual Studio Code and Cursor.

## Features

### Search Capabilities
- **Semantic Search**: Find code by meaning, not just keywords
- **Hybrid Search** (Default): Combine semantic understanding with keyword precision
- **Regex Search**: Traditional pattern matching when you need it
- **Automatic Reranking**: Cross-encoder reranking enabled by default for best relevance (⚡ RERANK badge)
- **Smart Context**: 2 lines of context before/after matches for better understanding

### UI & UX
- **Crisp, Clean Interface**: Terminal-inspired monospace design
- **Live Results**: Search as you type with 300ms debouncing (TUI-style)
- **Visual Score Indicators**: Color-coded relevance scores (cyan/blue/yellow/orange) with visual bars
- **Line-by-Line Previews**: Context-aware preview with accurate line numbering
- **Relative Paths**: Clean file paths relative to workspace root
- **Keyboard Navigation**: ↑/↓ to navigate, Enter to open, Esc to reset
- **Match Highlighting**: Visual distinction for matching lines in previews

### Integration
- **Direct File Navigation**: Click any result to jump to exact location with brief highlight
- **Real-time Index Status**: Green dot when indexed, yellow when needs indexing
- **One-Click Reindexing**: Rebuild workspace index with progress notifications
- **Editor Integration**: "Search Selection" command for quick searches

## Requirements

- [ck](https://github.com/BeaconBay/ck) must be installed and available in your PATH
- Install with: `cargo install ck-search`

## Usage

### Commands

- `ck: Search` (`Ctrl+Shift+;` / `Cmd+Shift+;`) - Open search panel
- `ck: Search Selection` (`Ctrl+Shift+'` / `Cmd+Shift+'`) - Search selected text
- `ck: Reindex` - Force rebuild of search index

### Search Modes

- **Hybrid** - Combines semantic and keyword search (default)
- **Semantic** - Find code by concept and meaning
- **Regex** - Traditional grep-style pattern matching

### Keyboard Navigation

- `↑/↓` - Navigate results
- `Enter` - Open selected result or trigger search
- `Esc` - Return focus to search input

## Extension Settings

- `ck.mode` - Integration mode: `cli` (default) or `mcp`
- `ck.cliPath` - Path to ck binary (default: `ck`)
- `ck.defaultMode` - Default search mode: `hybrid`, `semantic`, or `regex`
- `ck.topK` - Maximum number of results (default: 100)
- `ck.threshold` - Minimum relevance threshold (default: 0.02)
- `ck.pageSize` - Results per page (default: 50)

## Installation

### For Cursor

```bash
cd ck-vscode
./install-cursor.sh
```

Then restart Cursor.

### For VS Code

```bash
cd ck-vscode
npm install
npm run compile
code --install-extension . --force
```

## Development

Use Node.js 24, matching CI. Install the committed dependency versions with
`npm ci --ignore-scripts`.

The extension supports VS Code 1.93 and later. Keep `@types/vscode` on the
1.93 API line so compilation and packaging check that compatibility; upgrading
the types requires reviewing the minimum supported editor version together.

### Building

```bash
cd ck-vscode
npm ci --ignore-scripts
npm run compile
```

### Testing

1. Open in VS Code
2. Press F5 to launch Extension Development Host
3. Test search functionality

### Packaging

```bash
npm run package
```

This creates a `.vsix` file you can install locally or publish to the marketplace.

## Roadmap

- [x] Phase 1: CLI mode + sidebar UI
- [x] Automatic reranking for better relevance
- [x] Visual score indicators and crisp UI
- [x] Line numbers and match highlighting
- [x] Relative path display
- [ ] Phase 2: MCP server integration for persistent connections
- [ ] Phase 3: Full syntax highlighting in previews
- [ ] Phase 4: Streaming results for large codebases
- [ ] Phase 5: Multi-workspace support

## License

Same as ck - Apache 2.0 or MIT

## More Information

- [ck on GitHub](https://github.com/BeaconBay/ck)
- [Report Issues](https://github.com/BeaconBay/ck/issues)
