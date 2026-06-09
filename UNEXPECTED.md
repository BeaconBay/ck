# Unexpected Behaviors in ck

This file tracks instances where ck behaves unexpectedly during testing or usage.

## Format

**Command:** `command run`  
**Expected:** What should have happened  
**Actual:** What actually happened  
**Date:** YYYY-MM-DD  
**Status:** [Fixed/Open/Investigating]

---

## Issues Found

**Command:** `ck ""`
**Expected:** Error message or no results
**Actual:** Massive output with every empty line in the codebase being matched
**Date:** 2025-09-13
**Status:** Open
**Notes:** Empty pattern matches all empty lines, resulting in overwhelming output. Perhaps should warn or limit results?

---

**Command:** `ck --add /tmp/test.txt`
**Expected:** Add file to index or meaningful error
**Actual:** Error: "No file specified. Usage: ck --add <file>"
**Date:** 2025-09-13
**Status:** Open
**Notes:** The error message is incorrect - a file was specified. Seems like argument parsing issue.

---

**Command:** `ck --sem "🎉🦀✨"`
**Expected:** No results or error about emoji patterns
**Actual:** Returns seemingly random code results
**Date:** 2025-09-13
**Status:** Open
**Notes:** Emoji search returns unrelated results. The semantic embedding seems to handle emojis unpredictably.

---

**Command:** `ck -i "rrf|reciprocal" ck-engine/src/lib.rs | head -8`
**Expected:** First 8 matches, clean exit (grep behavior under SIGPIPE)
**Actual:** Rust panic printed to stderr: `thread 'main' panicked at ... failed printing to stdout: Broken pipe (os error 32)`
**Date:** 2026-06-09
**Status:** Fixed (fix/sigpipe-panic — SIGPIPE default disposition restored on Unix at startup; exits 141 like grep)
**Notes:** Piping into `head`/`less` is core grep usage. Needs SIGPIPE handling (e.g. restore default SIGPIPE disposition on Unix, or treat BrokenPipe write errors as silent success).

---

**Command:** MCP `index_status` on this repo
**Expected:** Index size reflecting 1825 embedded chunks (tens of MB)
**Actual:** `index_size_bytes: 928` — appears to report only the manifest, not the actual `.ck` directory size
**Date:** 2026-06-09
**Status:** Open

---

**Command:** MCP `semantic_search` query "where does the RRF reciprocal rank fusion merging of regex and semantic results happen"
**Expected:** `hybrid_search_with_progress` / RRF scoring code in ck-engine/src/lib.rs (it exists, has RRF comments, and is regex-findable)
**Actual:** Single result: a docs-site roadmap.md bullet (score 0.60). The actual implementation never surfaced; default threshold 0.6 filtered everything else. Same miss via CLI `--sem` scoped to ck-engine/ (best candidate scored 0.578, and was the wrong chunk). `--hybrid "RRF reciprocal rank fusion"` also failed to surface it in top 5.
**Date:** 2026-06-09
**Status:** Partially fixed (fix/hybrid-nl-queries — hybrid's keyword arm now falls back to IDF-ranked term matching for NL queries, fuses line hits into containing semantic chunks, and widens both arms pre-fusion; the scoped query now ranks the implementation #1, was #4). Remaining open: pure `--sem` on doc-heavy repos still ranks docs above implementations (semantic rank 87 for this chunk — needs corpus-level work, e.g. doc/code balancing or better chunk identity, not a threshold tweak).
**Notes:** Also: that first MCP search reported `search_time_ms: 1839209` (~30 min) — metric was real wall time including silent auto-reindexing; fixed separately in fix/mcp-index-metrics by reporting indexing separately.

---

**Command:** `ck --index /some/other/dir` (from inside a different directory)
**Expected:** Indexes `/some/other/dir` ("Create or update search index for the specified path")
**Actual:** The path argument lands in the positional *pattern* slot, `files` stays empty, and ck indexes the **current working directory** instead. `ck --index .` only works by coincidence (the "." pattern is ignored and "." is also the default path). Likely the same root cause as the `ck --add /tmp/test.txt` entry above.
**Date:** 2026-06-10
**Status:** Open
**Notes:** Found while writing a concurrent-indexing test: two `ck --index <tempdir>` processes silently indexed the ck repo itself (cwd). Command-mode flags (`--index`, `--clean`, `--add`, …) should treat the first positional arg as their target path.

---

## Instructions

When you encounter unexpected behavior while using ck:

1. Note the exact command you ran
2. Describe what you expected to happen
3. Describe what actually happened
4. Add the date
5. Set status to "Open"

This helps track and fix edge cases and user experience issues.