import os
import unittest
from unittest.mock import patch

from ck_mlx import embed
from ck_mlx.embed import APIEmbeddingProvider, EmbeddingProvider, get_provider


class FakeEmbeddingItem:
    def __init__(self, embedding, index=0):
        self.embedding = embedding
        self.index = index


class FakeEmbeddingResponse:
    def __init__(self, embedding):
        self.data = [FakeEmbeddingItem(embedding)]


class FakeEmbeddingsAPI:
    def __init__(self, embedding):
        self.embedding = embedding
        self.calls = []

    def create(self, **kwargs):
        self.calls.append(kwargs)
        return FakeEmbeddingResponse(self.embedding)


class FakeClient:
    def __init__(self, embedding):
        self.embeddings = FakeEmbeddingsAPI(embedding)


class EmbedProviderTests(unittest.TestCase):
    def setUp(self):
        embed.clear_embedding_caches()

    def tearDown(self):
        embed.clear_embedding_caches()

    def test_api_backend_returns_api_provider(self):
        with patch.dict(os.environ, {"CK_BACKEND": "api"}, clear=False):
            provider = get_provider()

        self.assertIsInstance(provider, APIEmbeddingProvider)
        self.assertIsInstance(provider, EmbeddingProvider)

    def test_api_provider_embeds_with_configured_model(self):
        fake_client = FakeClient([0.1, 0.2, 0.3])
        with patch.dict(os.environ, {"CK_BACKEND": "api", "OMLX_MODEL": "demo-model"}, clear=False):
            with patch("ck_mlx.embed._build_sync_client", return_value=fake_client):
                provider = APIEmbeddingProvider()
                vectors = provider.embed(["hello"], input_type="query")

        self.assertEqual(vectors, [[0.1, 0.2, 0.3]])
        self.assertEqual(provider.model_name(), "demo-model")
        self.assertEqual(fake_client.embeddings.calls[0]["model"], "demo-model")


if __name__ == "__main__":
    unittest.main()
