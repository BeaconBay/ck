use anyhow::Result;

#[cfg(feature = "fastembed")]
use std::path::{Path, PathBuf};

pub mod download;
pub mod reranker;
pub mod tokenizer;

pub use download::{ModelDownloadConfig, ModelDownloader};
pub use reranker::{RerankResult, Reranker, create_reranker, create_reranker_with_progress};
pub use tokenizer::TokenEstimator;

pub trait Embedder: Send + Sync {
    fn id(&self) -> &'static str;
    fn dim(&self) -> usize;
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

pub type ModelDownloadCallback = Box<dyn Fn(&str) + Send + Sync>;

pub fn create_embedder(model_name: Option<&str>) -> Result<Box<dyn Embedder>> {
    create_embedder_with_progress(model_name, None)
}

pub fn create_embedder_with_progress(
    model_name: Option<&str>,
    progress_callback: Option<ModelDownloadCallback>,
) -> Result<Box<dyn Embedder>> {
    let model = model_name.unwrap_or("BAAI/bge-small-en-v1.5");

    #[cfg(feature = "fastembed")]
    {
        Ok(Box::new(FastEmbedder::new_with_progress(
            model,
            progress_callback,
        )?))
    }

    #[cfg(not(feature = "fastembed"))]
    {
        let _ = model; // Suppress unused variable warning
        if let Some(callback) = progress_callback {
            callback("Using dummy embedder (no model download required)");
        }
        Ok(Box::new(DummyEmbedder::new()))
    }
}

pub struct DummyEmbedder {
    dim: usize,
}

impl Default for DummyEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

impl DummyEmbedder {
    pub fn new() -> Self {
        Self { dim: 384 } // Match default BGE model
    }
}

impl Embedder for DummyEmbedder {
    fn id(&self) -> &'static str {
        "dummy"
    }

    fn dim(&self) -> usize {
        self.dim
    }

    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|_| vec![0.0; self.dim]).collect())
    }
}

#[cfg(feature = "fastembed")]
pub struct FastEmbedder {
    model: fastembed::TextEmbedding,
    dim: usize,
}

#[cfg(feature = "fastembed")]
impl FastEmbedder {
    pub fn new(model_name: &str) -> Result<Self> {
        Self::new_with_progress(model_name, None)
    }

    pub fn new_with_progress(
        model_name: &str,
        progress_callback: Option<ModelDownloadCallback>,
    ) -> Result<Self> {
        Self::new_with_retry(model_name, progress_callback, 3)
    }

