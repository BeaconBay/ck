# ck-mlx

`ck-mlx` is a local code search CLI and MCP server with two embedding backends:

- `local` runs MLX embedding and reranking models directly on Apple Silicon.
- `api` talks to an OpenAI-compatible oMLX server.

This fork replaces the original Rust workspace with the working Python dual-backend implementation and keeps the user-facing search flow simple:

```bash
uv run ck-mlx --backend local status
uv run ck-mlx --backend local index . --force
uv run ck-mlx --backend local search "embedding provider" --mode hybrid --rerank
```

The `ck-mlx` console script is installed.

## Install

```bash
uv sync --extra local --group dev
```

Package install:

```bash
pip install 'ck-mlx[local]'
```

## Backend selection

Backend selection is automatic:

- `api` when `OMLX_API_KEY` is set
- otherwise `local`

Override that behavior with either:

- `CK_BACKEND=api|local`
- `--backend api|local`

## Local mode

Local mode downloads Hugging Face MLX models on first use. No running server is required.

```bash
CK_BACKEND=local uv run ck-mlx index . --force
CK_BACKEND=local uv run ck-mlx search "rerank provider" --mode hybrid --rerank
```

Default local models:

| Role | Default |
| --- | --- |
| Embedding | `mlx-community/bge-small-en-v1.5-6bit` |
| Reranking | `mlx-community/jina-reranker-v3-4bit-mxfp4` |

Useful local embedding alternatives:

| Model | Dimensions |
| --- | --- |
| `mlx-community/bge-small-en-v1.5-6bit` | 384 |
| `mlx-community/nomic-embed-text-v1.5-mlx` | 768 |
| `mlx-community/jina-embeddings-v2-base-code-mlx` | 768 |
| `mlx-community/bge-m3-mlx` | 1024 |

## API mode

API mode preserves the oMLX path:

```bash
export CK_BACKEND=api
export OMLX_BASE_URL=http://127.0.0.1:8000/v1
export OMLX_API_KEY=omlx-local
export OMLX_MODEL=zembed-1-embedding-mlx-6Bit
uv run ck-mlx index . --force
```

## Commands

- `uv run ck-mlx index <path>` builds or refreshes the local index.
- `uv run ck-mlx search "query" --mode hybrid` searches the current index.
- `uv run ck-mlx status` reports the active backend, model selection, and index metadata.

Indexes are stored under `.ck/index.db`.

## Environment variables

| Variable | Purpose | Default |
| --- | --- | --- |
| `CK_BACKEND` | `api` or `local` | `api` if `OMLX_API_KEY` is set, else `local` |
| `CK_LOCAL_MODEL` | local embedding model | `mlx-community/bge-small-en-v1.5-6bit` |
| `CK_LOCAL_RERANK_MODEL` | local reranker model | `mlx-community/jina-reranker-v3-4bit-mxfp4` |
| `CK_TOKENIZER_PATH` | explicit tokenizer JSON path | auto-discovered in local mode |
| `OMLX_BASE_URL` | API backend base URL | `http://127.0.0.1:8000/v1` |
| `OMLX_API_KEY` | API backend key | unset |
| `OMLX_MODEL` | API embedding model | `zembed-1-embedding-mlx-6Bit` |
| `OMLX_RERANK_MODEL` | API rerank model | `zerank-2-reranker-oQ6` |

## Notes

- Vector width is discovered from the active embedding model when the index is created.
- Semantic and hybrid search fail fast if the active backend does not match the stored index metadata.
- `main.py` is a thin wrapper around `ck_mlx.cli:app`.
