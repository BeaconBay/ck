import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from typer.testing import CliRunner

from ck_mlx import cli
from ck_mlx.chunk import Chunk
from ck_mlx.store import Store


class CliBackendOptionTests(unittest.TestCase):
    def setUp(self):
        self.runner = CliRunner()

    def test_help_shows_backend_option(self):
        result = self.runner.invoke(cli.app, ["--help"])

        self.assertEqual(result.exit_code, 0)
        self.assertIn("--backend", result.output)

    def test_backend_option_sets_environment_before_status(self):
        original = os.environ.pop("CK_BACKEND", None)
        try:
            result = self.runner.invoke(cli.app, ["--backend", "local", "status"])
        finally:
            if original is not None:
                os.environ["CK_BACKEND"] = original
            else:
                os.environ.pop("CK_BACKEND", None)

        self.assertEqual(result.exit_code, 0)
        self.assertIn("Active Backend: local", result.output)
        self.assertIn("Embedding Model: mlx-community/bge-small-en-v1.5-6bit", result.output)

    def test_status_reports_active_backend_and_index_metadata(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            store = Store(root, embedding_dimension=8, embedding_model="demo-model")
            store.update_file(
                "demo.py",
                1.0,
                "hash",
                [Chunk(content="def demo():\n    return 1\n", start_line=1, end_line=2)],
                [[0.1] * 8],
            )
            store.close()
            with patch.dict(os.environ, {"CK_BACKEND": "api", "OMLX_MODEL": "demo-model"}, clear=False):
                with self.runner.isolated_filesystem(temp_dir=tmpdir):
                    os.chdir(root)
                    result = self.runner.invoke(cli.app, ["status"])

        self.assertEqual(result.exit_code, 0)
        self.assertIn("Active Backend: api", result.output)
        self.assertIn("Indexed Embedding Model: demo-model", result.output)
        self.assertIn("Indexed Embedding Dimension: 8", result.output)


if __name__ == "__main__":
    unittest.main()
