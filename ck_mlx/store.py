import hashlib
import re
import sqlite3
from dataclasses import dataclass
from pathlib import Path
from typing import List, Optional, Set, Tuple

import sqlite_vec


@dataclass(frozen=True)
class IndexMetadata:
    embedding_dimension: Optional[int]
    embedding_model: Optional[str]


class Store:
    INDEX_DIR_NAME = ".ck"

    def __init__(
        self,
        index_dir: Path,
        embedding_dimension: Optional[int] = None,
        embedding_model: Optional[str] = None,
    ):
        self.db_dir = index_dir / self.INDEX_DIR_NAME
        self.db_dir.mkdir(parents=True, exist_ok=True)
        self.db_path = self.db_dir / "index.db"

        self.conn = sqlite3.connect(str(self.db_path))
        self.conn.enable_load_extension(True)
        sqlite_vec.load(self.conn)

        self.index_metadata = self._init_db(
            embedding_dimension=embedding_dimension,
            embedding_model=embedding_model,
        )
        self.embedding_dimension = self.index_metadata.embedding_dimension
        self.embedding_model = self.index_metadata.embedding_model

    def _init_db(
        self,
        *,
        embedding_dimension: Optional[int],
        embedding_model: Optional[str],
    ) -> IndexMetadata:
        cursor = self.conn.cursor()

        cursor.execute(
            """
            CREATE TABLE IF NOT EXISTS index_metadata (
                key TEXT PRIMARY KEY,
                value TEXT
            )
            """
        )

        cursor.execute(
            """
            CREATE TABLE IF NOT EXISTS files (
                path TEXT PRIMARY KEY,
                mtime REAL,
                content_hash TEXT
            )
            """
        )

        cursor.execute(
            """
            CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT,
                start_line INTEGER,
                end_line INTEGER,
                content TEXT
            )
            """
        )

        existing_dimension = self._get_existing_vector_dimension(cursor)
        metadata = self._read_index_metadata(cursor)

        resolved_dimension = (
            metadata.embedding_dimension or existing_dimension or embedding_dimension
        )
        if existing_dimension is None:
            if resolved_dimension is None:
                raise ValueError(
                    "Embedding dimension is required when creating a new index. "
                    "Probe the active embedding model before opening the store."
                )
            self._create_vector_table(cursor, resolved_dimension)
            existing_dimension = resolved_dimension
        elif (
            embedding_dimension is not None
            and existing_dimension != embedding_dimension
        ):
            raise ValueError(
                f"Existing index dimension {existing_dimension} does not match requested dimension {embedding_dimension}."
            )

        resolved_dimension = resolved_dimension or existing_dimension
        if resolved_dimension is None:
            raise ValueError(
                "Unable to determine the vector dimension for the current index."
            )

        if metadata.embedding_dimension is None:
            self._set_metadata_value(
                cursor, "embedding_dimension", str(resolved_dimension)
            )

        if (
            metadata.embedding_model
            and embedding_model
            and metadata.embedding_model != embedding_model
        ):
            raise ValueError(
                f"Existing index model '{metadata.embedding_model}' does not match requested model '{embedding_model}'."
            )

        resolved_model = metadata.embedding_model or embedding_model
        if resolved_model and metadata.embedding_model is None:
            self._set_metadata_value(cursor, "embedding_model", resolved_model)

        cursor.execute(
            """
            CREATE VIRTUAL TABLE IF NOT EXISTS fts_chunks USING fts5(
                content,
                content='chunks',
                content_rowid='id'
            )
            """
        )

        self.conn.commit()
        return IndexMetadata(
            embedding_dimension=resolved_dimension,
            embedding_model=resolved_model,
        )

    def _create_vector_table(self, cursor: sqlite3.Cursor, dimension: int) -> None:
        if dimension <= 0:
            raise ValueError(f"Embedding dimension must be positive, got {dimension}.")
        cursor.execute(
            f"""
            CREATE VIRTUAL TABLE IF NOT EXISTS vec_chunks USING vec0(
                rowid INTEGER PRIMARY KEY,
                embedding float[{dimension}]
            )
            """
        )

    def _table_exists(self, cursor: sqlite3.Cursor, table_name: str) -> bool:
        cursor.execute(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ? LIMIT 1",
            (table_name,),
        )
        return cursor.fetchone() is not None

    def _get_existing_vector_dimension(self, cursor: sqlite3.Cursor) -> Optional[int]:
        if not self._table_exists(cursor, "vec_chunks"):
            return None

        cursor.execute(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'vec_chunks'"
        )
        row = cursor.fetchone()
        if not row or not row[0]:
            return None

        match = re.search(r"embedding\s+float\[(\d+)\]", row[0], flags=re.IGNORECASE)
        return int(match.group(1)) if match else None

    def _read_index_metadata(self, cursor: sqlite3.Cursor) -> IndexMetadata:
        cursor.execute("SELECT key, value FROM index_metadata")
        data = {key: value for key, value in cursor.fetchall()}
        embedding_dimension = data.get("embedding_dimension")
        return IndexMetadata(
            embedding_dimension=int(embedding_dimension)
            if embedding_dimension
            else None,
            embedding_model=data.get("embedding_model"),
        )

    def _set_metadata_value(self, cursor: sqlite3.Cursor, key: str, value: str) -> None:
        cursor.execute(
            "INSERT INTO index_metadata(key, value) VALUES(?, ?) "
            "ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            (key, value),
        )

    def get_index_metadata(self) -> IndexMetadata:
        return self.index_metadata

    def get_file_info(self, path: str) -> Optional[Tuple[float, str]]:
        """Get (mtime, content_hash) for the path if it exists, otherwise None."""
        cursor = self.conn.cursor()
        cursor.execute("SELECT mtime, content_hash FROM files WHERE path = ?", (path,))
        row = cursor.fetchone()
        return row if row else None

    def get_all_indexed_paths(self) -> Set[str]:
        """Get the set of all relative paths currently in the database."""
        cursor = self.conn.cursor()
        cursor.execute("SELECT path FROM files")
        return {row[0] for row in cursor.fetchall()}

    def delete_file(self, path: str):
        """Delete all database entries for the file path, including FTS and vectors."""
        cursor = self.conn.cursor()

        cursor.execute("SELECT id, content FROM chunks WHERE path = ?", (path,))
        stale_chunks = cursor.fetchall()

        for chunk_id, content in stale_chunks:
            cursor.execute(
                "INSERT INTO fts_chunks(fts_chunks, rowid, content) VALUES('delete', ?, ?)",
                (chunk_id, content),
            )
            cursor.execute("DELETE FROM vec_chunks WHERE rowid = ?", (chunk_id,))

        cursor.execute("DELETE FROM chunks WHERE path = ?", (path,))
        cursor.execute("DELETE FROM files WHERE path = ?", (path,))
        self.conn.commit()

    def update_file(
        self,
        path: str,
        mtime: float,
        content_hash: str,
        chunks: List,
        embeddings: List[List[float]],
    ):
        """Update file registry and insert chunks/embeddings in a transaction."""
        if len(chunks) != len(embeddings):
            raise ValueError(
                f"Chunk count ({len(chunks)}) does not match embedding count ({len(embeddings)})."
            )
        if self.embedding_dimension is None:
            raise RuntimeError("Store has no configured embedding dimension.")

        for embedding in embeddings:
            if len(embedding) != self.embedding_dimension:
                raise ValueError(
                    f"Embedding width {len(embedding)} does not match store dimension {self.embedding_dimension}."
                )

        self.conn.execute("BEGIN TRANSACTION")
        try:
            cursor = self.conn.cursor()
            cursor.execute("SELECT id, content FROM chunks WHERE path = ?", (path,))
            stale_chunks = cursor.fetchall()
            for chunk_id, content in stale_chunks:
                cursor.execute(
                    "INSERT INTO fts_chunks(fts_chunks, rowid, content) VALUES('delete', ?, ?)",
                    (chunk_id, content),
                )
                cursor.execute("DELETE FROM vec_chunks WHERE rowid = ?", (chunk_id,))
            cursor.execute("DELETE FROM chunks WHERE path = ?", (path,))
            cursor.execute("DELETE FROM files WHERE path = ?", (path,))

            cursor.execute(
                "INSERT INTO files(path, mtime, content_hash) VALUES(?, ?, ?)",
                (path, mtime, content_hash),
            )

            for chunk, embedding in zip(chunks, embeddings):
                cursor.execute(
                    "INSERT INTO chunks(path, start_line, end_line, content) VALUES(?, ?, ?, ?)",
                    (path, chunk.start_line, chunk.end_line, chunk.content),
                )
                chunk_id = cursor.lastrowid

                serialized = sqlite_vec.serialize_float32(embedding)
                cursor.execute(
                    "INSERT INTO vec_chunks(rowid, embedding) VALUES(?, ?)",
                    (chunk_id, serialized),
                )

                cursor.execute(
                    "INSERT INTO fts_chunks(rowid, content) VALUES(?, ?)",
                    (chunk_id, chunk.content),
                )

            self.conn.commit()
        except Exception as e:
            self.conn.rollback()
            raise e

    def get_status(self) -> dict:
        """Return dict of status info (total files, total chunks, metadata)."""
        cursor = self.conn.cursor()
        cursor.execute("SELECT count(*) FROM files")
        total_files = cursor.fetchone()[0]

        cursor.execute("SELECT count(*) FROM chunks")
        total_chunks = cursor.fetchone()[0]

        return {
            "total_files": total_files,
            "total_chunks": total_chunks,
            "db_size_bytes": self.db_path.stat().st_size
            if self.db_path.exists()
            else 0,
            "embedding_dimension": self.embedding_dimension,
            "embedding_model": self.embedding_model,
        }

    def close(self):
        """Close connection."""
        self.conn.close()


