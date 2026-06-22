# ck-mlx: Dual-Backend MLX Embedding + Reranking

> **For Codex:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make `ck-mlx` publishable as a self-contained package by adding a `local` backend that runs MLX embedding and reranking models directly (no oMLX server required), while keeping the existing `api` backend intact. Matches what `ck` provides via FastEmbed/ONNX.

**Architecture overview:**
```
EmbeddingProvider (Protocol)      RerankerProvider (Protocol)
├─ APIEmbeddingProvider             ├─ APIRerankerProvider
└─ LocalMLXEmbeddingProvider        └─ LocalMLXRerankerProvider

get_provider() → EmbeddingProvider   get_reranker() → RerankerProvider
  CK_BACKEND=api  → API               CK_BACKEND=api  → API
  CK_BACKEND=local → Local MLX        CK_BACKEND=local → Local MLX
  default: api if OMLX_API_KEY set, else local
```

**Tech stack additions:** `mlx-embeddings`, `huggingface_hub` (optional deps under `[local]` extra).

**Bundled local models (auto-download from HF Hub on first use):**
- Embedder default: `mlx-community/bge-small-en-v1.5-mlx` (384 dims, ~33 MB, Metal-accelerated)
- Reranker default: `mlx-community/ms-marco-MiniLM-L-6-v2-mlx` (~23 MB)
- Configurable via `CK_LOCAL_MODEL` and `CK_LOCAL_RERANK_MODEL` env vars

---

### Task 1: Add optional dependencies and verify mlx-embeddings API

**Files:**
- Modify: `pyproject.toml`

**Step 1: Add optional deps**

Add to `pyproject.toml`:
```toml
[project.optional-dependencies]
local = [
    "mlx-embeddings>=0.1.0",
    "huggingface_hub>=0.24.0",
]
```

Run: `uv sync --extra local`

**Step 2: Verify mlx-embeddings API**

Run a quick probe to confirm the API surface:
```bash
uv run python -c "
from mlx_embeddings import load
model, tokenizer = load('mlx-community/bge-small-en-v1.5-mlx')
print('load API: ok')
import mlx.core as mx
tokens = tokenizer(['hello world'], return_tensors='mlx', padding=True, truncation=True)
output = model(**tokens)
print('forward pass shape:', output.shape)
"
```

If the API differs (e.g., different import path or different forward call), adjust Task 3 accordingly before proceeding.

---

### Task 2: Extract reranker logic from search.py into rerank.py

**Files:**
- Create: `ck_mlx/rerank.py`
- Modify: `ck_mlx/search.py`
- Test: `tests/test_rerank_provider.py`

**Step 1: Write the failing test**

Create `tests/test_rerank_provider.py` that:
- Imports `RerankerProvider`, `APIRerankerProvider`, `get_reranker`
- Verifies `APIRerankerProvider` is returned when `CK_BACKEND=api`
- Verifies the provider has a `.rerank(query, docs) -> list[float]` method

Run: `uv run pytest tests/test_rerank_provider.py -v`
Expected: FAIL (rerank.py does not exist).

**Step 2: Write minimal implementation**

Create `ck_mlx/rerank.py`:
```python
from typing import Protocol, runtime_checkable

@runtime_checkable
class RerankerProvider(Protocol):
    def rerank(self, query: str, docs: list[str]) -> list[float]: ...
    def model_name(self) -> str: ...
    def is_available(self) -> bool: ...

class APIRerankerProvider:
    # Move existing rerank logic from search.py here
    # Reads: OMLX_BASE_URL, OMLX_API_KEY, OMLX_RERANK_MODEL
    ...

class LocalMLXRerankerProvider:
    # Stub returning uniform scores for now (implemented in Task 5)
    def rerank(self, query: str, docs: list[str]) -> list[float]:
        return [1.0] * len(docs)
    def is_available(self) -> bool:
        return False  # until Task 5

def get_reranker() -> RerankerProvider:
    import os
    backend = os.environ.get('CK_BACKEND', 'api' if os.environ.get('OMLX_API_KEY') else 'local')
    if backend == 'local':
        return LocalMLXRerankerProvider()
    return APIRerankerProvider()
```

