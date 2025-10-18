---
title: TUI Mode
description: Interactive text user interface for ck with live search, code preview, keyboard navigation, and score heatmaps. Full reference for ck-tui.
---

# TUI (Interactive) Mode

The TUI (Text User Interface) provides a beautiful, interactive search experience with live results and code preview. This reference covers all features, keyboard shortcuts, and technical details of the TUI mode.

## Overview

The TUI mode transforms `ck` into a visual, interactive search tool that provides:
- **Live search results** as you type
- **Multiple preview modes** for understanding code context
- **Semantic, regex, and hybrid search** modes switchable with a keypress
- **Keyboard-driven navigation** for efficiency
- **Syntax highlighting** and code structure visualization

The TUI is designed for:
- **Code discovery**: Learning how a codebase works
- **Finding implementations**: Locating specific patterns or concepts
- **Comparing approaches**: Seeing different implementations side-by-side
- **Quick exploration**: Rapid iteration on search queries

## Launching the TUI

### Basic Launch

```bash
# Start interactive search in current directory
ck --tui .

# Start in a specific directory
ck --tui src/
```

### Launch with Initial Query

```bash
# Start with semantic search
ck --tui --sem "error handling" src/

# Start with regex pattern
ck --tui --regex "fn \w+_test" .

# Start with hybrid search
ck --tui --hybrid "timeout" .
```

### Launch Behavior

- First search in a directory creates an index (~1-2 seconds for medium repos)
- Subsequent searches are instant (uses cached index)
- TUI respects `.gitignore` and `.ckignore` files
- Defaults to semantic search mode

## Interface Layout

```bash
┌─────────────────────────────────────────────────────┐
│ Search: error handling              [Semantic] [●] │  ← Search box with mode indicator
├─────────────────────────────────────────────────────┤
│ Results (234)                                       │
│ ● src/lib.rs:45 (0.92)                            │  ← Results list with:
│   src/error.rs:12 (0.88)                          │    - Selection indicator
│   src/handler.rs:89 (0.85)                        │    - File path and line number
│   tests/error_test.rs:23 (0.82)                   │    - Relevance score (semantic)
│   docs/errors.md:5 (0.79)                         │
├─────────────────────────────────────────────────────┤
│ Preview: src/lib.rs:45-60              [Chunks]    │  ← Preview pane with:
│                                                     │    - File location
│ ┌─ function handle_error • 45 tokens ─┐           │    - Preview mode indicator
│ │                                      │           │    - Syntax highlighting
│ │  pub fn handle_error(e: Error) -> Result<()> {  │    - Code structure
│ │      match e {                                   │
│ │          Error::Io(err) => {...}                 │
│ │          Error::Parse(err) => {...}              │
│ │      }                                            │
│ │  }                                                │
│ └──────────────────────────────────────┘           │
└─────────────────────────────────────────────────────┘
```

### Interface Elements

**Top bar:**
- Query input field (editable when in search mode)
- Search mode indicator: `[Semantic]`, `[Regex]`, or `[Hybrid]`
- Active indicator: `[●]` shows search is active

**Results pane:**
- File paths relative to search directory
- Line numbers where matches occur
- Relevance scores (0.0-1.0 in semantic/hybrid modes)
- Selection indicator (●) shows current result
- Total result count

**Preview pane:**
- Current file and line range
- Preview mode indicator: `[Chunks]`, `[Heatmap]`, or `[Full File]`
- Syntax-highlighted code
- Chunk boundaries (in chunks mode)
- Relevance heat coloring (in heatmap mode)

## Keyboard Shortcuts

### Complete Reference Table

