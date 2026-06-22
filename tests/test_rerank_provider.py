import os
import unittest
from unittest.mock import patch

from ck_mlx.rerank import APIRerankerProvider, RerankerProvider, get_reranker


class RerankProviderTests(unittest.TestCase):
    def test_api_backend_returns_api_reranker(self):
        with patch.dict(os.environ, {"CK_BACKEND": "api"}, clear=False):
            provider = get_reranker()

        self.assertIsInstance(provider, APIRerankerProvider)
        self.assertIsInstance(provider, RerankerProvider)
        self.assertTrue(callable(provider.rerank))

    def test_api_reranker_accepts_empty_docs(self):
        provider = APIRerankerProvider()

        scores = provider.rerank("query", [])

        self.assertEqual(scores, [])


if __name__ == "__main__":
    unittest.main()
