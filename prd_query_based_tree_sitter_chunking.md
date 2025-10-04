# PRD: Query-Based Tree-Sitter Chunking

**Version**: 1.0  
**Date**: 2025-01-04  
**Status**: Draft  
**Author**: System Architecture

---

## Executive Summary

Refactor ck's language-specific chunking system so that chunk detection is driven by Tree-sitter queries rather than handwritten pattern matching, and enrich every chunk with contextual metadata (hierarchical ancestry, attached comments, trailing trivia, and size). The new design replaces hundreds of lines of imperative language-specific code with declarative `tags.scm` files plus a small shared refinement pipeline, while guaranteeing that the resulting chunks are semantically meaningful and usable within strict token limits. The migration is incremental and backwards-compatible: individual languages can switch to query-based chunking while others continue to use the current implementation.

**Current**: hard-coded matches across nine languages, lossy context handling, and no persisted ancestry.  
**Target**: per-language query files, shared refinement, and metadata-rich chunks that can be tailored to downstream embedding limits.

---

## Problem Statement

### Current Issues

1. **High maintenance burden**
   - Adding or updating a language requires editing Rust.
   - Grammar changes silently break our chunk recognition.
   - Logic is scattered across multiple functions.
2. **Incorrect semantics**
   - Haskell signatures and bodies are split, producing dozens of micro-chunks.
   - No automated validation; correctness requires manual inspection.
   - Complex constructs (e.g., arrow functions bound to constants) are cumbersome to express.
3. **Limited extensibility**
   - Community contributors need Rust expertise.
   - We lack a standard format describing what constitutes a chunk.
4. **Missing hierarchical and narrative context**
   - Chunks lose the enclosing module/class structure, forcing callers to re-derive ancestry.
   - Leading/trailing comments and docstrings are often dropped.
   - Single-line fragments surface for constructs like arrow functions despite surrounding declarations.
5. **Token-limit friction**
   - Default deployment relies on `bge-small` (~512 token limit), so long chunks are aggressively strided without conveying why or providing guidance for larger-context embeddings.

**Example – Haskell**  
Legacy code matched `signature`, `data_type`, etc. and then relied on bespoke merging logic. With queries we can capture both signatures and function bodies by name and merge them declaratively.

---

## Goals and Non-Goals

### Goals

1. **Declarative chunk definitions**
   - Each language ships a `queries/<language>/tags.scm` file using standard Tree-sitter S-expressions.
   - Queries are testable via `tree-sitter test` fixtures.
2. **Preserve refinement pipeline**
   - Continue attaching leading comments/decorators.
   - Continue expanding arrow functions to their declarations.
   - Retain striding logic for large chunks.
3. **Incremental migration**
   - Query and legacy strategies can coexist per language.
   - Roll out language-by-language with easy fallback.
4. **Community extensibility**
   - Language experts contribute queries without Rust changes.
   - Query edits do not require recompilation (queries are embedded but optionally overrideable).
5. **First-class contextual metadata**
    - Capture ancestry breadcrumbs (e.g., `module::Class::method`) for every chunk.
    - Preserve leading and trailing narrative context (comments, docstrings, qualifiers) in chunk text or metadata.
    - Persist chunk size estimates and striding hints so callers can choose appropriate embedding models.

### Non-Goals

- Rewriting Tree-sitter or changing chunk data structures (`Chunk`, `Span`, `ChunkType`).
- Supporting alternative query syntaxes beyond Tree-sitter S-expressions.
- Eliminating all imperative code; refinement logic remains in Rust.

---

## Success Metrics

| Metric | Current | Target | Measurement |
| --- | --- | --- | --- |
| Lines of chunk-matching Rust | ~580 | < 200 | `git diff --stat` |
| Haskell chunk accuracy | 44% (17/39) | > 90% | Manual inspection + clamps |
| Time to add new language | 2–4 hours | < 30 minutes | Developer survey |
| Chunk tests | Manual only | Automated | `tree-sitter test` |
| Chunks with ancestry & doc metadata | 0% | 100% | Sampled index inspection |
| Chunks under default token budget (512) with recorded downsampling guidance | <50% | ≥95% | Benchmark suite |
| Community language contributions | 0/year | ≥3/year | GitHub PR tracking |

---

## Architecture Overview

