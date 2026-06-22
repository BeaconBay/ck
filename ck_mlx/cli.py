import os
from pathlib import Path
from typing import Optional

import typer

from ck_mlx.cli_index import index_command
from ck_mlx.cli_query import search_command, status_command

INDEX_DIR_NAME = ".ck"

app = typer.Typer(help="ck-search — local code search with API or MLX backends")


@app.callback()
def configure_backend(
    backend: Optional[str] = typer.Option(
        None,
        "--backend",
        help="Embedding backend: api or local (default: auto-detect)",
    ),
) -> None:
    """Configure process-wide backend selection before a command runs."""
    if backend is None:
        return
    if backend not in {"api", "local"}:
        raise typer.BadParameter("backend must be 'api' or 'local'")
    os.environ["CK_BACKEND"] = backend


def find_index_root(start_path: Path) -> Path:
    curr = start_path.resolve()
    for parent in [curr] + list(curr.parents):
        if (parent / INDEX_DIR_NAME).exists():
            return parent
    return curr


@app.command("index")
def index(
    path: str = typer.Argument(".", help="Directory path to index"),
    force: bool = typer.Option(
        False, "--force", "-f", help="Force reindexing of all files"
    ),
) -> None:
    """Index files in the specified directory."""
    index_command(path, force)


@app.command("search")
def search(
    query: str = typer.Argument(..., help="Search query string"),
    mode: str = typer.Option(
        "hybrid", "--mode", "-m", help="Search mode (hybrid, semantic, lexical)"
    ),
    limit: int = typer.Option(10, "--limit", "-l", help="Number of results to return"),
    rerank: bool = typer.Option(
        False, "--rerank", "-r", help="Rerank results using the configured reranker"
    ),
    top_n: int = typer.Option(10, "--top-n", help="Number of top results to rerank"),
) -> None:
    """Search for relevant code chunks."""
    search_command(query, mode, limit, rerank, top_n)


@app.command("status")
def status_cmd() -> None:
    """Display index database status."""
    status_command()


if __name__ == "__main__":
    app()