| Key | Context | Action | Description |
|-----|---------|--------|-------------|
| **Navigation** |
| `↑` / `k` | Results list | Move up | Select previous result |
| `↓` / `j` | Results list | Move down | Select next result |
| `Ctrl+u` | Results list | Page up | Jump up half a page of results |
| `Ctrl+d` | Results list | Page down | Jump down half a page of results |
| `g` | Results list | Jump to top | Select first result |
| `G` | Results list | Jump to bottom | Select last result |
| `↑` / `k` | Full-file preview | Scroll up | Scroll preview up one line |
| `↓` / `j` | Full-file preview | Scroll down | Scroll preview down one line |
| **Search Input** |
| `i` | Any | Enter search mode | Start editing query |
| `/` | Any | Enter search mode | Alternative to `i` |
| `Esc` | Search mode | Exit search mode | Stop editing, keep query |
| `Enter` | Search mode | Execute search | Run search with current query |
| `Ctrl+c` | Search mode | Clear query | Delete all text from query |
| **Search Modes** |
| `s` | Any | Semantic mode | Switch to semantic search |
| `r` | Any | Regex mode | Switch to regex search |
| `h` | Any | Hybrid mode | Switch to hybrid search |
| **Preview Controls** |
| `m` | Any | Cycle preview mode | Rotate: Chunks → Heatmap → Full File → Chunks |
| `f` | Any | Toggle full-file | Switch between full-file and chunk view |
| **Actions** |
| `Enter` | Results list | Open in editor | Open file at match line in `$EDITOR` |
| `y` | Results list | Copy path | Copy file path to system clipboard |
| `q` | Any | Quit | Exit TUI mode |
| `Esc` | Any (not editing) | Quit | Alternative quit when not editing |

### Keyboard Shortcut Tips

**Vi-style navigation:**
- All navigation uses `j`/`k` for down/up (like vim)
- `g` and `G` for top/bottom (like vim)
- Works in both results and full-file preview

**Dual-purpose Enter:**
- In search mode: Execute search
- In results mode: Open file in editor

**Dual-purpose Escape:**
- In search mode: Exit search mode
- In results mode: Quit TUI

## Search Modes

### Semantic Mode (`s`)

**What it does:**
Searches by meaning and concept, not exact text matches. Uses AI embeddings to understand code semantics.

**Example queries:**
```bash
"error handling"          → Finds: try/catch, Result<>, match arms, panic!
"database connection pool" → Finds: connection management code
"retry mechanism"          → Finds: backoff, retry loops, circuit breakers
"authentication logic"     → Finds: login, auth middleware, token validation
```

**When to use:**
- Finding concepts across different implementations
- Discovering similar patterns written differently
- Learning how something is done in the codebase
- Broad exploration of unfamiliar code
- Finding code that does X without knowing the exact function names

**Strengths:**
- Finds conceptually similar code even with different terminology
- Great for cross-language patterns
- Discovers unexpected implementations
- Excellent for learning and exploration

**Limitations:**
- Requires index (1-2 second initial cost)
- Less precise for exact syntax patterns
- May return semantically similar but functionally different code

**Result scoring:**
- 0.9-1.0: Extremely relevant, likely exactly what you want
- 0.8-0.9: Highly relevant, strong semantic match
- 0.7-0.8: Relevant, worth reviewing
- 0.6-0.7: Potentially relevant, may be tangential
- Below 0.6: Weak match, probably not what you want

### Regex Mode (`r`)

**What it does:**
Classic grep-style pattern matching with full regex support. Searches exact text patterns.

**Example queries:**
```bash
"fn \w+_test"              → Finds: fn test_parse, fn integration_test
"TODO|FIXME"               → Finds: TODO and FIXME comments
"impl .* for"              → Finds: trait implementations in Rust
"async fn.*Error"          → Finds: async functions returning errors
```

**When to use:**
- Finding exact patterns or syntax
- Searching for specific identifiers
- Looking for TODOs, FIXMEs, or other markers
- Performance-critical searches (no indexing)
- Very large codebases where indexing is slow

**Strengths:**
- No indexing required (instant startup)
- Precise pattern matching
- Full regex power (capture groups, lookahead, etc.)
- Familiar to grep/ripgrep users

**Limitations:**
- Doesn’t understand code semantics
- Won’t find semantically similar but textually different code
- Requires knowing exact patterns to search for

**Pattern syntax:**
- Uses Rust regex crate (similar to PCRE)
- Case-sensitive by default (use `(?i)` for case-insensitive)
- Supports: `.*`, `\w+`, `\d+`, `[a-z]`, `(group)`, etc.

### Hybrid Mode (`h`)

**What it does:**
Combines semantic ranking with keyword filtering. Results must contain your keyword but are ranked by semantic relevance.

