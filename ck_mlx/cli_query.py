import os
from pathlib import Path
from typing import Any, Dict, List

import typer

from ck_mlx.chunk import find_tokenizer_path
from ck_mlx.embed import ensure_embedding_compatible, get_provider
from ck_mlx.rerank import get_reranker
from ck_mlx.search import Searcher
from ck_mlx.store import Store

INDEX_DIR_NAME = ".ck"


def find_index_root(start_path: Path) -> Path:
    curr = start_path.resolve()
    for parent in [curr] + list(curr.parents):
        if (parent / INDEX_DIR_NAME).exists():
            return parent
    return curr


def search_command(
    query: str, mode: str, limit: int, rerank: bool, top_n: int
) -> None:
    root_path = find_index_root(Path(".").resolve())
    db_path = root_path / INDEX_DIR_NAME / "index.db"
    if not db_path.exists():
        typer.echo("Error: No index found. Run 'ck index' first.", err=True)
        raise typer.Exit(1)
    mode = mode.lower()
    if mode not in {"hybrid", "semantic", "lexical"}:
        typer.echo(f"Error: Unsupported search mode '{mode}'.", err=True)
        raise typer.Exit(1)
    store = Store(root_path)
    try:
        results = _run_search(store, query, mode, limit, rerank, top_n)
        _print_results(results)
    except (RuntimeError, OSError, ValueError) as e:
        typer.echo(f"Error running search: {e}", err=True)
        raise typer.Exit(1) from e
    finally:
        store.close()


def status_command() -> None:
    provider = get_provider()
    reranker = get_reranker()
    backend = os.environ.get(
        "CK_BACKEND", "api" if os.environ.get("OMLX_API_KEY") else "local"
    )
    typer.echo(f"Active Backend: {backend}")
    typer.echo(f"Embedding Model: {provider.model_name()}")
    typer.echo(f"Rerank Model: {reranker.model_name()}")
    root_path = find_index_root(Path(".").resolve())
    db_path = root_path / INDEX_DIR_NAME / "index.db"
    if not db_path.exists():
        typer.echo("No index database found.")
        return
    store = Store(root_path)
    try:
        info = store.get_status()
        typer.echo(f"Index Root: {root_path}")
        typer.echo(f"Total Files: {info['total_files']}")
        typer.echo(f"Total Chunks: {info['total_chunks']}")
        typer.echo(f"Indexed Embedding Model: {info['embedding_model'] or 'unknown'}")
        typer.echo(
            f"Indexed Embedding Dimension: {info['embedding_dimension'] or 'unknown'}"
        )
        typer.echo(f"Database Size: {info['db_size_bytes'] / 1024 / 1024:.2f} MB")
        typer.echo(f"Tokenizer: {find_tokenizer_path() or 'Fallback (None)'}")
    finally:
        store.close()


def _run_search(
    store: Store, query: str, mode: str, limit: int, rerank: bool, top_n: int
) -> List[Dict[str, Any]]:
    if mode != "lexical":
        ensure_embedding_compatible(
            index_model=store.embedding_model,
            index_dimension=store.embedding_dimension,
            probe_dimension=False,
        )
    searcher = Searcher(store)
    if mode == "semantic":
        results = searcher.semantic_search(query, limit=limit)
    elif mode == "lexical":
        results = searcher.lexical_search(query, limit=limit)
    else:
        results = searcher.hybrid_search(query, limit=limit)
    return searcher.rerank(query, results, top_n=top_n) if rerank else results


def _print_results(results: List[Dict[str, Any]]) -> None:
    if not results:
        typer.echo("No matches found.")
        return
    for idx, item in enumerate(results):
        typer.echo(
            f"\n[{idx + 1}] {item['path']}:{item['start_line']}-{item['end_line']} "
            f"(Score: {item['score']:.4f}, Method: {item['method']})"
        )
        typer.echo("-" * 80)
        lines = item["content"].splitlines()
        for line in lines[:10]:
            typer.echo(f"  {line}")
        if len(lines) > 10:
            typer.echo(f"  ... ({len(lines) - 10} more lines)")
