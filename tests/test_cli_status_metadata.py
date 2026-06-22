import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from typer.testing import CliRunner

from ck_mlx import cli
from ck_mlx.chunk import Chunk
from ck_mlx.store import Store


class CliStatusMetadataTests(unittest.TestCase):
    def setUp(self):
        self.runner = CliRunner()

    def test_status_shows_embedding_metadata(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            store = Store(root, embedding_dimension=8, embedding_model="demo-model")
            store.update_file(
                "demo.py",
                1.0,
                "hash",
                [
                    Chunk(
                        content="def demo():\n    return 1\n", start_line=1, end_line=2
                    )
                ],
                [[0.1] * 8],
            )
            store.close()

            cwd = os.getcwd()
            try:
                os.chdir(root)
                result = self.runner.invoke(cli.app, ["status"])
            finally:
                os.chdir(cwd)

        self.assertEqual(result.exit_code, 0)
        self.assertIn("Embedding Model: demo-model", result.output)
        self.assertIn("Embedding Dimension: 8", result.output)

    def test_semantic_search_fails_fast_on_model_mismatch(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            store = Store(root, embedding_dimension=8, embedding_model="indexed-model")
            store.close()

            cwd = os.getcwd()
            try:
                os.chdir(root)
                with patch.dict(
                    os.environ, {"OMLX_MODEL": "runtime-model"}, clear=False
                ):
                    result = self.runner.invoke(
                        cli.app, ["search", "demo", "--mode", "semantic"]
                    )
            finally:
                os.chdir(cwd)

        self.assertEqual(result.exit_code, 1)
        self.assertIn("does not match indexed model", result.output)


if __name__ == "__main__":
    unittest.main()