    pub fn new_with_retry(
        model_name: &str,
        progress_callback: Option<ModelDownloadCallback>,
        max_retries: u32,
    ) -> Result<Self> {
        use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
        use std::thread;
        use std::time::Duration;

        let model = match model_name {
            // Current models
            "BAAI/bge-small-en-v1.5" => EmbeddingModel::BGESmallENV15,
            "sentence-transformers/all-MiniLM-L6-v2" => EmbeddingModel::AllMiniLML6V2,

            // Enhanced models with longer context
            "nomic-embed-text-v1" => EmbeddingModel::NomicEmbedTextV1,
            "nomic-embed-text-v1.5" => EmbeddingModel::NomicEmbedTextV15,
            "jina-embeddings-v2-base-code" => EmbeddingModel::JinaEmbeddingsV2BaseCode,

            // BGE variants
            "BAAI/bge-base-en-v1.5" => EmbeddingModel::BGEBaseENV15,
            "BAAI/bge-large-en-v1.5" => EmbeddingModel::BGELargeENV15,

            // Default to Nomic v1.5 for better performance
            _ => EmbeddingModel::NomicEmbedTextV15,
        };

        // Configure permanent model cache directory
        let model_cache_dir = Self::get_model_cache_dir()?;
        std::fs::create_dir_all(&model_cache_dir)?;

        if let Some(ref callback) = progress_callback {
            callback(&format!("Initializing model: {}", model_name));

            // Check if model already exists
            let model_exists = Self::check_model_exists(&model_cache_dir, model_name);
            if !model_exists {
                callback(&format!(
                    "Downloading model {} to {}",
                    model_name,
                    model_cache_dir.display()
                ));
            } else {
                callback(&format!("Using cached model: {}", model_name));
            }
        }

        // Configure max_length based on model capacity
        let max_length = match model {
            // Small models - keep at 512
            EmbeddingModel::BGESmallENV15 | EmbeddingModel::AllMiniLML6V2 => 512,
            EmbeddingModel::BGEBaseENV15 => 512,

            // Large context models - use their full capacity!
            EmbeddingModel::NomicEmbedTextV1 | EmbeddingModel::NomicEmbedTextV15 => 8192,
            EmbeddingModel::JinaEmbeddingsV2BaseCode => 8192,

            // BGE large can handle more
            EmbeddingModel::BGELargeENV15 => 512, // Conservative for BGE

            _ => 512, // Safe default
        };

        // Try to create embedding with retry logic for network failures
        let mut last_error = None;
        let mut embedding = None;

        for attempt in 0..max_retries {
            if attempt > 0 {
                if let Some(ref callback) = progress_callback {
                    callback(&format!(
                        "Retrying download (attempt {}/{})",
                        attempt + 1,
                        max_retries
                    ));
                }
                thread::sleep(Duration::from_secs(2_u64.pow(attempt)));
            }

            let init_options = InitOptions::new(model.clone())
                .with_show_download_progress(progress_callback.is_some())
                .with_cache_dir(model_cache_dir.clone())
                .with_max_length(max_length);

            match TextEmbedding::try_new(init_options) {
                Ok(emb) => {
                    embedding = Some(emb);
                    break;
                }
                Err(e) => {
                    last_error = Some(e);
                    if let Some(ref callback) = progress_callback {
                        callback(&format!(
                            "Download attempt {} failed: {}",
                            attempt + 1,
                            last_error.as_ref().unwrap()
                        ));
                    }
                }
            }
        }

        let embedding = match embedding {
            Some(emb) => emb,
            None => {
                return Err(last_error.unwrap_or_else(|| {
                    anyhow::anyhow!("Failed to download model after {} attempts", max_retries)
                }));
            }
        };

        if let Some(ref callback) = progress_callback {
            callback("Model loaded successfully");
        }

        let dim = match model {
            // Small models (384 dimensions)
            EmbeddingModel::BGESmallENV15 => 384,
            EmbeddingModel::AllMiniLML6V2 => 384,

            // Large context models (768 dimensions)
            EmbeddingModel::NomicEmbedTextV1 => 768,
            EmbeddingModel::NomicEmbedTextV15 => 768,
            EmbeddingModel::JinaEmbeddingsV2BaseCode => 768,
            EmbeddingModel::BGEBaseENV15 => 768,

            // Large models (1024 dimensions)
            EmbeddingModel::BGELargeENV15 => 1024,

            _ => 384, // Default to 384 for BGE default
        };

        Ok(Self {
            model: embedding,
            dim,
        })
    }

    fn get_model_cache_dir() -> Result<PathBuf> {
        // Use platform-appropriate cache directory
        let cache_dir = if let Some(cache_home) = std::env::var_os("XDG_CACHE_HOME") {
            PathBuf::from(cache_home).join("ck")
        } else if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home).join(".cache").join("ck")
        } else if let Some(appdata) = std::env::var_os("LOCALAPPDATA") {
            PathBuf::from(appdata).join("ck").join("cache")
        } else {
            // Fallback to current directory if no home found
            PathBuf::from(".ck_models")
        };

        Ok(cache_dir.join("models"))
    }

    fn check_model_exists(cache_dir: &Path, model_name: &str) -> bool {
        // Simple heuristic - check if model directory exists
        let model_dir = cache_dir.join(model_name.replace("/", "_"));
        model_dir.exists()
    }
}

