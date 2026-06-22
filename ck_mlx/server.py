import os
import re
from pathlib import Path

from fastmcp import FastMCP

from ck_mlx.search import Searcher
from ck_mlx.store import Store
from ck_mlx.walk import walk_files

INDEX_DIR_NAME = ".ck"

mcp = FastMCP("ck-search")


def find_index_root(start_path: Path) -> Path:
    cwd = start_path.resolve()
    for parent in [cwd] + list(cwd.parents):
        if (parent / INDEX_DIR_NAME).exists():
            return parent
    return cwd


def get_searcher() -> Searcher:
    root = find_index_root(Path(os.getcwd()))
    store = Store(root)
    return Searcher(store)


@mcp.tool()
def semantic_search(query: str, threshold: float = 0.55, page_size: int = 15) -> str:
    """Run semantic search on the code index, returning formatted matches."""
    searcher = None
    try:
        searcher = get_searcher()
        results = searcher.semantic_search(query, limit=page_size)
        filtered = [r for r in results if r["score"] >= threshold]

        if not filtered:
            return "No matches found above threshold."

        output = []
        for idx, item in enumerate(filtered):
            output.append(
                f"[{idx + 1}] File: {item['path']} lines {item['start_line']}-{item['end_line']} (Score: {item['score']:.4f})\n"
                f"```\n{item['content']}\n```\n"
            )
        return "\n".join(output)
    except Exception as e:
        return f"Error executing semantic search: {e}"
    finally:
        if searcher is not None:
            searcher.store.close()


@mcp.tool()
def hybrid_search(query: str, page_size: int = 20, snippet_length: int = 400) -> str:
    """Run hybrid Reciprocal Rank Fusion search on the code index."""
    searcher = None
    try:
        searcher = get_searcher()
        results = searcher.hybrid_search(query, limit=page_size)

        if not results:
            return "No matches found."

        output = []
        for idx, item in enumerate(results):
            content = item["content"]
            if len(content) > snippet_length:
                content = (
                    content[:snippet_length]
                    + f"\n... (truncated, total {len(content)} chars)"
                )
            output.append(
                f"[{idx + 1}] File: {item['path']} lines {item['start_line']}-{item['end_line']} (RRF Score: {item['score']:.4f})\n"
                f"```\n{content}\n```\n"
            )
        return "\n".join(output)
    except Exception as e:
        return f"Error executing hybrid search: {e}"
    finally:
        if searcher is not None:
            searcher.store.close()


@mcp.tool()
def regex_search(query: str, page_size: int = 30) -> str:
    """Run a local regex match on the walked files in the workspace (no index needed)."""
    try:
        root = find_index_root(Path(os.getcwd()))

        try:
            pattern = re.compile(query)
        except re.error as e:
            return f"Invalid regex pattern: {e}"

        matches = []
        for file_path in walk_files(str(root)):
            try:
                rel_path = file_path.relative_to(root)
            except ValueError:
                rel_path = file_path

            try:
                with open(file_path, "r", encoding="utf-8", errors="replace") as f:
                    lines = f.readlines()
            except Exception:
                continue

            for line_idx, line in enumerate(lines):
                if pattern.search(line):
                    start = max(0, line_idx - 2)
                    end = min(len(lines), line_idx + 3)
                    context_snippet = "".join(lines[start:end])

                    matches.append(
                        {
                            "path": str(rel_path),
                            "line": line_idx + 1,
                            "content": context_snippet,
                        }
                    )
                    if len(matches) >= page_size:
                        break
            if len(matches) >= page_size:
                break

        if not matches:
            return "No regex matches found."

        output = []
        for idx, item in enumerate(matches):
            output.append(
                f"[{idx + 1}] File: {item['path']} line {item['line']}\n"
                f"```\n{item['content']}\n```\n"
            )
        return "\n".join(output)
    except Exception as e:
        return f"Error executing regex search: {e}"


@mcp.tool()
def index_status() -> str:
    """Get the current index status."""
    store = None
    try:
        root = find_index_root(Path(os.getcwd()))
        store = Store(root)
        status = store.get_status()
        return (
            f"Index Root: {root}\n"
            f"Total files indexed: {status['total_files']}\n"
            f"Total chunks: {status['total_chunks']}\n"
            f"Embedding model: {status['embedding_model'] or 'unknown'}\n"
            f"Embedding dimension: {status['embedding_dimension'] or 'unknown'}\n"
            f"Database size: {status['db_size_bytes'] / 1024 / 1024:.2f} MB"
        )
    except Exception as e:
        return f"Error getting index status: {e}"
    finally:
        if store is not None:
            store.close()


if __name__ == "__main__":
    mcp.run()
