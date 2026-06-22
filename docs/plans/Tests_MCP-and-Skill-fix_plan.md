# Tests + MCP & Skill fix plan

Live verification results

### Reindex
Ran:

- `OMLX_API_KEY='…' uv run ck-mlx index . --force`

Result:
- indexed `15 files`
- wrote `65 chunks`
- completed successfully against live oMLX

### Model probe
Ran:

- `OMLX_API_KEY='…' uv run python -c "from ck_mlx.embed import get_embedding_metadata; print(get_embedding_metadata())"`

Result:
- `{'model': 'zembed-1-embedding-mlx-6Bit', 'dimension': 2560}`

### Persisted index metadata
Ran:

- `OMLX_API_KEY='…' uv run ck-mlx status`

Result:
- `Embedding Model: zembed-1-embedding-mlx-6Bit`
- `Embedding Dimension: 2560`

So the rebuilt index is now metadata-aware, not legacy `unknown`.

### Live semantic search smoke test
Ran:

- `OMLX_API_KEY='…' uv run ck-mlx search "embedding model metadata" --mode semantic --limit 5`

Result:
- semantic search returned relevant hits from:
  - `ck_mlx/embed.py`
  - `ck_mlx/store.py`
  - `ck_mlx/cli.py`

## Current verified index state

From `ck-mlx status`:

- `Total Files: 15`
- `Total Chunks: 65`
- `Embedding Model: zembed-1-embedding-mlx-6Bit`
- `Embedding Dimension: 2560`

## Notes

- I used the API key only in the shell commands for verification; I did **not** reintroduce it into the code.
- The old legacy index metadata issue is now resolved for this repo because the index was rebuilt successfully.

next steps:
1. update claude code, antigravity CLI, opencode, and codex CLI MCP configs to point at `ck-mlx`
2. run one hybrid search + rerank smoke test too
3. update claude code, codex, antigravity CLI, and opencode skills instructing on usage of ck to reference ck-mlx
