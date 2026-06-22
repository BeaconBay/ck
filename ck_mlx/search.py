import math
import sqlite3
import urllib.error
from typing import Any, Dict, List

from ck_mlx.embed import embed_texts, ensure_embedding_compatible
from ck_mlx.rerank import get_reranker
from ck_mlx.store import Store


class Searcher:
    def __init__(self, store: Store):
        self.store = store

    def semantic_search(self, query: str, limit: int = 20) -> List[Dict[str, Any]]:
        """Run a semantic vector search query using cosine similarity equivalent distance."""
        ensure_embedding_compatible(
            index_model=self.store.embedding_model,
            index_dimension=self.store.embedding_dimension,
            probe_dimension=False,
        )

        query_embs = embed_texts([query], input_type="query")
        if not query_embs:
            return []
        query_emb = query_embs[0]

        norm = math.sqrt(sum(x * x for x in query_emb))
        if norm > 0:
            query_emb = [x / norm for x in query_emb]

        import sqlite_vec

        serialized = sqlite_vec.serialize_float32(query_emb)

        cursor = self.store.conn.cursor()
        cursor.execute(
            "SELECT rowid, distance FROM vec_chunks WHERE embedding MATCH ? AND k = ?",
            (serialized, limit),
        )
        vec_results = cursor.fetchall()

        results = []
        for chunk_id, distance in vec_results:
            if distance is None:
                continue
            similarity = 1.0 - (distance * distance / 2.0)

            cursor.execute(
                "SELECT path, start_line, end_line, content FROM chunks WHERE id = ?",
                (chunk_id,),
            )
            chunk_row = cursor.fetchone()
            if chunk_row:
                path, start_line, end_line, content = chunk_row
                results.append(
                    {
                        "id": chunk_id,
                        "path": path,
                        "start_line": start_line,
                        "end_line": end_line,
                        "content": content,
                        "score": similarity,
                        "method": "semantic",
                    }
                )
        return results

    def lexical_search(self, query: str, limit: int = 20) -> List[Dict[str, Any]]:
        """Run an FTS5 full-text keyword search query with BM25 ranking."""
        cursor = self.store.conn.cursor()

        try:
            cursor.execute(
                """
                SELECT rowid, bm25(fts_chunks)
                FROM fts_chunks
                WHERE fts_chunks MATCH ?
                LIMIT ?
                """,
                (query, limit),
            )
            fts_rows = cursor.fetchall()
        except sqlite3.OperationalError:
            clean_query = query.replace('"', "").replace("'", "")
            try:
                cursor.execute(
                    """
                    SELECT rowid, bm25(fts_chunks)
                    FROM fts_chunks
                    WHERE fts_chunks MATCH ?
                    LIMIT ?
                    """,
                    (clean_query, limit),
                )
                fts_rows = cursor.fetchall()
            except sqlite3.OperationalError:
                return []

        results = []
        for chunk_id, bm25_score in fts_rows:
            score = -bm25_score

            cursor.execute(
                "SELECT path, start_line, end_line, content FROM chunks WHERE id = ?",
                (chunk_id,),
            )
            chunk_row = cursor.fetchone()
            if chunk_row:
                path, start_line, end_line, content = chunk_row
                results.append(
                    {
                        "id": chunk_id,
                        "path": path,
                        "start_line": start_line,
                        "end_line": end_line,
                        "content": content,
                        "score": score,
                        "method": "lexical",
                    }
                )
        return results

    def hybrid_search(
        self, query: str, limit: int = 20, rrf_k: int = 60
    ) -> List[Dict[str, Any]]:
        """Combine semantic and lexical search rankings using Reciprocal Rank Fusion (RRF)."""
        fetch_limit = limit * 2
        sem_res = self.semantic_search(query, limit=fetch_limit)
        lex_res = self.lexical_search(query, limit=fetch_limit)

        sem_ranks = {item["id"]: idx for idx, item in enumerate(sem_res)}
        lex_ranks = {item["id"]: idx for idx, item in enumerate(lex_res)}

        all_items = {}
        for item in sem_res:
            all_items[item["id"]] = item
        for item in lex_res:
            if item["id"] not in all_items:
                all_items[item["id"]] = item

        rrf_scores = {}
        for chunk_id in all_items:
            score = 0.0
            if chunk_id in sem_ranks:
                score += 1.0 / (rrf_k + sem_ranks[chunk_id] + 1)
            if chunk_id in lex_ranks:
                score += 1.0 / (rrf_k + lex_ranks[chunk_id] + 1)
            rrf_scores[chunk_id] = score

        sorted_ids = sorted(
            rrf_scores.keys(), key=lambda cid: rrf_scores[cid], reverse=True
        )

        results = []
        for cid in sorted_ids[:limit]:
            item = all_items[cid].copy()
            item["score"] = rrf_scores[cid]
            item["method"] = "hybrid"
            results.append(item)

        return results

    def rerank(
        self, query: str, results: List[Dict[str, Any]], top_n: int = 10
    ) -> List[Dict[str, Any]]:
        """Rerank search results using the configured reranker provider."""
        if not results:
            return []

        top_n = min(top_n, len(results))
        documents = [item["content"] for item in results]
        try:
            scores = get_reranker().rerank(query, documents)
        except (RuntimeError, OSError, urllib.error.URLError, TypeError, AttributeError, ValueError) as e:
            print(
                f"Warning: Reranking failed ({e}), returning top results without reranking."
            )
            return results[:top_n]

        ranked = sorted(
            zip(results, scores), key=lambda item_score: item_score[1], reverse=True
        )
        final_results = []
        for item, score in ranked[:top_n]:
            copied = item.copy()
            copied["score"] = score
            copied["method"] = f"{copied['method']}+rerank"
            final_results.append(copied)
        return final_results


if __name__ == "__main__":
    import tempfile
    from pathlib import Path

    from ck_mlx.chunk import Chunk

    with tempfile.TemporaryDirectory() as tmpdir:
        tmp_path = Path(tmpdir)
        store = Store(tmp_path, embedding_dimension=8, embedding_model="debug-model")

        mock_chunks = [
            Chunk(
                content="def retrieve_embeddings(query):\n    # calls openai SDK",
                start_line=1,
                end_line=2,
            ),
            Chunk(
                content="def search_database(query):\n    # runs sql",
                start_line=4,
                end_line=5,
            ),
        ]
        mock_embs = [
            [0.1] * 8,
            [0.0] * 8,
        ]
        store.update_file("search.py", 123.45, "hash_xxx", mock_chunks, mock_embs)

        searcher = Searcher(store)

        res_lex = searcher.lexical_search("database")
        print(
            "Lexical search results:",
            [(r["path"], r["start_line"], r["score"]) for r in res_lex],
        )

        store.close()
