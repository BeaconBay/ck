import os
from pathlib import Path
from typing import List, Tuple

import typer

from ck_mlx.chunk import Chunk, Chunker
from ck_mlx.embed import embed_texts, get_embedding_metadata
from ck_mlx.store import Store, compute_hash
from ck_mlx.walk import walk_files

PendingFile = Tuple[str, float, str, List[Chunk]]
IndexCandidate = Tuple[Path, str, float, str, str]


def index_command(path: str, force: bool) -> None:
    root_path = Path(path).resolve()
    if not root_path.exists():
        typer.echo(f"Error: Path {root_path} does not exist.", err=True)
        raise typer.Exit(1)
    store = _open_store(root_path)
    try:
        _index_files(root_path, store, force)
    finally:
        store.close()


def _open_store(root_path: Path) -> Store:
    try:
        embedding_meta = get_embedding_metadata()
        typer.echo(
            f"Embedding model: {embedding_meta['model']} ({embedding_meta['dimension']} dims)"
        )
        typer.echo(f"Initializing store at {root_path}")
        return Store(
            root_path,
            embedding_dimension=embedding_meta["dimension"],
            embedding_model=embedding_meta["model"],
        )
    except (RuntimeError, OSError, ValueError) as e:
        typer.echo(f"Error initializing index store: {e}", err=True)
        raise typer.Exit(1) from e


def _index_files(root_path: Path, store: Store, force: bool) -> None:
    typer.echo("Walking files...")
    walked_files = list(walk_files(str(root_path)))
    walked_rel_paths = {str(p.relative_to(root_path)) for p in walked_files}
    for rel_path in store.get_all_indexed_paths() - walked_rel_paths:
        store.delete_file(rel_path)
    candidates, skipped_count = _collect_candidates(root_path, store, walked_files, force)
    if not candidates:
        typer.echo(f"No changes detected. Skipped {skipped_count} files.")
        typer.echo(f"Total indexed: {len(walked_rel_paths)} files.")
        return
    typer.echo(f"Indexing {len(candidates)} files (skipped {skipped_count} unchanged)...")
    indexed_count = _embed_and_store(candidates, store)
    typer.echo(f"Successfully indexed {indexed_count} files.")
    status_info = store.get_status()
    typer.echo(
        f"Database status: {status_info['total_files']} files, {status_info['total_chunks']} chunks."
    )


def _collect_candidates(
    root_path: Path, store: Store, walked_files: List[Path], force: bool
) -> Tuple[List[IndexCandidate], int]:
    candidates: List[IndexCandidate] = []
    skipped_count = 0
    for file_path in walked_files:
        rel_path = str(file_path.relative_to(root_path))
        try:
            mtime = os.path.getmtime(file_path)
            content = file_path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        content_hash = compute_hash(content)
        db_info = store.get_file_info(rel_path)
        if not force and db_info and db_info == (mtime, content_hash):
            skipped_count += 1
            continue
        candidates.append((file_path, rel_path, mtime, content_hash, content))
    return candidates, skipped_count


def _embed_and_store(candidates: List[IndexCandidate], store: Store) -> int:
    chunker = Chunker()
    indexed_count = 0
    pending_files: List[PendingFile] = []
    for idx, (file_path, rel_path, mtime, content_hash, _content) in enumerate(candidates):
        pending_files.append((rel_path, mtime, content_hash, chunker.chunk_file(file_path)))
        if len(pending_files) >= 10 or idx + 1 == len(candidates):
            indexed_count += _flush_pending(pending_files, store)
            pending_files = []
        if (idx + 1) % 10 == 0 or idx + 1 == len(candidates):
            typer.echo(f"Progress: {idx + 1}/{len(candidates)} files processed...")
    return indexed_count


def _flush_pending(pending_files: List[PendingFile], store: Store) -> int:
    all_texts: List[str] = []
    for _, _, _, chunks_list in pending_files:
        all_texts.extend([chunk.content for chunk in chunks_list])
    all_embeddings = embed_texts(all_texts, input_type="document") if all_texts else []
    emb_idx = 0
    for rel_path, mtime, content_hash, chunks in pending_files:
        embeddings = all_embeddings[emb_idx : emb_idx + len(chunks)]
        emb_idx += len(chunks)
        store.update_file(rel_path, mtime, content_hash, chunks, embeddings)
    return len(pending_files)