def compute_hash(text: str) -> str:
    """Compute the SHA-256 hash of a text string."""
    return hashlib.sha256(text.encode("utf-8", errors="replace")).hexdigest()


if __name__ == "__main__":
    import tempfile

    from ck_mlx.chunk import Chunk

    with tempfile.TemporaryDirectory() as tmpdir:
        tmp_path = Path(tmpdir)
        print(f"Testing Store in temporary directory: {tmp_path}")
        store = Store(tmp_path, embedding_dimension=8, embedding_model="debug-model")

        print("Initial status:", store.get_status())

        mock_chunks = [
            Chunk(content="def foo():\n    pass", start_line=1, end_line=2),
            Chunk(content="def bar():\n    return 42", start_line=4, end_line=5),
        ]
        mock_embs = [
            [0.1] * 8,
            [0.2] * 8,
        ]

        store.update_file("main.py", 12345.67, "hash_abc123", mock_chunks, mock_embs)
        print("Status after insertion:", store.get_status())

        info = store.get_file_info("main.py")
        print("File info for main.py:", info)

        cursor = store.conn.cursor()
        cursor.execute("SELECT rowid FROM fts_chunks WHERE fts_chunks MATCH 'return'")
        row = cursor.fetchone()
        print("FTS query match for 'return' rowid:", row)

        store.delete_file("main.py")
        print("Status after deletion:", store.get_status())

        store.close()