### Processing Flow

1. **Parse** – Build the syntax tree via Tree-sitter.
2. **Execute query (new)** – Load `tags.scm` and apply it to the parse tree.
3. **Extract chunks** – Convert captures (e.g., `@definition.function`) into `Chunk` structs.
4. **Annotate ancestry (new)** – Walk parent nodes to build a lexical breadcrumb (e.g., `module::Class::method`).
5. **Refine** – Run existing helpers (`extend_with_leading_trivia`, `expand_arrow_function_context`, `adjust_chunk_type_for_context`).
6. **Stride** – Apply existing striding when chunks exceed size limits.

### Component Design

1. **Query loading**
   - Embed per-language `tags.scm` files and support optional overrides for experimentation.
   - Provide a compatibility path when queries are missing during migration.

2. **Query execution & chunk extraction**
   - Apply queries to the syntax tree to capture semantic units (functions, methods, classes, modules) rather than raw nodes.
   - Derive lexical ancestry by traversing parent nodes and record it as metadata.
   - Collect associated comments/docstrings either through explicit captures or refinement.
   - Track chunk size (token estimate) alongside the text so downstream consumers can choose embeddings or stride appropriately.

3. **Refinement pipeline**
   - Attach leading/trailing trivia, ensure minimal chunk span thresholds, and handle arrow-function expansion generically.
   - Stabilize chunk lengths so they stay informative and respect the default embedding budget.

4. **Compatibility layer**
   - Allow languages to opt in gradually; when queries are absent we revert to the existing extractor and mark metadata gaps (no ancestry/doc info).

### Query Conventions

| Capture | Meaning | ChunkType |
| --- | --- | --- |
| `@definition.function` | Top-level functions | `Function` |
| `@definition.method` | Methods inside classes/impls | `Method` |
| `@definition.class` | Classes, structs, data types | `Class` |
| `@definition.module` | Modules/namespaces | `Module` |
| `@name` | Identifier (required) | Metadata |
| `@doc` | Documentation (optional) | Metadata |
| `@context` (derived) | Ancestry breadcrumb | Metadata |

Example (`queries/rust/tags.scm`):

```scm
(function_item
  name: (identifier) @name) @definition.function

(impl_item
  body: (declaration_list
    (function_item
      name: (identifier) @name) @definition.method))
```

Advanced patterns include multi-node grouping for Haskell, declaration-based arrow functions for TypeScript/JavaScript, and filters that exclude locals for Go. Ancestry computation is generic: once we obtain the captured node we climb parents until the file root, collecting any ancestor names we can derive from the grammar (module → class → method, etc.). When queries reveal doc comments (e.g., via `@doc` capture) we attach them directly, eliminating the need for ad hoc whitespace heuristics.

---

## Implementation Plan

### Phase 1 – Foundation (Week 1)
- Introduce query loading/execution infrastructure and the migration-friendly compatibility layer.
- Port Rust to query-based chunking while other languages remain legacy.
- Emit ancestry, documentation, and token-size metadata for Rust chunks and persist it in the index.
- Deliverables: `queries/rust/tags.scm`, query executor, metadata schema updates, and regression tests comparing query vs legacy Rust results on representative files.

### Phase 2 – Core Languages (Week 2)
- Add query files and fixtures for Python, TypeScript/JavaScript, and Go.
- Validate ancestry breadcrumbs and comment preservation across representative samples.
- Ensure default chunk sizes respect embedding budgets; annotate any segments that require striding.
- Update CLI/TUI chunk visualisation to surface the new metadata (breadcrumbs, stride hints, documentation presence) and support dynamic overlap widths.
- Deliverables: queries for core languages, updated UI components, metadata validation summary.

### Phase 3 – Complex Languages (Week 3)
- Implement Haskell queries with signature/function pairing and retire bespoke merge logic.
- Add Ruby, C#, and Zig query files; verify nested structures, decorators, and trailing documentation remain intact.
- Extend CLI/TUI regression coverage to confirm hierarchical metadata renders correctly across languages and view modes.
- Deliverables: queries for remaining languages, enhanced automated checks, documented improvements (e.g., Haskell chunk reduction).

