import os
from dataclasses import dataclass
from functools import lru_cache
from typing import Any, Dict, List, Optional, Protocol, runtime_checkable

from openai import OpenAI


@dataclass(frozen=True)
class EmbeddingConfig:
    base_url: str
    api_key: str
    model: str
    max_concurrent_requests: int
    batch_size: int


@runtime_checkable
class EmbeddingProvider(Protocol):
    def embed(
        self, texts: List[str], input_type: Optional[str] = None
    ) -> List[List[float]]: ...

    def dimension(self) -> int: ...

    def model_name(self) -> str: ...


class APIEmbeddingProvider:
    def __init__(self, config: Optional[EmbeddingConfig] = None):
        self.config = config or get_embedding_config()

    def embed(
        self, texts: List[str], input_type: Optional[str] = None
    ) -> List[List[float]]:
        if not texts:
            return []
        client = _build_sync_client(self.config.base_url, self.config.api_key)
        vectors: List[List[float]] = []
        for batch in _batches(texts, self.config.batch_size):
            response = client.embeddings.create(
                model=self.config.model,
                input=batch,
                extra_body=_build_extra_body(input_type),
            )
            sorted_data = sorted(response.data, key=lambda item: item.index)
            vectors.extend([item.embedding for item in sorted_data])
        return vectors

    def dimension(self) -> int:
        metadata = _discover_embedding_metadata(
            self.config.base_url, self.config.api_key, self.config.model
        )
        return int(metadata["dimension"])

    def model_name(self) -> str:
        return self.config.model


class LocalMLXEmbeddingProvider:
    DEFAULT_MODEL = "mlx-community/bge-small-en-v1.5-6bit"

    def __init__(self, model_id: Optional[str] = None):
        self._model_id = model_id or os.environ.get("CK_LOCAL_MODEL", self.DEFAULT_MODEL)
        self._model: Any | None = None
        self._tokenizer: Any | None = None
        self._dim: int | None = None

    def _load(self) -> None:
        if self._model is None:
            from mlx_embeddings import load

            self._model, self._tokenizer = load(self._model_id)

    def embed(
        self, texts: List[str], input_type: Optional[str] = None
    ) -> List[List[float]]:
        if not texts:
            return []
        self._load()
        if hasattr(self._model, "process"):
            return _to_vectors(self._model.process([{"text": text} for text in texts], processor=self._tokenizer))
        try:
            from mlx_embeddings import generate

            output = generate(self._model, self._tokenizer, texts=texts)
        except (ImportError, TypeError, AttributeError):
            tokens = self._tokenizer(
                texts, return_tensors="mlx", padding=True, truncation=True
            )
            output = self._model(**tokens)
        vectors = _extract_embedding_vectors(output)
        if vectors:
            self._dim = len(vectors[0])
        return vectors

    def dimension(self) -> int:
        if self._dim is None:
            self._dim = len(self.embed(["ck-search dimension probe"])[0])
        return self._dim

    def model_name(self) -> str:
        return self._model_id


@lru_cache(maxsize=16)
def _build_sync_client(base_url: str, api_key: str) -> OpenAI:
    return OpenAI(base_url=base_url, api_key=api_key)


def get_embedding_config() -> EmbeddingConfig:
    return EmbeddingConfig(
        base_url=os.environ.get("OMLX_BASE_URL", "http://127.0.0.1:8000/v1"),
        api_key=os.environ.get("OMLX_API_KEY", "omlx-local"),
        model=os.environ.get("OMLX_MODEL", "zembed-1-embedding-mlx-6Bit"),
        max_concurrent_requests=max(
            1, int(os.environ.get("OMLX_MAX_CONCURRENT_REQUESTS", "8"))
        ),
        batch_size=max(1, int(os.environ.get("OMLX_BATCH_SIZE", "1"))),
    )


def get_sync_client(config: Optional[EmbeddingConfig] = None) -> OpenAI:
    config = config or get_embedding_config()
    return _build_sync_client(config.base_url, config.api_key)


def get_provider() -> EmbeddingProvider:
    backend = os.environ.get(
        "CK_BACKEND", "api" if os.environ.get("OMLX_API_KEY") else "local"
    )
    if backend == "local":
        return LocalMLXEmbeddingProvider()
    if backend == "api":
        return APIEmbeddingProvider()
    raise ValueError("CK_BACKEND must be 'api' or 'local'.")


def _build_extra_body(input_type: Optional[str]) -> Optional[Dict[str, Any]]:
    if not input_type:
        return None
    return {"input_type": input_type}


def _batches(texts: List[str], size: int) -> List[List[str]]:
    return [texts[i : i + size] for i in range(0, len(texts), size)]


@lru_cache(maxsize=32)
def _discover_embedding_metadata(
    base_url: str, api_key: str, model: str
) -> Dict[str, Any]:
    provider = APIEmbeddingProvider(
        EmbeddingConfig(base_url, api_key, model, max_concurrent_requests=1, batch_size=1)
    )
    vectors = provider.embed(["ck-search dimension probe"], input_type="document")
    if not vectors:
        raise RuntimeError("Embedding probe returned no vectors.")
    dimension = len(vectors[0])
    if dimension <= 0:
        raise RuntimeError("Embedding probe returned an empty vector.")
    return {"model": model, "dimension": dimension}


def clear_embedding_caches() -> None:
    _build_sync_client.cache_clear()
    _discover_embedding_metadata.cache_clear()


def get_embedding_metadata() -> Dict[str, Any]:
    provider = get_provider()
    return {"model": provider.model_name(), "dimension": provider.dimension()}


def ensure_embedding_compatible(
    *,
    index_model: Optional[str],
    index_dimension: Optional[int],
    probe_dimension: bool = False,
) -> Dict[str, Any]:
    provider = get_provider()
    model = provider.model_name()
    if index_model and model != index_model:
        raise ValueError(
            f"Configured embedding model '{model}' does not match indexed model '{index_model}'. "
            "Reindex with the new model or switch the active backend/model back to the indexed model."
        )
    runtime_dimension = provider.dimension() if probe_dimension else index_dimension
    if index_dimension is not None and runtime_dimension != index_dimension:
        raise ValueError(
            f"Configured embedding dimension {runtime_dimension} does not match indexed dimension {index_dimension}. "
            "Reindex with the current model before running semantic search."
        )
    return {"model": model, "dimension": runtime_dimension}


def embed_texts(
    texts: List[str], input_type: Optional[str] = None
) -> List[List[float]]:
    return get_provider().embed(texts, input_type=input_type)


def _extract_embedding_vectors(output: Any) -> List[List[float]]:
    if hasattr(output, "text_embeds"):
        return _to_vectors(output.text_embeds)
    if hasattr(output, "last_hidden_state"):
        import mlx.core as mx

        vectors = mx.mean(output.last_hidden_state, axis=1)
        mx.eval(vectors)
        return _to_vectors(vectors)
    return _to_vectors(output)


def _to_vectors(value: Any) -> List[List[float]]:
    raw = value.tolist() if hasattr(value, "tolist") else value
    return [[float(item) for item in row] for row in raw]
