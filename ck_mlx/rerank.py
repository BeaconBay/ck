import json
import os
import urllib.error
import urllib.request
from typing import Any, List, Protocol, runtime_checkable

from ck_mlx.embed import get_embedding_config


@runtime_checkable
class RerankerProvider(Protocol):
    def rerank(self, query: str, docs: List[str]) -> List[float]: ...

    def model_name(self) -> str: ...

    def is_available(self) -> bool: ...


class APIRerankerProvider:
    def __init__(self) -> None:
        self._model = os.environ.get("OMLX_RERANK_MODEL", "zerank-2-reranker-oQ6")
        config = get_embedding_config()
        self._base_url = config.base_url
        self._api_key = config.api_key

    def rerank(self, query: str, docs: List[str]) -> List[float]:
        if not docs:
            return []
        request = urllib.request.Request(
            f"{self._base_url}/rerank",
            headers={
                "Content-Type": "application/json",
                "Authorization": f"Bearer {self._api_key}",
            },
            data=json.dumps(
                {
                    "model": self._model,
                    "query": query,
                    "documents": docs,
                    "top_n": len(docs),
                }
            ).encode("utf-8"),
            method="POST",
        )
        with urllib.request.urlopen(request, timeout=15) as response:
            data = json.loads(response.read().decode())
        scores = [0.0] * len(docs)
        for item in data.get("results", []):
            scores[int(item["index"])] = float(item["relevance_score"])
        return scores

    def model_name(self) -> str:
        return self._model

    def is_available(self) -> bool:
        return True


class LocalMLXRerankerProvider:
    DEFAULT_MODEL = "mlx-community/jina-reranker-v3-4bit-mxfp4"

    def __init__(self, model_id: str | None = None) -> None:
        self._model_id = model_id or os.environ.get(
            "CK_LOCAL_RERANK_MODEL", self.DEFAULT_MODEL
        )
        self._model: Any | None = None
        self._tokenizer: Any | None = None

    def _load(self) -> None:
        if self._model is None:
            from mlx_embeddings import load

            self._model, self._tokenizer = load(self._model_id)

    def rerank(self, query: str, docs: List[str]) -> List[float]:
        if not docs:
            return []
        self._load()
        if hasattr(self._model, "process"):
            output = self._model.process(
                {
                    "query": {"text": query},
                    "documents": [{"text": doc} for doc in docs],
                },
                processor=self._tokenizer,
            )
            return _scores_from_output(output)
        pairs = [[query, doc] for doc in docs]
        tokens = self._tokenizer(
            pairs, return_tensors="mlx", padding=True, truncation=True
        )
        output = self._model(**tokens)
        return _scores_from_output(output)

    def model_name(self) -> str:
        return self._model_id

    def is_available(self) -> bool:
        try:
            import mlx_embeddings  # noqa: F401
        except ImportError:
            return False
        return True


def get_reranker() -> RerankerProvider:
    backend = os.environ.get(
        "CK_BACKEND", "api" if os.environ.get("OMLX_API_KEY") else "local"
    )
    if backend == "local":
        return LocalMLXRerankerProvider()
    if backend == "api":
        return APIRerankerProvider()
    raise ValueError("CK_BACKEND must be 'api' or 'local'.")


def _scores_from_output(output: Any) -> List[float]:
    logits = output.logits if hasattr(output, "logits") else output
    raw = logits.tolist() if hasattr(logits, "tolist") else logits
    if raw and isinstance(raw[0], list):
        return [float(row[0]) for row in raw]
    return [float(score) for score in raw]
