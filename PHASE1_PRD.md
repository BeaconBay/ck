# Phase 1 PRD: Agent-Ready Code Navigation

## Executive Summary

Transform `ck` into the definitive AI agent toolkit for code navigation by adding structured output, improved filtering, symbol indexing, and edit anchors. Focus: agent-ready primitives that maintain grep-like composability.

## Feature 1: JSONL Output (Agent-Friendly Structured Data)

### Problem Statement
AI agents struggle with grep's unstructured text output, requiring fragile regex parsing. Agents need reliable, structured data for automation.

### Solution
Add `--jsonl` flag for machine-readable output format.

### Acceptance Criteria

**CLI Interface:**
```bash
# Basic JSONL output
ck "function.*auth" --jsonl

# JSONL with options
ck --sem "authentication logic" --jsonl --no-snippet --context 3 --topk 10
```

**Output Format:**
```jsonl
{"path":"src/auth.ts","span":{"line_start":42,"line_end":58,"byte_start":1203,"byte_end":1456},"language":"typescript","snippet":"function authenticate(user) {...}","score":0.89,"chunk_hash":"abc123","index_epoch":1699123456}
{"path":"lib/user.py","span":{"line_start":15,"line_end":23,"byte_start":456,"byte_end":789},"language":"python","snippet":"def login(username):...","score":0.76,"chunk_hash":"def456","index_epoch":1699123456}
```

**Required Fields:**
- `path`: Relative file path
- `span`: Object with `line_start`, `line_end`, `byte_start`, `byte_end`
- `language`: Detected programming language
- `snippet`: Code snippet (optional with `--no-snippet`)
- `score`: Relevance score (0.0-1.0, null for regex)
- `chunk_hash`: Unique hash for this code chunk
- `index_epoch`: Timestamp when index was created

**Flags:**
- `--jsonl`: Enable JSONL output mode
- `--no-snippet`: Exclude code snippets (just metadata)
- `--context N`: Include N lines before/after match
- `--topk N`: Limit results to top N matches
- `--threshold S`: Minimum similarity score (0.0-1.0)
- `--timeout MS`: Query timeout in milliseconds

**Implementation:**
- **Module**: `ck-cli/src/main.rs` - Add JSONL output option
- **Module**: `ck-cli/src/output.rs` - Create new output formatter
- **Module**: `ck-core/src/lib.rs` - Extend SearchResult struct
- **Tests**: Comprehensive JSONL format validation

---

## Feature 2: Improved Filtering (Directory and Language Constraints)

### Problem Statement
Current filtering requires complex regex patterns. Agents need simple, predictable constraints for scoping searches.

### Solution
Add intuitive directory and language filtering flags.

### Acceptance Criteria

**CLI Interface:**
```bash
# Directory filtering
ck "auth" --in src,lib --not-in tests,node_modules

# Language filtering  
ck "class.*User" --lang js,ts,py

# Combined filtering
ck --sem "database connection" --in src --lang py,js --jsonl
```

**Flags:**
- `--in <dir[,dir...]>`: Include only specified directories
- `--not-in <dir[,dir...]>`: Exclude specified directories  
- `--lang <ext[,ext...]>`: Filter by file extensions/languages

**Supported Languages:**
- `js,ts,jsx,tsx` (JavaScript/TypeScript)
- `py` (Python)
- `rs` (Rust)
- `go` (Go)
- `rb` (Ruby)
- `hs` (Haskell)
- `java` (Java)
- `cpp,cc,cxx` (C++)

**Implementation:**
- **Module**: `ck-cli/src/main.rs` - Add filtering CLI args
- **Module**: `ck-index/src/lib.rs` - Extend filtering logic in existing override builder
- **Module**: `ck-core/src/lib.rs` - Add language detection utilities
- **Tests**: Directory and language filtering validation

---

## Feature 3: Symbol Index (Definitions/References/Exports)

### Problem Statement
Finding where symbols are defined, used, or exported requires manual scanning. Agents need fast symbol navigation.

