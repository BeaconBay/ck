import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from ck_mlx.chunk import Chunker, find_tokenizer_path


class ChunkTokenizerLocalTests(unittest.TestCase):
    def test_ck_tokenizer_path_takes_precedence(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            tok_path = Path(tmpdir) / "tokenizer.json"
            tok_path.write_text("{}")
            with patch.dict(os.environ, {"CK_TOKENIZER_PATH": str(tok_path)}, clear=False):
                found = find_tokenizer_path()

        self.assertEqual(found, str(tok_path))

    def test_local_backend_checks_hf_cache_and_falls_back_safely(self):
        with patch.dict(
            os.environ,
            {"CK_BACKEND": "local", "CK_LOCAL_MODEL": "mlx-community/demo"},
            clear=False,
        ):
            with patch("huggingface_hub.try_to_load_from_cache", return_value=None) as cache:
                found = find_tokenizer_path()
                chunker = Chunker()

        self.assertIsNone(found)
        self.assertIsNone(chunker.tokenizer)
        cache.assert_called_with("mlx-community/demo", "tokenizer.json")


if __name__ == "__main__":
    unittest.main()
