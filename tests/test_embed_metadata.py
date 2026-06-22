import os
import unittest
from unittest.mock import patch

from ck_mlx import embed


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


class EmbedMetadataTests(unittest.TestCase):
    def setUp(self):
        embed.clear_embedding_caches()

    def tearDown(self):
        embed.clear_embedding_caches()

    def test_get_embedding_metadata_reports_model_and_dimension(self):
        fake_client = FakeClient([0.1] * 8)
        with patch.dict(os.environ, {"CK_BACKEND": "api", "OMLX_MODEL": "test-embed-model"}, clear=False):
            with patch("ck_mlx.embed._build_sync_client", return_value=fake_client):
                metadata = embed.get_embedding_metadata()

        self.assertEqual(metadata["model"], "test-embed-model")
        self.assertEqual(metadata["dimension"], 8)
        self.assertEqual(fake_client.embeddings.calls[0]["model"], "test-embed-model")

    def test_ensure_embedding_compatible_rejects_model_mismatch(self):
        with patch.dict(os.environ, {"CK_BACKEND": "api", "OMLX_MODEL": "runtime-model"}, clear=False):
            with self.assertRaisesRegex(ValueError, "does not match indexed model"):
                embed.ensure_embedding_compatible(
                    index_model="indexed-model",
                    index_dimension=8,
                    probe_dimension=False,
                )


if __name__ == "__main__":
    unittest.main()