### Solution
Add `ck sym` subcommand for symbol-based queries using ctags-grade heuristics.

### Acceptance Criteria

**CLI Interface:**
```bash
# Find symbol definitions
ck sym --def authenticate

# Find symbol references
ck sym --refs UserController

# Pattern-based symbol search
ck sym --like ".*Handler"

# List all exports
ck sym --exports --lang ts,js
```

**Output Format (JSONL):**
```jsonl
{"symbol":"authenticate","kind":"function","role":"definition","path":"src/auth.ts","line":42,"byte":1203,"language":"typescript"}
{"symbol":"authenticate","kind":"function","role":"reference","path":"src/login.ts","line":15,"byte":456,"language":"typescript"}
{"symbol":"UserModel","kind":"class","role":"export","path":"src/models/user.ts","line":1,"byte":0,"language":"typescript"}
```

**Symbol Kinds:**
- `function`, `class`, `interface`, `type`, `const`, `var`, `import`, `export`

**Symbol Roles:**
- `definition`: Where symbol is defined
- `reference`: Where symbol is used
- `export`: Where symbol is exported

**Implementation:**
- **Module**: `ck-cli/src/sym.rs` - New subcommand implementation
- **Module**: `ck-symbols/` - New crate for symbol extraction
- **Module**: `ck-symbols/src/extractors/` - Language-specific symbol extractors
- **Dependencies**: Leverage existing tree-sitter parsers
- **Tests**: Symbol extraction accuracy per language

---

## Feature 4: Anchors for Patch Safety (Edit Context Verification)

### Problem Statement
Agents making code edits need stable reference points to verify context hasn't changed before applying modifications.

### Solution
Add `ck anchors` subcommand to generate stable edit anchors around code spans.

### Acceptance Criteria

**CLI Interface:**
```bash
# Generate anchors for specific line range
ck anchors src/auth.ts --line-start 42 --line-end 58 --json

# Generate anchors with context
ck anchors src/auth.ts --line-start 42 --line-end 58 --context 3 --json
```

**Output Format:**
```json
{
  "file": "src/auth.ts",
  "target_span": {"line_start": 42, "line_end": 58, "byte_start": 1203, "byte_end": 1456},
  "pre_anchor": "export class UserAuth {",
  "post_anchor": "  public logout(): void {",
  "chunk_hash": "abc123def456",
  "context_lines": 3,
  "generated_at": 1699123456
}
```

**Flags:**
- `--line-start N`: Start line of target span
- `--line-end N`: End line of target span  
- `--context N`: Lines of context for anchor generation (default: 2)
- `--json`: Output as JSON (default for anchors)

**Anchor Selection Algorithm:**
1. Find stable lines before/after target (class/function declarations, comments)
2. Prefer unique, unlikely-to-change identifiers
3. Include enough context to uniquely identify location
4. Generate content hash for change detection

**Implementation:**
- **Module**: `ck-cli/src/anchors.rs` - New subcommand
- **Module**: `ck-core/src/anchors.rs` - Anchor generation logic
- **Tests**: Anchor stability and uniqueness validation

---

## Implementation Approach

### Feature Priority
1. **JSONL Output** - Foundation for all agent interactions
2. **Enhanced Filtering** - Builds on existing systems  
3. **Symbol Index** - Leverages tree-sitter infrastructure
4. **Edit Anchors** - Enables safe programmatic modifications

### Development Strategy
- Iterative implementation with immediate testing
- Each feature provides standalone value
- Maintain backward compatibility
- Preserve grep-like performance characteristics

### Quality Gates
- **Unit tests**: Each feature in isolation
- **Integration tests**: Cross-feature compatibility  
- **Agent simulation**: Real workflow validation
- **Performance**: Sub-500ms query responses

---

## Future Considerations (Phase 2+)

This Phase 1 foundation enables:
- **Import/Call graphs**: Building on symbol index
- **Configurable context**: Extending anchor system  
- **ck-toolbelt**: Higher-order repo analysis tools

Each Phase 1 feature is designed to be independently valuable while creating building blocks for advanced agent workflows.