::: warning RRF Scoring Scale
Hybrid search uses Reciprocal Rank Fusion (RRF) scoring, which produces values in the 0.01-0.05 range, not 0.0-1.0 like semantic search. For threshold filtering, use values like `0.02` instead of `0.6`. See [Hybrid Search](/features/hybrid-search#understanding-hybrid-thresholds) for details.
:::

**Example queries:**
```bash
"timeout"                  → Finds: Code with "timeout" keyword, ranked by relevance
"connect"                  → Finds: Code with "connect", prioritizes connection logic
"parse"                    → Finds: Code with "parse", ranks parsing functions higher
```

**When to use:**
- You know a keyword but want semantic ranking
- Filtering broad semantic searches to specific terms
- Balance between precision and semantic understanding
- Best of both worlds approach

**Strengths:**
- More precise than pure semantic (keyword filter)
- Better ranking than pure regex (semantic scores)
- Good for narrowing semantic results

**Limitations:**
- Still requires indexing
- Less flexible than full regex
- Keyword must appear exactly (no fuzzy matching)

**How it works:**
1. First pass: Regex filter for keyword
2. Second pass: Semantic ranking of filtered results
3. Results: Only files with keyword, sorted by relevance

## Preview Modes

### Chunks Mode

**What it shows:**
The matched code chunk with semantic boundaries, showing complete functions, classes, or logical blocks.

**Visual example:**
```rust
┌─ function handle_request • 45 tokens ─┐
│                                        │
│  async fn handle_request(req: Request) -> Result<Response> {
│      let result = match req.method {
│          Method::GET => handle_get(req).await?,
│          Method::POST => handle_post(req).await?,
│          Method::DELETE => handle_delete(req).await?,
│          _ => return Err(Error::MethodNotAllowed),
│      };
│      Ok(result)
│  }
└────────────────────────────────────────┘
```

**Features:**
- Shows chunk boundaries with tree-sitter precision
- Displays chunk type (function, class, method, struct, etc.)
- Shows token count estimates (useful for LLM context)
- Breadcrumbs for nested code (e.g., “impl MyStruct > fn new”)
- Syntax highlighting
- Automatic boundary detection

**Best for:**
- Understanding code structure
- Finding specific functions or methods
- Seeing complete logical units
- Getting ready-to-copy code blocks
- Understanding scope and boundaries

**Technical details:**
- Uses tree-sitter for precise syntax boundaries
- Respects language-specific structure (functions, classes, etc.)
- Falls back to heuristic chunking for unsupported languages
- Chunk size: Configurable, typically 100-500 tokens

### Heatmap Mode

**What it shows:**
Colors code lines by semantic relevance to your query, showing which specific lines are most relevant.

**Visual example:**
```bash
│ 🟢  pub fn process_timeout(duration: Duration) -> Result<()> {  (0.95)
│ 🟢      let elapsed = start.elapsed();                          (0.89)
│ 🟡      if elapsed > duration {                                 (0.72)
│ 🟡          log::warn!("Operation timed out");                  (0.68)
│ 🟠          return Err(Error::Timeout);                         (0.58)
│ ⚪      }                                                        (0.35)
│ ⚪      Ok(())                                                   (0.28)
│ ⚪  }                                                            (0.15)
```

**Color Scale:**
- 🟢 **Bright Green** (0.875-1.0): Extremely relevant, core match
- 🟢 **Green** (0.75-0.875): Highly relevant, strong match
- 🟡 **Yellow** (0.625-0.75): Moderately relevant, supporting code
- 🟠 **Orange** (0.5-0.625): Somewhat relevant, contextual
- ⚪ **Gray** (0-0.5): Low relevance, boilerplate or unrelated

**Features:**
- Line-by-line relevance scoring
- Visual gradient showing importance
- Exact relevance scores displayed
- Helps identify the most important lines in a file
- Great for skimming large results

**Best for:**
- Finding the most relevant lines within a file
- Understanding what specifically matched your query
- Comparing different implementations
- Identifying key lines in large files
- Skipping boilerplate to find core logic

**Technical details:**
- Each line gets individual embedding similarity score
- Scores are relative to your search query
- Color thresholds are fixed for consistency
- Works best with semantic and hybrid search

### Full File Mode

**What it shows:**
The complete file with syntax highlighting and scrolling capability.

**Features:**
- Full syntax highlighting
- Scrollable with `↑`/`↓` or `j`/`k`
- Automatically jumps to matched line
- Shows file path and current line range
- Great for seeing complete context
- Respects language-specific syntax

**Best for:**
- Understanding how matched code fits in the larger file
- Seeing imports, dependencies, and context
- Reading complete implementations
- Understanding file structure
- Finding related code nearby

**Controls in full-file mode:**
- `j` / `↓`: Scroll down one line
- `k` / `↑`: Scroll up one line
- `Ctrl+d`: Scroll down half page
- `Ctrl+u`: Scroll up half page
- `f`: Toggle back to chunks mode
- `m`: Cycle to chunks or heatmap mode

**Technical details:**
- Loads entire file into memory
- Syntax highlighting via tree-sitter
- May be slow for very large files (>10MB)
- Binary files are not displayed
- Initial view centers on matched line

## Common Workflows

### Finding a Specific Function

**Goal:** Locate and open a function for editing

**Steps:**
1. Launch TUI: `ck --tui src/`
2. Press `s` to ensure semantic mode
3. Type query: "parse configuration file"
4. Press `Enter` to search
5. Press `m` to cycle to chunks mode
6. Navigate results with `j`/`k`
7. Press `Enter` to open in editor

**Why this works:**
- Semantic search finds conceptually similar functions
- Chunks mode shows complete function boundaries
- Direct editor integration for quick editing

### Exploring Error Handling Patterns

**Goal:** Learn how errors are handled across the codebase

**Steps:**
1. Search: `ck --tui --sem "error handling" .`
2. Review initial results
3. Press `m` for heatmap mode
4. See which specific lines handle errors
5. Press `f` for full-file context
6. Navigate to different files with `j`/`k`
7. Press `y` to copy interesting file paths
8. Open multiple files in editor to compare

**Why this works:**
- Semantic search finds various error handling approaches
- Heatmap shows the most relevant error-handling lines
- Full-file mode provides complete implementation context

### Finding TODOs with Context

**Goal:** Find all TODO comments and understand surrounding code

**Steps:**
1. Launch TUI: `ck --tui .`
2. Press `r` for regex mode
3. Type pattern: `TODO|FIXME|XXX`
4. Press `Enter` to search
5. Press `m` for chunks mode
6. Review each TODO in its function context
7. Use `y` to copy paths for later work

**Why this works:**
- Regex mode precisely matches TODO markers
- Chunks mode shows the function/context around each TODO
- Easy to prioritize and track for later

## Configuration

### Environment Variables

**EDITOR**
```bash
# Set your preferred editor for the Enter key
export EDITOR=nvim           # Neovim
export EDITOR=vim            # Vim
export EDITOR=code           # VS Code (waits for file to close)
export EDITOR="code -r"      # VS Code (reuse window)
export EDITOR=emacs          # Emacs
export EDITOR=nano           # Nano
```

The TUI uses `$EDITOR` to determine which editor to launch when you press `Enter` on a result. The file will open at the specific line number of the match.

**Terminal Configuration**
```bash
# Ensure proper terminal type (usually automatic)
export TERM=xterm-256color   # 256 color support

# For tmux users
export TERM=screen-256color  # tmux 256 color support
```

### Color Scheme

The TUI adapts to your terminal’s color scheme:

**Recommended terminals:**
- **macOS**: iTerm2, Alacritty, WezTerm, Terminal.app
- **Linux**: Alacritty, kitty, GNOME Terminal, konsole
- **Windows**: Windows Terminal, Alacritty, WezTerm
- **Cross-platform**: Alacritty, WezTerm

**For best results:**
- Use a terminal with 24-bit true color support
- Dark mode terminals generally work best
- Modern terminal emulators (2020+) recommended
- Ensure `$TERM` is set correctly

### .ckignore Configuration

The TUI respects `.ckignore` files for excluding directories and files from search:

```bash
# Example .ckignore
node_modules/
target/
*.log
.git/
```

Place `.ckignore` in your project root or search directory. See [Configuration](/reference/configuration) for full details.

## Tips & Tricks

### Effective Querying

**Good semantic queries (specific, concept-based):**
```bash
✅ "authentication middleware"      # Specific pattern
✅ "database connection pool"       # Clear concept
✅ "retry mechanism with backoff"   # Detailed pattern
✅ "error propagation"              # Specific technique
✅ "lazy initialization"            # Known pattern
```

**Less effective queries (too vague):**
```bash
❌ "the code that handles stuff"    # Too vague
❌ "things"                          # Not specific
❌ "good code"                       # Subjective, meaningless
❌ "how to"                          # Too general
```

**Regex pattern tips:**
```bash
✅ "fn test_\w+"                    # Specific pattern
✅ "TODO|FIXME|XXX"                 # Multiple alternatives
✅ "impl .* for \w+"                # Trait implementations
✅ "async fn.*-> Result"            # Async functions returning results
```

### Performance Tips

**Index management:**
1. **Index once, search many**: First search creates index (~1-2 sec for medium repos)
2. **Reindex when needed**: Delete `.ck/` directory to rebuild index
3. **Exclude large dirs**: Use `.ckignore` for `node_modules`, `target`, etc.

**Search optimization:**
1. **Use regex for exact matches**: Faster than semantic for simple string searches
2. **Narrow your scope**: Search `src/` instead of `.` when possible
3. **Start specific**: Specific queries return fewer, better results

**Preview optimization:**
1. **Full-file mode**: Use sparingly on large files (can be slow to render)
2. **Chunks mode**: Fastest preview mode, good default
3. **Heatmap mode**: Moderate performance, great for skimming

### Workflow Optimization

**Set your editor properly:**
```bash
# Add to ~/.bashrc or ~/.zshrc
export EDITOR=nvim
```

**Quick iteration on queries:**
- Use `i` or `/` to edit search without leaving TUI
- Press `Enter` to re-run search
- Iterate rapidly on query refinement

**Choose the right preview mode:**
- **Chunks**: Default, great for understanding structure
- **Heatmap**: Best for finding most relevant lines
- **Full-file**: Use when you need complete context

**Keyboard efficiency:**
- Learn `j`/`k` navigation (faster than arrow keys)
- Use `g`/`G` for quick jumps to top/bottom
- `Ctrl+d`/`Ctrl+u` for page navigation
- Press `y` to copy paths as you browse

## Troubleshooting

### TUI not launching

**Symptoms:**
- TUI doesn’t appear
- Terminal shows garbled output
- Immediate crash or error

**Solutions:**

```bash
# Check terminal compatibility
echo $TERM
# Should output: xterm-256color, screen-256color, etc.

# Try explicit TERM setting
TERM=xterm-256color ck --tui .

# Verify terminal supports TUI
tput colors
# Should output: 256 or higher
```

**Common causes:**
- Very old terminal emulator
- SSH session without proper TERM forwarding
- Terminal doesn’t support required features

**Workarounds:**
- Use CLI mode instead: `ck --sem "query" .`
- Upgrade terminal emulator
- Use local terminal instead of SSH

### Slow scrolling in full-file mode

**Solutions:**
1. Switch to chunks or heatmap mode (press `m`)
2. Use regex mode for very large files
3. Narrow search scope to smaller directories
4. Exclude large files with `.ckignore`

**Why it happens:**
- Very large files (>10MB) are slow to render
- Syntax highlighting is CPU-intensive
- Full-file mode loads entire file into memory

### Search not finding anything

**Debugging steps:**

```bash
# 1. Check mode indicator (top-right)
#    Ensure you're in the right mode (semantic/regex/hybrid)

# 2. Verify index exists
ls .ck/
# Should show index files; if not, first search creates it

# 3. Try regex mode to confirm file exists
# Press 'r' then search for a known string

# 4. Check .gitignore/.ckignore
cat .gitignore .ckignore
# Look for patterns that might exclude your target files
```

**Common causes:**
- Wrong search mode (semantic vs regex)
- File excluded by `.gitignore` or `.ckignore`
- Binary file (TUI only searches text)
- Typo in search query
- File outside search directory

### Clipboard not working

**Platform-specific solutions:**

**macOS:**
```bash
# Usually works out of the box
which pbcopy
```

**Linux:**
```bash
# Install xclip or xsel
sudo apt install xclip
# Verify installation
which xclip
```

**Windows:**
```bash
# Usually works in Windows Terminal
where clip
```

## Next Steps

- Learn about [hybrid search thresholds](/features/hybrid-search#understanding-hybrid-thresholds)
- Explore [semantic search](/features/semantic-search) in depth
- Try [MCP integration](/features/mcp-integration) with AI agents
- Check [CLI reference](/reference/cli) for all options
