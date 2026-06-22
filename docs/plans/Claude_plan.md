Plan: ck-mlx — a Metal-accelerated local code search tool

 Context

 ck (BeaconBay, Rust + FastEmbed/ONNX) is the current semantic code-search engine for
 hermes-agent. On Apple Silicon it runs embeddings on CPU via ONNX, which pegged
 650–760% CPU and 1+ GB RAM during reindex — repeatedly observed in this session
 (jina-code and nomic-v1.5 switches both spiked; one stale v0.7.11 process had run
 95+ hours). The published "1M LOC in <2 min" benchmark does not hold on this
 hardware. ck's embedding backend (ck-embed crate) is compiled in — it cannot be
 pointed at an external embedder.

 Meanwhile the user already runs oMLX (localhost:8000), a Metal-accelerated,
 OpenAI-compatible model server using mlx-embeddings. It exposes /v1/embeddings and
 /v1/rerank. The hard part of any embedding-search tool — fast local inference on
 Apple Silicon — is therefore already solved and external to our code.

 Goal: build a small Python tool, ck-mlx, that replicates ck's search UX (CLI + MCP
 semantic_search/hybrid_search/index_status) but delegates embedding to oMLX over
 HTTP. Outcome: same search quality with the Neural-Engine/Metal path, so indexing no
 longer burns CPU. Not a literal fork of ck's Rust source — a clean Python
 reimplementation of its feature set. (Name ck-mlx is a placeholder; adjustable.)

 Architecture

 Standalone uv-managed project at ~/code/ck-mlx/. Embedding is ~10 lines (the openai
 SDK pointed at oMLX); everything else is orchestration ck already proved out.

 file walk + .ckignore -> chunker -> oMLX /v1/embeddings (batched)
 |
 sqlite-vec store (vectors + metadata, incremental)
 |
 search: semantic (cosine) | BM25 (FTS5) | hybrid (RRF) | rerank (/v1/rerank)
 |
 CLI (typer) + MCP server (fastmcp)

 Components (files under ck_mlx/)

 - embed.py — openai SDK client, base_url="http://localhost:8000/v1", key from
 OMLX_API_KEY. embed(texts: list[str]) -> list[vec] sends batched input arrays;
 concurrency bounded to oMLX's --max-concurrent-requests (default 8). Handles the
 query-vs-document prefix (see research items).
 - chunk.py — THE quality-critical piece. Function/class-aware splitting via
 tree-sitter-language-pack, fallback to fixed token windows for unknown file types.
 Token-count to zembed-1's max sequence length with ~15% overlap. Tokenizer via
 tokenizers matching the model family.
 - walk.py — pathspec for .ckignore/.gitignore + os.walk. Must exclude .ck-mlx/ (own
 index) and **/node_modules/ — the exact runaway-loop lessons from ck this session.
 Seed default ignores from the hardened ~/.hermes/hermes-agent/.ckignore.
 - store.py — SQLite + sqlite-vec. Schema: chunks(id, path, start_line, end_line,
 content, mtime, content_hash) + a vec0 virtual table. Incremental: skip files whose
 (mtime, hash) is unchanged → cheap reindex.
 - search.py — semantic (embed query → vec top-k), lexical (SQLite FTS5 BM25), hybrid
 (RRF fusion), optional rerank (POST top-N to /v1/rerank).
 - server.py — fastmcp exposing semantic_search, hybrid_search, regex_search,
 index_status, matching ck-search's tool names/params so it drops into existing
 .claude/settings.json, codex, and opencode configs with a one-line MCP swap.
 - cli.py — ck-mlx index ., ck-mlx search "...", ck-mlx status; mirror ck flags where
 cheap.

 Dependencies

 openai, sqlite-vec, pathspec, tree-sitter-language-pack, tokenizers, fastmcp, typer.
 (BM25 via stdlib SQLite FTS5 — no extra dep.)

 Build stages (each independently useful)

 1. MVP (~day): walk (fixed-window chunking) → embed → store → cosine top-k → cli
 search + MCP semantic_search. Replaces ck's semantic_search.
 2. Incremental + ignore: .ckignore parsing, mtime/hash skip, index_status. Cheap
 re-indexing.
 3. Hybrid: FTS5 BM25 + RRF fusion; MCP hybrid_search. Full day-to-day ck parity.
 4. Quality + integration: /v1/rerank; tree-sitter function-aware chunking; swap the 3. Hybrid: FTS5 BM25 + RRF fusion; MCP hybrid_search. Full day-to-day ck parity. 4. Quality + integration: /v1/rerank; tree-sitter function-aware chunking; swap the ck-search MCP entry for ck-mlx in the Claude/codex/opencode configs. Prerequisites / research (resolve before Stage 1 indexing) These need the user or the oMLX key; they gate quality, not the plan: - OMLX_API_KEY — required by /v1/embeddings and admin endpoints. User to provide / name the env var. - zembed-1 specs — max sequence length (sets chunk ceiling), embedding dimensions (sets vec table width), and whether it needs query: / search_document: prefixes (wrong prefix silently tanks recall). Pull from oMLX admin (/admin/api/models, needs key) or the model card. - Smoke test /v1/embeddings returns a vector of the expected dimension before building the store. Verification - Endpoint smoke test: openai call to oMLX returns a correctly-sized vector. - Index hermes-agent end-to-end: ck-mlx index ~/.hermes/hermes-agent, confirm file/chunk counts comparable to ck's ~1,300 files. - Resource win (the whole point): watch ps aux during index — oMLX stays Metal-bound and modest; no 700%-CPU ck-style spike. - Quality parity: run a handful of known queries (e.g. "code that retries on timeout", "fallback chain reset") against both ck and ck-mlx; results should be comparable or better. - MCP integration: point a config's MCP entry at ck-mlx, run a search through the agent, confirm tool-shape compatibility. Risks / notes - Chunking is the main quality risk — staged so fixed-window ships first and tree-sitter is a later upgrade; prefix handling (above) matters as much as splitting. - sqlite-vec on Apple Silicon — installs via pip wheel; confirm load at Stage 1. - CPU win is contingent on oMLX genuinely using Metal (mlx-embeddings confirms it does). Step 0 (on approval) Create ~/code/ck-mlx/, uv init, and write this plan to ~/code/ck-mlx/PLAN.md as the handoff doc before any code — since plan mode could not write to ~/code directly.