### Phase 4 – Cleanup & Documentation (Week 4)
- Remove legacy chunk matching once all languages are ported to queries.
- Finalize documentation for query authoring, metadata conventions, and embedding guidance (CONTRIBUTING, UI docs, migration notes/blog post).
- Complete performance benchmarking for chunking throughput and index footprint.

## Testing Strategy

1. **Query unit tests** – Tree-sitter fixtures per language (e.g., `queries/rust/tests/functions.txt`) asserting expected captures.
2. **Comparison tests** – Rust tests that diff legacy vs query results until legacy is removed.
3. **Metadata regression suite** – Updated tests validating hierarchical breadcrumbs, comment/doc inclusion, minimum chunk spans, and token estimates across representative samples.
4. **UI integration tests** – Refactored CLI/TUI snapshot or golden-output tests confirming chunk overlays render breadcrumbs, stride hints, and overlapping markers correctly.
5. **Performance benchmarks** – Measure chunking throughput and confirm chunk size metadata aligns with `bge-small` targets (512 tokens) while retaining fidelity.

---

## Migration Guide

### For Language Maintainers

1. Create `queries/<language>/tags.scm` with standard captures.
2. Copy or adapt patterns from upstream `tree-sitter-<language>` queries.
3. Add Tree-sitter tests verifying captures.
4. Validate ancestry breadcrumbs and comment preservation with sample files (ensure modules/classes and docstrings are picked up).
5. Ensure token estimates stay within default embedding budgets (or note expected striding).
6. Submit PR—no Rust changes required.

### For ck Contributors

- During migration, languages can opt into query-based chunking by supplying a query file. Others continue using the legacy matcher until their queries are ready.
- After all languages are ported, delete legacy code paths and require queries.

---

## Risks and Mitigations

| Risk | Impact | Probability | Mitigation |
| --- | --- | --- | --- |
| Query execution is slower than legacy | Medium | Low | Benchmark early, cache compiled queries, profile hot paths |
| Grammar updates break queries | High | Low | Pin Tree-sitter versions, add compatibility tests |
| Some semantics are hard to express declaratively | High | Medium | Keep refinement and limited fallback logic; support hybrid approaches |
| Metadata footprint grows | Low | Medium | Store ancestry/comments as compact structures, allow opt-out for constrained indices |
| Default embedding budget still exceeded | Medium | Medium | Enforce minimum chunk size thresholds, annotate oversize chunks with guidance, consider adaptive striding |
| Community adoption remains low | Low | Medium | Provide templates, tutorials, and highlight query contributions |
| Query syntax learning curve | Low | High | Document authoring, share boilerplate, consider tooling |

---

## Open Questions

1. **Query caching** – Parse all queries at startup vs lazy-load per language? (Recommendation: parse at startup; cost is negligible.)
2. **Missing queries** – Continue fallback to legacy during migration, tighten to hard error once all languages are ported.
3. **Version compatibility** – Maintain single set of queries per language; rely on compatibility tests to catch grammar changes.
4. **Breadcrumb format** – Decide on canonical delimiter and maximum depth (recommend `::`, configurable per consumer).
5. **Embedding strategy** – Determine when to automatically recommend larger-context models (e.g., upgrade from `bge-small` to Nomic v1.5) based on chunk metadata.
6. **Override strategy** – Embed queries in binary but allow filesystem overrides for experimentation.

---

## Future Enhancements

- **Scope-aware chunking** (`locals.scm`) to track variable definitions and references.
- **Custom chunk metadata** via `#set!` to tag documentation, tests, or deprecated blocks.
- **Query composition** for sharing common patterns across languages.
- **IDE integrations** that surface chunk boundaries and navigation.
- **Community query marketplace** showcasing language support status and best-practice queries.

---

## Appendices

### Appendix A – Sample Queries

See `queries/rust/tags.scm`, `queries/haskell/tags.scm`, and `queries/typescript/tags.scm` for concrete examples covering functions, methods, classes, modules, and advanced patterns.

### Appendix B – Migration Checklist

- **Phase 1**: Create query infrastructure, port Rust, add comparison tests.
- **Phase 2**: Add Python, TypeScript/JavaScript, Go queries + tests; run benchmarks.
- **Phase 3**: Port Haskell, Ruby, C#, Zig; validate improvements.
- **Phase 4**: Remove legacy logic, update documentation, publish performance results.

---

_End of document._
