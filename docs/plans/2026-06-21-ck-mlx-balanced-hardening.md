# ck-mlx Balanced Hardening Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove brittle model assumptions from `ck-mlx`, especially the fixed embedding width in the SQLite vector store, then verify CLI and MCP behavior still work.

**Architecture:** Keep the existing `ck_mlx` structure intact, but move model-specific assumptions to runtime discovery and persisted index metadata. The store should initialize vector schema from the active embedding model instead of assuming `2560`, and the CLI/search layers should fail clearly when the configured model does not match an existing index.

**Tech Stack:** Python, Typer, FastMCP, OpenAI-compatible oMLX API, SQLite, sqlite-vec.

---

### Task 1: Add runtime embedding model metadata and dimension discovery

**Files:**
- Modify: `ck-mlx/ck_mlx/embed.py`
- Test: `ck-mlx/tests/test_embed_metadata.py`

**Step 1: Write the failing test**

Create a test that stubs the embedding client response and asserts a helper can return both the embedding vector and its dimension for the configured model.

**Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_embed_metadata.py -v`
Expected: FAIL because metadata helper does not exist.

**Step 3: Write minimal implementation**

Add a small metadata helper in `embed.py` that:
- centralizes base URL / API key / model selection
- performs a one-text embedding smoke call when dimension is unknown
- returns the discovered dimension and model name

**Step 4: Run test to verify it passes**

Run: `uv run pytest tests/test_embed_metadata.py -v`
Expected: PASS.

**Step 5: Commit**

Skip commit in this session unless explicitly requested by the user.

### Task 2: Remove fixed vector width from the store and persist index metadata

**Files:**
- Modify: `ck-mlx/ck_mlx/store.py`
- Test: `ck-mlx/tests/test_store_dynamic_dimensions.py`

**Step 1: Write the failing test**

Create a test that initializes the store with a non-2560 dimension (for example 8), inserts data, and verifies the schema and metadata are created correctly.

**Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_store_dynamic_dimensions.py -v`
Expected: FAIL because the store hardcodes `float[2560]`.

**Step 3: Write minimal implementation**

Update `store.py` to:
- require or derive an embedding dimension at initialization time when creating a new index
- create a metadata table for active model name and vector dimension
- validate an existing index against the currently configured dimension
- expose metadata via status helpers

**Step 4: Run test to verify it passes**

Run: `uv run pytest tests/test_store_dynamic_dimensions.py -v`
Expected: PASS.

**Step 5: Commit**

Skip commit in this session unless explicitly requested by the user.

### Task 3: Wire the CLI and MCP paths to use metadata-aware store initialization

**Files:**
- Modify: `ck-mlx/ck_mlx/cli.py`
- Modify: `ck-mlx/ck_mlx/server.py`
- Modify: `ck-mlx/ck_mlx/search.py`
- Test: `ck-mlx/tests/test_cli_status_metadata.py`

**Step 1: Write the failing test**

Create a test that verifies `status` exposes the model name and vector dimension, and that a mismatched model/dimension produces a clear error.

**Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_cli_status_metadata.py -v`
Expected: FAIL because status does not expose metadata and mismatch checks do not exist.

**Step 3: Write minimal implementation**

Update CLI/server/search initialization so they:
- discover embedding metadata during index creation
- open existing indexes in read mode using persisted metadata
- present model/dimension in `status` / `index_status`
- fail clearly on mismatches instead of silently using the wrong store

**Step 4: Run test to verify it passes**

Run: `uv run pytest tests/test_cli_status_metadata.py -v`
Expected: PASS.

**Step 5: Commit**

Skip commit in this session unless explicitly requested by the user.

### Task 4: Clean up project docs and smoke-validate the tool

**Files:**
- Modify: `ck-mlx/README.md`
- Modify: `ck-mlx/main.py`
- Test: `ck-mlx/tests/test_walk_ignore.py` (optional if walk behavior is touched)

**Step 1: Write the failing test**

Only if behavior changes. Otherwise skip to smoke validation.

**Step 2: Run targeted validation**

Run:
- `uv run python -m compileall ck_mlx`
- `uv run ck-mlx --help`
- `uv run ck-mlx status` (against an existing index if present)

Expected: commands succeed and status shows metadata.

**Step 3: Write minimal implementation**

Update docs and entrypoint notes so the supported invocation path is clear and aligned with `pyproject.toml`.

**Step 4: Run smoke validation again**

Expected: PASS.

**Step 5: Commit**

Skip commit in this session unless explicitly requested by the user.
