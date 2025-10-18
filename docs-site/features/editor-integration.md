---
title: Editor Integration
description: Native VSCode and Cursor extensions for ck semantic search. Search code without leaving your editor with inline results and live updates.
---

# Editor Integration

Bring ck's semantic code search directly into your code editor with native extensions.

## Overview

The ck VSCode/Cursor extension provides seamless integration of semantic search capabilities within your editor, eliminating context switching and enabling instant code exploration while you work.

### Key Benefits

- **Instant Access**: Search without leaving your editor
- **Visual Results**: Clean, TUI-inspired interface with color-coded relevance scores
- **Smart Navigation**: Click to jump directly to code locations
- **Live Updates**: Results update as you type with intelligent debouncing
- **Context Aware**: Preview matches with surrounding code for better understanding

## VSCode & Cursor Extension

### Installation

#### For Cursor

The simplest installation method for Cursor:

```bash
cd ck-vscode
./install-cursor.sh
```

Then restart Cursor to activate the extension.

#### For VS Code

Manual installation for VS Code:

```bash
cd ck-vscode
npm install
npm run compile
code --install-extension . --force
```

Restart VS Code to complete installation.

#### Requirements

- **ck binary**: Must be installed and available in your PATH
- Install with: `cargo install ck-search`
- Verify: `ck --version`

## Features

### Search Capabilities

#### Hybrid Search (Default)
Combines semantic understanding with keyword precision for optimal results:

```bash
Search: authentication flow
```

Returns both semantically related code (auth handling, login logic) and exact keyword matches, ranked by relevance.

::: tip Default Mode
Hybrid search is enabled by default with automatic reranking for best relevance (⚡ RERANK badge shown when active).
:::

#### Semantic Search
Find code by meaning and concept, not just keywords:

```bash
Search mode: Semantic
Query: error handling patterns
```

Returns all error handling approaches across your codebase, even if they use different terminology.

#### Regex Search
Traditional pattern matching when you need precise control:

```bash
Search mode: Regex
Query: function\s+handle\w+Error
```

Works like grep with full regular expression support.

### User Interface

#### Search Panel Layout

```bash
┌─────────────────────────────────────────────────────┐
│ ck Search                                    [mode] │
├─────────────────────────────────────────────────────┤
│ [Search input field]                                │
│                                                     │
│ Results (23) ⚡ RERANK                             │
│ ────────────────────────────────────────────────── │
│ ■■■■■■■■■■ 0.87  src/auth/handler.ts:45           │
│   authenticate user credentials                     │
│ ■■■■■■■■░░ 0.73  src/middleware/auth.ts:12         │
│   verify JWT token                                  │
│ ■■■■■■░░░░ 0.65  src/utils/errors.ts:89            │
│   handle authentication errors                      │
└─────────────────────────────────────────────────────┘
```

#### Visual Elements

- **Score Bars**: Visual representation of relevance (cyan/blue/yellow/orange)
- **Score Values**: Numerical relevance scores (0.0 - 1.0)
- **Relative Paths**: Clean file paths relative to workspace root
- **Line Numbers**: Precise location information
- **Context Preview**: 2 lines before/after matches for understanding

#### Real-Time Features

- **Live Search**: Results update as you type (300ms debounce)
- **Instant Preview**: Hover to see more context
- **Quick Navigation**: Click any result to jump to exact location
- **Brief Highlights**: Temporary highlight when opening files
- **Status Indicator**:
  - 🟢 Green dot = Index up to date
  - 🟡 Yellow dot = Needs reindexing

## Commands

Access ck functionality through VS Code’s command palette or keyboard shortcuts:

| Command | Shortcut (Windows/Linux) | Shortcut (macOS) | Description |
|---------|-------------------------|------------------|-------------|
| `ck: Search` | `Ctrl+Shift+;` | `Cmd+Shift+;` | Open search panel |
| `ck: Search Selection` | `Ctrl+Shift+'` | `Cmd+Shift+'` | Search selected text |
| `ck: Reindex` | — | — | Rebuild search index |

### Usage Examples

#### Quick Search Workflow

