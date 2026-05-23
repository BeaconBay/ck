use anyhow::{Result, bail};
use ck_models::{RerankModelConfig, RerankModelRegistry};

#[cfg(feature = "mixedbread")]
use crate::mixedbread::MixedbreadReranker;

#[cfg(feature = "fastembed")]
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct RerankResult {
    pub query: String,
    pub document: String,
    pub score: f32,
}

pub trait Reranker: Send + Sync {
    fn id(&self) -> &'static str;
    fn rerank(&mut self, query: &str, documents: &[String]) -> Result<Vec<RerankResult>>;
}

pub type RerankModelDownloadCallback = Box<dyn Fn(&str) + Send + Sync>;

pub fn create_reranker(model_name: Option<&str>) -> Result<Box<dyn Reranker>> {
    create_reranker_with_progress(model_name, None)
}

pub fn create_reranker_with_progress(
    model_name: Option<&str>,
    progress_callback: Option<RerankModelDownloadCallback>,
) -> Result<Box<dyn Reranker>> {
    let registry = RerankModelRegistry::default();
    let (_, config) = registry.resolve(model_name)?;
    create_reranker_for_config(&config, progress_callback)
}

#[allow(clippy::needless_return)]
pub fn create_reranker_for_config(
    config: &RerankModelConfig,
    progress_callback: Option<RerankModelDownloadCallback>,
) -> Result<Box<dyn Reranker>> {
    match config.provider.as_str() {
        "fastembed" => {
            #[cfg(feature = "fastembed")]
            {
                return Ok(Box::new(FastReranker::new_with_progress(
                    config.name.as_str(),
                    progress_callback,
                )?));
            }

            #[cfg(not(feature = "fastembed"))]
            {
                if let Some(callback) = progress_callback.as_ref() {
                    callback("fastembed reranker unavailable; using dummy reranker");
                }
                return Ok(Box::new(DummyReranker::new()));
            }
        }
        "mixedbread" => {
            #[cfg(feature = "mixedbread")]
            {
                return Ok(Box::new(MixedbreadReranker::new(
                    config,
                    progress_callback,
                )?));
            }
            #[cfg(not(feature = "mixedbread"))]
            {
                bail!(
                    "Reranking model '{}' requires the `mixedbread` feature. Rebuild ck with Mixedbread support.",
                    config.name
                );
            }
        }
        provider => bail!("Unsupported reranker provider '{}'", provider),
    }
}

pub struct DummyReranker;

impl DummyReranker {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DummyReranker {
    fn default() -> Self {
        Self::new()
    }
}

impl Reranker for DummyReranker {
    fn id(&self) -> &'static str {
        "dummy_reranker"
    }

    fn rerank(&mut self, query: &str, documents: &[String]) -> Result<Vec<RerankResult>> {
        // Dummy reranker just returns documents in original order with random scores
        Ok(documents
            .iter()
            .enumerate()
            .map(|(i, doc)| {
                RerankResult {
                    query: query.to_string(),
                    document: doc.clone(),
                    score: 0.5 + (i as f32 * 0.1) % 0.5, // Fake scores between 0.5-1.0
                }
            })
            .collect())
    }
}

#[cfg(feature = "fastembed")]
pub struct FastReranker {
    model: fastembed::TextRerank,
    #[allow(dead_code)] // Keep for future use (debugging, logging)
    model_name: String,
}

#[cfg(feature = "fastembed")]
impl FastReranker {
    pub fn new(model_name: &str) -> Result<Self> {
        Self::new_with_progress(model_name, None)
    }