#[cfg(feature = "fastembed")]
impl Embedder for FastEmbedder {
    fn id(&self) -> &'static str {
        "fastembed"
    }

    fn dim(&self) -> usize {
        self.dim
    }

    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let embeddings = self.model.embed(text_refs, None)?;
        Ok(embeddings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dummy_embedder() {
        let mut embedder = DummyEmbedder::new();

        assert_eq!(embedder.id(), "dummy");
        assert_eq!(embedder.dim(), 384);

        let texts = vec!["hello".to_string(), "world".to_string()];
        let embeddings = embedder.embed(&texts).unwrap();

        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), 384);
        assert_eq!(embeddings[1].len(), 384);

        // Dummy embedder should return all zeros
        assert!(embeddings[0].iter().all(|&x| x == 0.0));
        assert!(embeddings[1].iter().all(|&x| x == 0.0));
    }

    #[test]
    fn test_create_embedder_dummy() {
        #[cfg(not(feature = "fastembed"))]
        {
            let embedder = create_embedder(None).unwrap();
            assert_eq!(embedder.id(), "dummy");
            assert_eq!(embedder.dim(), 384);
        }
    }

    #[test]
    fn test_embedder_trait_object() {
        let mut embedder: Box<dyn Embedder> = Box::new(DummyEmbedder::new());

        let texts = vec!["test".to_string()];
        let result = embedder.embed(&texts);
        assert!(result.is_ok());

        let embeddings = result.unwrap();
        assert_eq!(embeddings.len(), 1);
        assert_eq!(embeddings[0].len(), 384);
    }

    #[cfg(feature = "fastembed")]
    #[test]
    fn test_fastembed_creation() {
        // This test requires downloading models, so we'll skip it in CI
        if std::env::var("CI").is_ok() {
            return;
        }

        let embedder = FastEmbedder::new("BAAI/bge-small-en-v1.5");

        // FastEmbed creation might fail due to network issues or missing models
        // In a real test environment, you'd want to ensure models are available
        match embedder {
            Ok(mut embedder) => {
                assert_eq!(embedder.id(), "fastembed");
                assert_eq!(embedder.dim(), 384);

                let texts = vec!["hello world".to_string()];
                let result = embedder.embed(&texts);
                assert!(result.is_ok());

                let embeddings = result.unwrap();
                assert_eq!(embeddings.len(), 1);
                assert_eq!(embeddings[0].len(), 384);

                // Real embeddings should not be all zeros
                assert!(!embeddings[0].iter().all(|&x| x == 0.0));
            }
            Err(_) => {
                // In test environments, FastEmbed might not be available
                // This is acceptable for unit tests
            }
        }
    }

    #[cfg(feature = "fastembed")]
    #[test]
    fn test_create_embedder_fastembed() {
        if std::env::var("CI").is_ok() {
            return;
        }

        let embedder = create_embedder(Some("BAAI/bge-small-en-v1.5"));

        match embedder {
            Ok(embedder) => {
                assert_eq!(embedder.id(), "fastembed");
                assert_eq!(embedder.dim(), 384);
            }
            Err(_) => {
                // Model might not be available in test environment
            }
        }
    }

    #[test]
    fn test_embedder_empty_input() {
        let mut embedder = DummyEmbedder::new();
        let texts: Vec<String> = vec![];
        let embeddings = embedder.embed(&texts).unwrap();
        assert_eq!(embeddings.len(), 0);
    }

    #[test]
    fn test_embedder_single_text() {
        let mut embedder = DummyEmbedder::new();
        let texts = vec!["single text".to_string()];
        let embeddings = embedder.embed(&texts).unwrap();

        assert_eq!(embeddings.len(), 1);
        assert_eq!(embeddings[0].len(), 384);
    }

    #[test]
    fn test_embedder_multiple_texts() {
        let mut embedder = DummyEmbedder::new();
        let texts = vec![
            "first text".to_string(),
            "second text".to_string(),
            "third text".to_string(),
        ];
        let embeddings = embedder.embed(&texts).unwrap();

        assert_eq!(embeddings.len(), 3);
        for embedding in &embeddings {
            assert_eq!(embedding.len(), 384);
        }
    }
}
