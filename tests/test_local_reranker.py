import os
import unittest
from unittest.mock import patch

from ck_mlx.rerank import LocalMLXRerankerProvider


class FakeLogits:
    def tolist(self):
        return [[0.2], [0.9]]


class FakeRerankOutput:
    logits = FakeLogits()


class FakeRerankModel:
    def __call__(self, **kwargs):
        return FakeRerankOutput()


class FakeTokenizer:
    def __call__(self, pairs, **kwargs):
        return {"input_ids": [[1], [2]]}


class LocalRerankerTests(unittest.TestCase):
    def test_local_reranker_scores_each_document(self):
        with patch("mlx_embeddings.load", return_value=(FakeRerankModel(), FakeTokenizer())) as load:
            provider = LocalMLXRerankerProvider("demo-reranker")

            scores = provider.rerank("query", ["first", "second"])

        self.assertEqual(scores, [0.2, 0.9])
        self.assertTrue(provider.is_available())
        load.assert_called_once_with("demo-reranker")

    def test_local_reranker_model_env_overrides_default_model(self):
        with patch.dict(os.environ, {"CK_LOCAL_RERANK_MODEL": "env-reranker"}, clear=False):
            provider = LocalMLXRerankerProvider()

        self.assertEqual(provider.model_name(), "env-reranker")


if __name__ == "__main__":
    unittest.main()
