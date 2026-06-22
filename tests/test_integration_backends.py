import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from typer.testing import CliRunner

from ck_mlx import cli


def fake_vectors(texts, input_type=None):
    vectors = []
    for text in texts:
        if "embedding" in text.lower():
            vectors.append([1.0, 0.0, 0.0])
        else:
            vectors.append([0.0, 1.0, 0.0])
    return vectors


class IntegrationBackendsTests(unittest.TestCase):
    def setUp(self):
        self.runner = CliRunner()

    def test_local_backend_indexes_and_searches_with_cli_surface(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / "sample.py").write_text(
                "def embedding_provider():\n    return 'local embedding provider'\n"
            )
            with patch.dict(os.environ, {"CK_BACKEND": "local", "CK_LOCAL_MODEL": "local-test-model"}, clear=False):
                with patch(
                    "ck_mlx.cli_index.get_embedding_metadata",
                    return_value={"model": "local-test-model", "dimension": 3},
                ):
                    with patch("ck_mlx.cli_index.embed_texts", side_effect=fake_vectors):
                        index_result = self.runner.invoke(cli.app, ["index", str(root), "--force"])
                with patch("ck_mlx.search.embed_texts", side_effect=fake_vectors):
                    cwd = os.getcwd()
                    try:
                        os.chdir(root)
                        search_result = self.runner.invoke(
                            cli.app, ["search", "embedding provider", "--mode", "semantic"]
                        )
                    finally:
                        os.chdir(cwd)

        self.assertEqual(index_result.exit_code, 0, index_result.output)
        self.assertEqual(search_result.exit_code, 0, search_result.output)
        self.assertIn("sample.py", search_result.output)

    def test_api_backend_status_when_api_key_is_configured(self):
        if not os.environ.get("OMLX_API_KEY"):
            self.skipTest("OMLX_API_KEY is not configured")
        result = self.runner.invoke(cli.app, ["--backend", "api", "status"])

        self.assertEqual(result.exit_code, 0)
        self.assertIn("Active Backend: api", result.output)


if __name__ == "__main__":
    unittest.main()