Update `search.py` to import from `rerank.py` instead of inline implementation.

**Step 3: Run test to verify it passes**

Run: `uv run pytest tests/test_rerank_provider.py -v`
Expected: PASS.

---

### Task 3: Add EmbeddingProvider protocol and APIEmbeddingProvider

**Files:**
- Modify: `ck_mlx/embed.py`
- Test: `tests/test_embed_provider.py`

**Step 1: Write the failing test**

Create `tests/test_embed_provider.py` that:
- Imports `EmbeddingProvider`, `APIEmbeddingProvider`, `get_provider`
- Verifies `APIEmbeddingProvider` is returned when `CK_BACKEND=api`
- Stubs the openai client and verifies `.embed(["hello"])` returns a list of floats
- Verifies `.model_name()` returns the configured model

Run: `uv run pytest tests/test_embed_provider.py -v`
Expected: FAIL (no protocol or renamed class yet).

**Step 2: Write minimal implementation**

In `embed.py`, add:
```python
from typing import Protocol, runtime_checkable

@runtime_checkable
class EmbeddingProvider(Protocol):
    def embed(self, texts: list[str], input_type: str | None = None) -> list[list[float]]: ...
    def dimension(self) -> int: ...
    def model_name(self) -> str: ...

class APIEmbeddingProvider:
    # Rename/wrap current embed_texts() and get_embedding_metadata() here
    ...

class LocalMLXEmbeddingProvider:
    # Stub returning zero vectors for now (implemented in Task 4)
    def embed(self, texts, input_type=None):
        raise NotImplementedError('mlx-embeddings not yet wired')

def get_provider() -> EmbeddingProvider:
    import os
    backend = os.environ.get('CK_BACKEND', 'api' if os.environ.get('OMLX_API_KEY') else 'local')
    if backend == 'local':
        return LocalMLXEmbeddingProvider()
    return APIEmbeddingProvider()
```

Keep existing top-level `embed_texts()` and `get_embedding_metadata()` as thin shims calling `get_provider()` for backward compat with `cli.py` and `search.py`.

**Step 3: Run test to verify it passes**

Run: `uv run pytest tests/test_embed_provider.py -v`
Expected: PASS.

---

### Task 4: Implement LocalMLXEmbeddingProvider

**Files:**
- Modify: `ck_mlx/embed.py`
- Test: `tests/test_local_embed_provider.py`

**Step 1: Write the failing test**

Create `tests/test_local_embed_provider.py` that:
- Creates a `LocalMLXEmbeddingProvider` with a mock/patched `mlx_embeddings.load`
- Verifies `.embed(["hello world"])` returns a list containing one vector
- Verifies `.dimension()` returns the vector length
- Verifies `CK_LOCAL_MODEL` env var overrides the default model

Run: `uv run pytest tests/test_local_embed_provider.py -v`
Expected: FAIL (stub raises NotImplementedError).

**Step 2: Write minimal implementation**

In `LocalMLXEmbeddingProvider.__init__`:
```python
import os
from mlx_embeddings import load
import mlx.core as mx

class LocalMLXEmbeddingProvider:
    DEFAULT_MODEL = 'mlx-community/bge-small-en-v1.5-mlx'
    
    def __init__(self):
        model_id = os.environ.get('CK_LOCAL_MODEL', self.DEFAULT_MODEL)
        self._model_id = model_id
        self._model = None
        self._tokenizer = None
        self._dim: int | None = None
    
    def _load(self):
        if self._model is None:
            self._model, self._tokenizer = load(self._model_id)
    
    def embed(self, texts, input_type=None):
        self._load()
        tokens = self._tokenizer(texts, return_tensors='mlx', padding=True, truncation=True)
        output = self._model(**tokens)
        # Most MLX embedding models return last_hidden_state; mean pool over sequence dim
        vecs = mx.mean(output.last_hidden_state, axis=1)
        mx.eval(vecs)
        return vecs.tolist()
    
    def dimension(self):
        if self._dim is None:
            self._dim = len(self.embed(['dim probe'])[0])
        return self._dim
    
    def model_name(self):
        return self._model_id
```

