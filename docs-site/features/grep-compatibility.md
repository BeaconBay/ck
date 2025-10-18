---
title: grep Compatibility
description: ck is a drop-in replacement for grep and ripgrep. All standard flags work, with automatic smart filtering and optional semantic enhancement.
---

# grep Compatibility

ck is a drop-in replacement for grep/ripgrep. All your muscle memory works.

## Philosophy

ck maintains full compatibility with standard grep flags and behavior while adding semantic capabilities. You don’t need to learn new tools — ck enhances what you already know.

## Supported Flags

### Basic Search

```bash
# Pattern search
ck "pattern" file.txt
ck "pattern" *.rs
ck "TODO|FIXME" src/

# Case-insensitive
ck -i "warning" src/

# Whole word match
ck -w "test" src/

# Invert match
ck -v "exclude" src/
```

### Output Control

```bash
# Line numbers
ck -n "pattern" file.txt

# Only filenames with matches
ck -l "pattern" src/

# Only filenames without matches
ck -L "pattern" src/

# Hide filenames
ck --no-filename "pattern" file1 file2

# Count matches
ck -c "pattern" src/
```

### Context Display

```bash
# N lines after match
ck -A 3 "error" src/

# N lines before match
ck -B 2 "error" src/

# N lines before and after
ck -C 2 "error" src/

# Context with line numbers
ck -n -C 3 "error" src/
```

### Recursive Search

```bash
# Recursive
ck -R "pattern" .

# Alternative
ck -r "pattern" .

# With exclusions
ck -R --exclude "*.test.js" "pattern" src/
```

### File Filtering

```bash
# Specific file types
ck "pattern" **/*.rs
ck "pattern" **/*.{js,ts}

# Exclude patterns
ck --exclude "*.test.js" "pattern" src/
ck --exclude "node_modules" "pattern" .

# Multiple exclusions
ck --exclude "dist" --exclude "build" "pattern" .
```

## grep vs ck Examples

All these work identically in both tools:

```bash
# Find TODO comments
grep -rn "TODO" src/
ck -rn "TODO" src/

# Case-insensitive search with context
grep -i -C 2 "error" src/
ck -i -C 2 "error" src/

# List files containing pattern
grep -rl "auth" src/
ck -rl "auth" src/

# Count matches per file
grep -rc "function" src/
ck -rc "function" src/

# Invert match
grep -v "test" file.txt
ck -v "test" file.txt
```

## Differences from grep

### Enhanced Features

ck adds semantic search without breaking compatibility:

```bash
# Traditional grep behavior
ck "error" src/           # Exact keyword match

# Enhanced semantic search (opt-in)
ck --sem "error" src/     # Conceptual match
```

### Automatic Smart Filtering

ck automatically excludes:
- Binary files
- `.git` directories
- `.gitignore` patterns
- `.ckignore` patterns
- Build artifacts

```bash
# grep searches everything
grep -r "pattern" .       # Searches .git, binaries, etc.

# ck is smarter by default
ck -r "pattern" .         # Automatically filters noise

# Disable if needed
ck --no-ignore --no-ckignore -r "pattern" .
```

### Output Stream Separation

```bash
# ck properly separates output
ck "pattern" src/ > results.txt           # Only results
ck --status . 2> /dev/null                # Only status

# grep mixes streams
grep "pattern" src/ 2>&1 | process.sh     # Mixed output
```

## Migration from grep/ripgrep

### Direct Replacement

```bash
# In shell aliases
alias grep='ck'

# In scripts
#!/bin/bash
# sed 's/grep/ck/g' old_script.sh > new_script.sh

# No changes needed - works immediately
ck -rn "TODO" src/
```

### Gradual Adoption

```bash
# Keep grep for basic tasks
grep "simple pattern" file.txt

# Use ck for code search
ck --sem "authentication" src/

# Use ck hybrid for complex patterns
ck --hybrid "connection timeout" src/
```

## Advanced grep Features

### Regular Expressions

```bash
# Extended regex (ERE)
ck "function.*auth" src/

# Match word boundaries
ck -w "test" src/

# Multiple patterns
ck "error|warning|fatal" logs/
```

### Multiple Files

```bash
# Multiple file patterns
ck "pattern" *.rs *.go *.py

# Recursive with globbing
ck "pattern" src/**/*.rs
ck "pattern" {src,lib,tests}/**/*.rs
```

### Output Formatting

```bash
# No filename prefix (single file)
ck "pattern" file.txt

# With filename prefix (multiple files)
ck "pattern" *.rs

# Force no filename
ck --no-filename "pattern" *.rs
```

## Performance Comparison

| Tool | Speed | Features | Use Case |
|------|-------|----------|----------|
| grep | Fast | Basic | Simple text search |
| ripgrep | Fastest | Enhanced | Fast code search |
| ck | Fast | Semantic | Concept search + grep |

```bash
# For simple keyword search, all are fast:
time grep -r "pattern" src/     # ~100ms
time rg "pattern" src/          # ~50ms
time ck "pattern" src/          # ~60ms

# ck adds semantic without slowing keyword search:
time ck --sem "pattern" src/    # ~400ms (includes semantic understanding)
```

## Best Practices

### When to Use grep-Style Search

✅ **Use keyword search for:**
- Exact string matches
- Variable/function names
- File paths
- Configuration values
- Log analysis

```bash
ck "TODO" src/                  # Find exact TODO markers
ck "fn main" src/               # Find main functions
ck "import.*React" src/         # Find React imports
```

### When to Add Semantic Search

✅ **Add semantic search for:**
- Conceptual code search
- Refactoring
- Understanding unfamiliar code
- Finding design patterns

```bash
ck --sem "error handling" src/  # Find all error patterns
ck --hybrid "auth" src/         # Find auth with keyword present
```

## Integration

### Shell Aliases

```bash
# Replace grep
alias grep='ck'
alias rg='ck'

# Add semantic alias
alias sgrep='ck --sem'
alias hgrep='ck --hybrid'
```

### Git Aliases

```bash
# .gitconfig
[alias]
    search = "!f() { git ls-files | xargs ck \"$@\"; }; f"
    ssearch = "!f() { git ls-files | xargs ck --sem \"$@\"; }; f"
```

### Editor Integration

```vim
" Vim/Neovim
set grepprg=ck\ --vimgrep\ $*
set grepformat=%f:%l:%c:%m

" Search in vim
:grep pattern
:copen
```

## Troubleshooting

### Results Differ from grep

```bash
# Check if .gitignore/.ckignore excluding files
ck --no-ignore --no-ckignore "pattern" .

# Verify binary file handling
ck --no-ignore "pattern" .
```

### Performance Issues

```bash
# Use specific paths
ck "pattern" src/ instead of ck "pattern" .

# Exclude large directories
ck --exclude "node_modules" "pattern" .

# Check index status (doesn't affect keyword search)
ck --status .
```

## Next Steps

- Learn [basic usage](/guide/basic-usage) patterns
- Try [semantic search](/features/semantic-search)
- Explore [hybrid search](/features/hybrid-search)
- Check [CLI reference](/reference/cli)
