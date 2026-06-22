import os
import unittest
from unittest.mock import patch

from ck_mlx.embed import LocalMLXEmbeddingProvider


class FakeEmbeddingOutput:
    text_embeds = [[0.1, 0.2, 0.3]]


class FakeEmbeddingModel:
    def __call__(self, **kwargs):
        return FakeEmbeddingOutput()


class FakeTokenizer:
    def __call__(self, texts, **kwargs):
        return {"input_ids": [[1, 2, 3]]}


class LocalEmbedProviderTests(unittest.TestCase):
    def test_local_embedder_loads_lazily_and_reports_dimension(self):
        with patch("mlx_embeddings.load", return_value=(FakeEmbeddingModel(), FakeTokenizer())) as load:
            provider = LocalMLXEmbeddingProvider("demo-local-model")

            vectors = provider.embed(["hello world"])
            dimension = provider.dimension()

        self.assertEqual(vectors, [[0.1, 0.2, 0.3]])
        self.assertEqual(dimension, 3)
        load.assert_called_once_with("demo-local-model")

    def test_local_model_env_overrides_default_model(self):
        with patch.dict(os.environ, {"CK_LOCAL_MODEL": "env-local-model"}, clear=False):
            provider = LocalMLXEmbeddingProvider()

        self.assertEqual(provider.model_name(), "env-local-model")


if __name__ == "__main__":
    unittest.main()