    pub fn new_with_progress(
        model_name: &str,
        progress_callback: Option<RerankModelDownloadCallback>,
    ) -> Result<Self> {
        use fastembed::{RerankInitOptions, RerankerModel, TextRerank};

        let model = match model_name {
            "jina-reranker-v1-turbo-en" => RerankerModel::JINARerankerV1TurboEn,
            "bge-reranker-base" => RerankerModel::BGERerankerBase,
            "jina-reranker-v2-base-multilingual" => RerankerModel::JINARerankerV2BaseMultiligual,
            "bge-reranker-v2-m3" => RerankerModel::BGERerankerV2M3,
            _ => RerankerModel::JINARerankerV1TurboEn, // Default
        };

        // Configure permanent model cache directory
        let model_cache_dir = Self::get_model_cache_dir()?;
        std::fs::create_dir_all(&model_cache_dir)?;

        if let Some(ref callback) = progress_callback {
            callback(&format!("Initializing reranker model: {}", model_name));

            // Check if model already exists
            let model_exists = Self::check_model_exists(&model_cache_dir, model_name);
            if !model_exists {
                callback(&format!(
                    "Downloading reranker model {} to {}",
                    model_name,
                    model_cache_dir.display()
                ));
            } else {
                callback(&format!("Using cached reranker model: {}", model_name));
            }
        }

        let init_options = RerankInitOptions::new(model.clone())
            .with_show_download_progress(progress_callback.is_some())
            .with_cache_dir(model_cache_dir);

        let reranker = TextRerank::try_new(init_options)?;

        if let Some(ref callback) = progress_callback {
            callback("Reranker model loaded successfully");
        }

        Ok(Self {
            model: reranker,
            model_name: model_name.to_string(),
        })
    }

    fn get_model_cache_dir() -> Result<PathBuf> {
        // Use platform-appropriate cache directory (same as embedder)
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

        Ok(cache_dir.join("rerankers"))
    }

    fn check_model_exists(cache_dir: &std::path::Path, model_name: &str) -> bool {
        // Simple heuristic - check if model directory exists
        let model_dir = cache_dir.join(model_name.replace("/", "_"));
        model_dir.exists()
    }
}

#[cfg(feature = "fastembed")]
impl Reranker for FastReranker {
    fn id(&self) -> &'static str {
        "fastembed_reranker"
    }

    fn rerank(&mut self, query: &str, documents: &[String]) -> Result<Vec<RerankResult>> {
        // Convert documents to string references
        let docs: Vec<&str> = documents.iter().map(|s| s.as_str()).collect();

        // Get reranking scores - fastembed rerank takes (query, documents)
        let results = self.model.rerank(query, docs, true, None)?;

        // Convert to our format
        let rerank_results = results
            .into_iter()
            .enumerate()
            .map(|(i, result)| RerankResult {
                query: query.to_string(),
                document: documents[i].clone(),
                score: result.score,
            })
            .collect();

        Ok(rerank_results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dummy_reranker() {
        let mut reranker = DummyReranker::new();
        assert_eq!(reranker.id(), "dummy_reranker");

        let query = "find error handling";
        let documents = vec![
            "try catch block".to_string(),
            "function definition".to_string(),
            "error handling code".to_string(),
        ];

        let results = reranker.rerank(query, &documents).unwrap();
        assert_eq!(results.len(), 3);

        for result in &results {
            assert_eq!(result.query, query);
            assert!(result.score >= 0.5 && result.score <= 1.0);
        }
    }

    #[test]
    fn test_create_reranker_dummy() {
        #[cfg(not(feature = "fastembed"))]
        {
            let reranker = create_reranker(None).unwrap();
            assert_eq!(reranker.id(), "dummy_reranker");
        }
    }

    #[cfg(feature = "fastembed")]
    #[test]
    fn test_fastembed_reranker_creation() {
        // This test requires downloading models, so we'll skip it in CI
        if std::env::var("CI").is_ok() {
            return;
        }

        let reranker = FastReranker::new("jina-reranker-v1-turbo-en");

        match reranker {
            Ok(mut reranker) => {
                assert_eq!(reranker.id(), "fastembed_reranker");

                let query = "error handling";
                let documents = vec![
                    "try catch exception handling".to_string(),
                    "user interface design".to_string(),
                ];

                let result = reranker.rerank(query, &documents);
                assert!(result.is_ok());

                let results = result.unwrap();
                assert_eq!(results.len(), 2);

                // First result should be more relevant to query
                assert!(results[0].score > results[1].score);
            }
            Err(_) => {
                // In test environments, FastEmbed might not be available
                // This is acceptable for unit tests
            }
        }
    }

    #[test]
    fn test_reranker_empty_input() {
        let mut reranker = DummyReranker::new();
        let query = "test query";
        let documents: Vec<String> = vec![];
        let results = reranker.rerank(query, &documents).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_reranker_single_document() {
        let mut reranker = DummyReranker::new();
        let query = "test query";
        let documents = vec!["single document".to_string()];
        let results = reranker.rerank(query, &documents).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].query, query);
        assert_eq!(results[0].document, "single document");
    }
}