> NOTE: `mlx-embeddings` output format may differ (e.g., pooled output directly, or different attribute name). Adjust the pooling line based on the probe from Task 1.

**Step 3: Run test to verify it passes**

Run: `uv run pytest tests/test_local_embed_provider.py -v`
Expected: PASS.

**Step 4: Integration smoke test**

```bash
CK_BACKEND=local uv run ck-mlx index ~/code/ck-mlx --force
```
Expected: indexes files, downloads bge-small on first run, stays low CPU.

---

### Task 5: Implement LocalMLXRerankerProvider

**Files:**
- Modify: `ck_mlx/rerank.py`
- Test: `tests/test_local_reranker.py`

**Step 1: Write the failing test**

Create `tests/test_local_reranker.py` that:
- Creates a `LocalMLXRerankerProvider` with mock/patched mlx load
- Verifies `.rerank(query, docs)` returns a list of floats of len(docs)
- Verifies `.is_available()` returns True when mlx-embeddings is importable

Run: `uv run pytest tests/test_local_reranker.py -v`
Expected: FAIL (`is_available` returns False, stub scores).

**Step 2: Write minimal implementation**

The reranker is a cross-encoder: score = f([query, doc]) for each doc.
```python
class LocalMLXRerankerProvider:
    DEFAULT_MODEL = 'mlx-community/ms-marco-MiniLM-L-6-v2-mlx'
    
    def __init__(self):
        model_id = os.environ.get('CK_LOCAL_RERANK_MODEL', self.DEFAULT_MODEL)
        self._model_id = model_id
        self._model = None
        self._tokenizer = None
    
    def _load(self):
        if self._model is None:
            from mlx_embeddings import load
            self._model, self._tokenizer = load(self._model_id)
    
    def rerank(self, query, docs):
        self._load()
        pairs = [[query, doc] for doc in docs]
        tokens = self._tokenizer(pairs, return_tensors='mlx', padding=True, truncation=True)
        import mlx.core as mx
        logits = self._model(**tokens).logits
        mx.eval(logits)
        scores = logits[:, 0].tolist() if logits.ndim == 2 else logits.tolist()
        return scores
    
    def is_available(self):
        try:
            import mlx_embeddings  # noqa
            return True
        except ImportError:
            return False
    
    def model_name(self):
        return self._model_id
```

> NOTE: Cross-encoder output format varies. If the model outputs a single logit per pair, use `logits[:, 0]`. If it outputs a scalar, use directly. Probe output shape and adjust.

**Step 3: Run test to verify it passes**

Run: `uv run pytest tests/test_local_reranker.py -v`
Expected: PASS.

---

### Task 6: Abstract tokenizer in chunk.py for local backend

**Files:**
- Modify: `ck_mlx/chunk.py`
- Test: `tests/test_chunk_tokenizer_local.py`

**Step 1: Write the failing test**

Create `tests/test_chunk_tokenizer_local.py` that:
- Patches `CK_BACKEND=local` and `CK_LOCAL_MODEL=mlx-community/bge-small-en-v1.5-mlx`
- Creates a `Chunker` and verifies it loads a tokenizer successfully without needing `~/.omlx/`

Run: `uv run pytest tests/test_chunk_tokenizer_local.py -v`
Expected: FAIL (hardcoded omlx path, no fallback for local model).

**Step 2: Write minimal implementation**

Update `find_tokenizer_path()` to add a new branch for local backend:
```python
def find_tokenizer_path() -> str | None:
    # 1. Explicit env override (always wins)
    env_path = os.environ.get('OMLX_TOKENIZER_PATH') or os.environ.get('CK_TOKENIZER_PATH')
    if env_path and os.path.exists(env_path):
        return env_path
    
    # 2. Local MLX backend: resolve tokenizer from HF Hub cache
    backend = os.environ.get('CK_BACKEND', 'api' if os.environ.get('OMLX_API_KEY') else 'local')
    if backend == 'local':
        model_id = os.environ.get('CK_LOCAL_MODEL', LocalMLXEmbeddingProvider.DEFAULT_MODEL)
        try:
            from huggingface_hub import try_to_load_from_cache
            path = try_to_load_from_cache(model_id, 'tokenizer.json')
            if path:
                return str(path)
        except Exception:
            pass
        return None  # Chunker falls back to character-based splitting
    
    # 3. API backend: existing ~/.omlx path logic
    default_path = '/Users/matthewgold/.omlx/models/lexrivera/zembed-1-embedding-mlx-6Bit/tokenizer.json'
    if os.path.exists(default_path):
        return default_path
    ...
```

