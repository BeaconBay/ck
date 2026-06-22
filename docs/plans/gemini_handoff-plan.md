# Handoff Plan: ck-mlx Implementation & Verification

This handoff plan outlines the current status of the `ck-mlx` tool implementation, details about the active indexing task, and next steps for verification.

---

## 1. Project Status

All core modules have been fully implemented in [~/code/ck-mlx/ck_mlx/](file:///Users/matthewgold/code/ck-mlx/ck_mlx/):

- **[embed.py](file:///Users/matthewgold/code/ck-mlx/ck_mlx/embed.py):** Asynchronous parallel batch-1 oMLX API client wrapper.
- **[walk.py](file:///Users/matthewgold/code/ck-mlx/ck_mlx/walk.py):** Directory walker adhering to `.gitignore` and `.ckignore`.
- **[chunk.py](file:///Users/matthewgold/code/ck-mlx/ck_mlx/chunk.py):** Tree-sitter-based function-aware chunker with tokenizer-based fallback.
- **[store.py](file:///Users/matthewgold/code/ck-mlx/ck_mlx/store.py):** SQLite + `sqlite-vec` + FTS5 database wrapper.
- **[search.py](file:///Users/matthewgold/code/ck-mlx/ck_mlx/search.py):** Lexical, semantic, hybrid (RRF), and reranking search logic.
- **[cli.py](file:///Users/matthewgold/code/ck-mlx/ck_mlx/cli.py):** Typer CLI (`ck-mlx index`, `search`, `status`).
- **[server.py](file:///Users/matthewgold/code/ck-mlx/ck_mlx/server.py):** FastMCP server reproducing the exact tool names/signatures of the Rust `ck` server.

---

## 2. Active Indexing Status

A background indexing process is currently executing:
- **Command:** `PYTHONUNBUFFERED=1 ck-mlx index /Users/matthewgold/.hermes/hermes-agent`
- **Task ID:** `task-986` (PID `80615`)
- **Status:** Running (17 files, 451 chunks successfully indexed and written to SQLite so far).

### Key Finding on Performance
The local `omlx-server` takes **~6-7 seconds** to return embeddings for even small inputs. This is because the MLX backend compiles a new static computation graph for varying input sequence lengths (e.g., each chunk has a slightly different token count). As a result, indexing 4,302 files will take significant time.

> [!NOTE]
> The CPU usage of `omlx-server` remains low, indicating that compilation and execution are correctly happening on the Apple Silicon GPU/Neural Engine.

---

## 3. Next Steps (Handoff Tasks)

1. **Allow Indexing to Finish or Target a Smaller Directory:**
   - Either let `task-986` finish indexing the entire `hermes-agent` workspace.
   - Or kill `task-986` and run a test index on a smaller codebase (e.g., `ck-mlx` itself: `ck-mlx index ~/code/ck-mlx`).
2. **Swap the MCP Configurations:**
   - Open `~/.hermes/.claude/settings.json` and replace the Rust `ck-search` configuration with `ck-mlx`.
   - The command for the new MCP server is:
     ```json
     "mcpServers": {
       "ck-mlx": {
         "command": "/Users/matthewgold/code/ck-mlx/.venv/bin/mcp",
         "args": ["run", "/Users/matthewgold/code/ck-mlx/ck_mlx/server.py"]
       }
     }
     ```
3. **Verify Search Functionality:**
   - Run semantic or hybrid search queries using the CLI:
     ```bash
     ck-mlx search "retrieve embeddings" --mode hybrid --rerank
     ```
   - Verify compatibility of FastMCP tools when called by Claude Desktop or another agent.