1. Press `Ctrl+Shift+;` (or `Cmd+Shift+;`) to open search panel
2. Type your query: `database connection pooling`
3. Results appear instantly with relevance scores
4. Press `↑`/`↓` to navigate results
5. Press `Enter` to open selected file at exact line

#### Search Selection Workflow

1. Select code in editor: `handleUserAuth`
2. Press `Ctrl+Shift+'` (or `Cmd+Shift+'`)
3. Extension searches for semantically similar code
4. Review results to find related implementations
5. Click result to navigate

## Keyboard Navigation

Efficient keyboard-driven workflow:

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate through results |
| `Enter` | Open selected result or trigger search |
| `Esc` | Return focus to search input |
| `Tab` | Cycle through UI elements |

## Configuration

Customize ck extension behavior through VS Code settings:

### Integration Mode

```json
{
  "ck.mode": "cli"  // Options: "cli" (default) or "mcp"
}
```

- **CLI mode**: Spawns ck binary for each search (current stable mode)
- **MCP mode**: Persistent connection via Model Context Protocol (experimental)

### Search Settings

```json
{
  "ck.defaultMode": "hybrid",    // Options: "hybrid", "semantic", "regex"
  "ck.topK": 100,                // Maximum number of results
  "ck.threshold": 0.02,          // Minimum relevance threshold
  "ck.pageSize": 50              // Results per page
}
```