**Step 3: Run test to verify it passes**

Run: `uv run pytest tests/test_chunk_tokenizer_local.py -v`
Expected: PASS.

---

### Task 7: Wire --backend CLI option and update README

**Files:**
- Modify: `ck_mlx/cli.py`
- Modify: `README.md`
- Test: `tests/test_cli_backend_option.py`

**Step 1: Write the failing test**

Verify `ck-mlx --help` shows `--backend` option and that `CK_BACKEND` env var is documented.

**Step 2: Write minimal implementation**

In `cli.py`, add to `app` or to `index` and `search` commands:
```python
backend: str = typer.Option(None, '--backend', help='Embedding backend: api or local (default: auto-detect)')
```
If provided, set `os.environ['CK_BACKEND'] = backend` before calling `get_provider()`.

Update `README.md`:
- Section: "Local mode (self-contained, no server)" with `pip install ck-mlx[local]` and `ck-mlx index .`
- Section: "API mode (oMLX)" with the existing env var instructions
- Table of supported local models and their sizes

**Step 3: Smoke validate**

```bash
uv run ck-mlx --help                          # shows --backend
CK_BACKEND=local uv run ck-mlx status        # shows local backend + model name
CK_BACKEND=local uv run ck-mlx search "embedding provider" --mode hybrid --rerank
```

---

### Task 8: Full integration test (both backends)

**Files:**
- Test: `tests/test_integration_backends.py`

Create a test that:
1. Indexes `~/code/ck-mlx` with `CK_BACKEND=local` into a temp dir
2. Runs semantic search and verifies results are returned
3. If `OMLX_API_KEY` is set, runs the same with `CK_BACKEND=api` and compares result count

Run: `CK_BACKEND=local uv run pytest tests/test_integration_backends.py -v`
Expected: PASS.

---

## Environment Variables Reference

| Variable | Purpose | Default |
|----------|---------|--------|
| `CK_BACKEND` | `api` or `local` | `api` if `OMLX_API_KEY` set, else `local` |
| `CK_LOCAL_MODEL` | HF repo ID for local embedder | `mlx-community/bge-small-en-v1.5-mlx` |
| `CK_LOCAL_RERANK_MODEL` | HF repo ID for local reranker | `mlx-community/ms-marco-MiniLM-L-6-v2-mlx` |
| `CK_TOKENIZER_PATH` | Override tokenizer.json path | resolved from HF cache |
| `OMLX_BASE_URL` | API backend base URL | `http://127.0.0.1:8000/v1` |
| `OMLX_API_KEY` | API backend key | required for `api` mode |
| `OMLX_MODEL` | API backend embedding model | `zembed-1-embedding-mlx-6Bit` |
| `OMLX_RERANK_MODEL` | API backend rerank model | from env |

## Supported Local Models

| Alias | HF ID | Dims | Size |
|-------|-------|------|------|
| bge-small (default) | `mlx-community/bge-small-en-v1.5-mlx` | 384 | ~33 MB |
| nomic | `mlx-community/nomic-embed-text-v1.5-mlx` | 768 | ~274 MB |
| jina-code | `mlx-community/jina-embeddings-v2-base-code-mlx` | 768 | ~274 MB |
| bge-m3 | `mlx-community/bge-m3-mlx` | 1024 | ~570 MB |

Rerankers:
| Alias | HF ID | Size |
|-------|-------|------|
| ms-marco-small (default) | `mlx-community/ms-marco-MiniLM-L-6-v2-mlx` | ~23 MB |
| bge-reranker-v2-m3 | `mlx-community/bge-reranker-v2-m3-mlx` | ~570 MB |
