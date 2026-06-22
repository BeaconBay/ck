import tempfile
import unittest
from pathlib import Path

from ck_mlx.chunk import Chunk
from ck_mlx.store import Store


class StoreDynamicDimensionsTests(unittest.TestCase):
    def test_store_persists_dynamic_dimension_and_model(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            store = Store(root, embedding_dimension=8, embedding_model="demo-model")
            chunks = [
                Chunk(content="def demo():\n    return 1\n", start_line=1, end_line=2)
            ]
            embeddings = [[0.2] * 8]
            store.update_file("demo.py", 1.0, "hash", chunks, embeddings)
            status = store.get_status()
            store.close()

            reopened = Store(root)
            reopened_status = reopened.get_status()
            reopened.close()

        self.assertEqual(status["embedding_dimension"], 8)
        self.assertEqual(status["embedding_model"], "demo-model")
        self.assertEqual(status["total_chunks"], 1)
        self.assertEqual(reopened_status["embedding_dimension"], 8)
        self.assertEqual(reopened_status["embedding_model"], "demo-model")

    def test_store_rejects_dimension_mismatch(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            store = Store(root, embedding_dimension=8, embedding_model="demo-model")
            store.close()

            with self.assertRaisesRegex(
                ValueError, "does not match requested dimension"
            ):
                Store(root, embedding_dimension=16, embedding_model="demo-model")


if __name__ == "__main__":
    unittest.main()