::: warning Hybrid Threshold Scale
Hybrid search uses RRF scoring (~0.01-0.05 range). See [Hybrid Search thresholds](/features/hybrid-search#understanding-hybrid-thresholds) for details.
:::

### Binary Path

Specify custom ck binary location:

```json
{
  "ck.cliPath": "/usr/local/bin/ck"  // Default: "ck" (from PATH)
}
```

Useful when:
- ck is not in PATH
- Using development builds
- Testing multiple ck versions

## Index Management

### Automatic Index Updates

The extension monitors index status and displays indicators:

- **Green dot**: Index is current, ready to search
- **Yellow dot**: Files have changed, reindexing recommended

### Manual Reindexing

Rebuild the search index when needed:

1. Open Command Palette (`Ctrl+Shift+P` / `Cmd+Shift+P`)
2. Run: `ck: Reindex`
3. Progress notification appears
4. Status updates to green when complete

::: tip When to Reindex
- After pulling major code changes
- After adding new files to gitignore
- When search results seem outdated
- After switching branches with significant changes
:::

## Integration Modes

### CLI Mode (Current)

**Status**: ✅ Stable (Phase 1)

Spawns ck binary for each search operation:

**Advantages**:
- Simple, reliable architecture
- No persistent connections to manage
- Each search is independent
- Works with any ck version

**Process Flow**:
```bash
User types → Debounce → Spawn ck → Parse JSONL → Display results
```

### MCP Mode (Experimental)

**Status**: 🚧 Phase 2

Persistent connection via Model Context Protocol:

**Advantages**:
- Faster repeat searches (no spawn overhead)
- Streaming results for large codebases
- Reduced CPU/memory churn
- Advanced features (incremental updates, watching)

**Coming Soon**: Enable with `"ck.mode": "mcp"` in settings.

## Development

### Building from Source

Clone and build the extension:

```bash
# Clone ck repository
git clone https://github.com/BeaconBay/ck.git
cd ck/ck-vscode

# Install dependencies
npm install

# Compile TypeScript
npm run compile

# Install extension
code --install-extension . --force
```

### Development Workflow

1. **Open Extension Project**:
   ```bash
   code ck-vscode/
   ```

2. **Start Watch Mode**:
   ```bash
   npm run watch  # Auto-compile on changes
   ```

3. **Launch Extension Development Host**:
   - Press `F5` in VS Code
   - New window opens with extension loaded

4. **Test Changes**:
   - Make code edits in `src/` or `webview/`
   - Reload extension: `Ctrl+R` / `Cmd+R` in development host
   - Test functionality

5. **Package Extension**:
   ```bash
   npm run package  # Creates .vsix file
   ```

### Architecture Overview

```bash
ck-vscode/
├── src/
│   ├── extension.ts       # Entry point, command registration
│   ├── searchPanel.ts     # Webview provider, UI management
│   ├── cliAdapter.ts      # Binary spawning, result parsing
│   ├── mcpAdapter.ts      # MCP integration (experimental)
│   └── types.ts           # TypeScript interfaces
├── webview/
│   ├── main.js            # UI logic, event handling
│   └── styles.css         # TUI-inspired styling
├── resources/
│   └── icon.png           # Extension icon
└── package.json           # Extension manifest
```

## Roadmap

### Completed ✅

- [x] **Phase 1**: CLI mode with sidebar UI
- [x] **Automatic reranking** for better relevance
- [x] **Visual score indicators** with color coding
- [x] **Line numbers** and match highlighting
- [x] **Relative path display** for clean results
- [x] **Keyboard navigation** for efficient workflow
- [x] **Search selection** command for quick searches

### In Progress 🚧

- [ ] **Phase 2**: MCP server integration for persistent connections
- [ ] **Streaming results** for large codebases
- [ ] **Progress indicators** for long-running searches

### Planned 📋

- [ ] **Phase 3**: Full syntax highlighting in previews
- [ ] **Phase 4**: Peek view for inline results
- [ ] **Phase 5**: Multi-workspace support
- [ ] **Result filtering** UI controls
- [ ] **Search history** and favorites
- [ ] **Custom theme** support

## Troubleshooting

### Extension Doesn’t Activate

**Symptoms**: Extension icon not visible, commands not available

**Solutions**:
1. Check Output panel: View → Output → "ck"
2. Look for errors in Developer Tools: Help → Toggle Developer Tools
3. Verify extension installed: Extensions → Search "ck"
4. Restart VS Code

### ck Binary Not Found

**Symptoms**: "ck command not found" error

**Solutions**:
1. Verify ck in PATH:
   ```bash
   which ck    # macOS/Linux
   where ck    # Windows
   ```

2. Install if missing:
   ```bash
   cargo install ck-search
   ```

3. Set absolute path in settings:
   ```json
   {
     "ck.cliPath": "/usr/local/bin/ck"
   }
   ```

### Search Returns No Results

**Symptoms**: Empty results despite known matches

**Solutions**:
1. Test ck in terminal:
   ```bash
   cd /path/to/workspace
   ck --sem "test query" .
   ```

2. Check index status (yellow dot = needs reindexing)

3. Manually reindex: Run `ck: Reindex` command

4. Verify workspace folder is open in VS Code

5. Check threshold setting (might be too high):
   ```json
   {
     "ck.threshold": 0.02  // Lower for more results
   }
   ```

### Results Seem Outdated

**Symptoms**: New code doesn’t appear, deleted code still shows

**Solutions**:
1. Run `ck: Reindex` command
2. Check `.ckignore` patterns aren’t excluding files
3. Verify files are committed/staged (respects `.gitignore`)

### Webview Not Loading

**Symptoms**: Blank search panel

**Solutions**:
1. Check CSP errors in Developer Tools console
2. Verify webview resources exist in `ck-vscode/webview/`
3. Reinstall extension:
   ```bash
   code --uninstall-extension beaconbay.ck-search
   code --install-extension . --force
   ```

### Performance Issues

**Symptoms**: Slow searches, high CPU usage

**Solutions**:
1. Reduce `topK` setting:
   ```json
   {
     "ck.topK": 50  // Default: 100
   }
   ```

2. Increase debounce (edit webview config if needed)

3. Use regex mode for simple patterns (faster than semantic)

4. Exclude large directories in `.ckignore`

## JetBrains Plugin (Planned)

Support for IntelliJ IDEA, PyCharm, and WebStorm is planned for future releases.

**Roadmap**:
- v0.7: Initial IntelliJ IDEA plugin
- Common features across all JetBrains IDEs
- Native UI integration with IDE theme support

Track progress: [GitHub Issue #](https://github.com/BeaconBay/ck/issues)

## See Also

- [TUI Mode](/features/tui-mode) — Interactive terminal interface
- [MCP Integration](/features/mcp-integration) — AI agent integration
- [CLI Reference](/reference/cli) — Complete command-line reference
- [Hybrid Search](/features/hybrid-search) — Understanding threshold scales
- [GitHub Repository](https://github.com/BeaconBay/ck) — Source code and issues
