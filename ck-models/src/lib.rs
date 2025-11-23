use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub name: String,
    pub provider: String,
    pub dimensions: usize,
    pub max_tokens: usize,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRegistry {
    pub models: HashMap<String, ModelConfig>,
    pub default_model: String,
}

impl Default for ModelRegistry {
    fn default() -> Self {
        let mut models = HashMap::new();

        models.insert(
            "bge-small".to_string(),
            ModelConfig {
                name: "BAAI/bge-small-en-v1.5".to_string(),
                provider: "fastembed".to_string(),
                dimensions: 384,
                max_tokens: 512,
                description: "Small, fast English embedding model".to_string(),
            },
        );

        models.insert(
            "minilm".to_string(),
            ModelConfig {
                name: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
                provider: "fastembed".to_string(),
                dimensions: 384,
                max_tokens: 256,
                description: "Lightweight English embedding model".to_string(),
            },
        );

        // Add enhanced models
        models.insert(
            "nomic-v1.5".to_string(),
            ModelConfig {
                name: "nomic-embed-text-v1.5".to_string(),
                provider: "fastembed".to_string(),
                dimensions: 768,
                max_tokens: 8192,
                description: "High-quality English embedding model with large context window"
                    .to_string(),
            },
        );

        models.insert(
            "jina-code".to_string(),
            ModelConfig {
                name: "jina-embeddings-v2-base-code".to_string(),
                provider: "fastembed".to_string(),
                dimensions: 768,
                max_tokens: 8192,
                description: "Code-specific embedding model optimized for programming tasks"
                    .to_string(),
            },
        );

        models.insert(
            "mxbai-xsmall".to_string(),
            ModelConfig {
                name: "mixedbread-ai/mxbai-embed-xsmall-v1".to_string(),
                provider: "mixedbread".to_string(),
                dimensions: 384,
                max_tokens: 4096,
                description: "Mixedbread xsmall embedding model (4k context, 384 dims) optimized for local semantic search".to_string(),
            },
        );

        Self {
            models,
            default_model: "bge-small".to_string(), // Keep BGE as default for backward compatibility
        }
    }
}

impl ModelRegistry {
    fn format_available_models(&self) -> String {
        self.models.keys().cloned().collect::<Vec<_>>().join(", ")
    }

    fn resolve_alias_or_name(&self, key: &str) -> Option<(String, &ModelConfig)> {
        if let Some(config) = self.models.get(key) {
            return Some((key.to_string(), config));
        }

        self.models
            .iter()
            .find(|(_, config)| config.name == key)
            .map(|(alias, config)| (alias.clone(), config))
    }

    pub fn resolve(&self, requested: Option<&str>) -> Result<(String, ModelConfig)> {
        match requested {
            Some(name) => {
                let (alias, config) = self.resolve_alias_or_name(name).ok_or_else(|| {
                    anyhow!(
                        "Unknown model '{}'. Available models: {}",
                        name,
                        self.format_available_models()
                    )
                })?;
                Ok((alias, config.clone()))
            }
            None => {
                let alias = self.default_model.clone();
                let config = self
                    .get_default_model()
                    .cloned()
                    .ok_or_else(|| anyhow!("No default model configured in registry"))?;
                Ok((alias, config))
            }
        }
    }

    pub fn aliases(&self) -> Vec<String> {
        let mut keys = self.models.keys().cloned().collect::<Vec<_>>();
        keys.sort();
        keys
    }

    pub fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let data = std::fs::read_to_string(path)?;
            Ok(serde_json::from_str(&data)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    pub fn get_model(&self, name: &str) -> Option<&ModelConfig> {
        self.models.get(name)
    }

    pub fn get_default_model(&self) -> Option<&ModelConfig> {
        self.models.get(&self.default_model)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankModelConfig {
    pub name: String,
    pub provider: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankModelRegistry {
    pub models: HashMap<String, RerankModelConfig>,
    pub default_model: String,
}

impl Default for RerankModelRegistry {
    fn default() -> Self {
        let mut models = HashMap::new();

        models.insert(
            "jina".to_string(),
            RerankModelConfig {
                name: "jina-reranker-v1-turbo-en".to_string(),
                provider: "fastembed".to_string(),
                description:
                    "Jina Turbo reranker (default) tuned for English code + text relevance"
                        .to_string(),
            },
        );

        models.insert(
            "bge".to_string(),
            RerankModelConfig {
                name: "BAAI/bge-reranker-base".to_string(),
                provider: "fastembed".to_string(),
                description: "BGE reranker base model for multilingual use cases".to_string(),
            },
        );

        models.insert(
            "mxbai".to_string(),
            RerankModelConfig {
                name: "mixedbread-ai/mxbai-rerank-xsmall-v1".to_string(),
                provider: "mixedbread".to_string(),
                description: "Mixedbread xsmall reranker (quantized) optimized for local inference"
                    .to_string(),
            },
        );

        Self {
            models,
            default_model: "jina".to_string(),
        }
    }
}

impl RerankModelRegistry {
    fn format_available_models(&self) -> String {
        self.models.keys().cloned().collect::<Vec<_>>().join(", ")
    }

    fn resolve_alias_or_name(&self, key: &str) -> Option<(String, &RerankModelConfig)> {
        if let Some(config) = self.models.get(key) {
            return Some((key.to_string(), config));
        }

        self.models
            .iter()
            .find(|(_, config)| config.name == key)
            .map(|(alias, config)| (alias.clone(), config))
    }

    pub fn resolve(&self, requested: Option<&str>) -> Result<(String, RerankModelConfig)> {
        match requested {
            Some(name) => {
                let (alias, config) = self.resolve_alias_or_name(name).ok_or_else(|| {
                    anyhow!(
                        "Unknown rerank model '{}'. Available models: {}",
                        name,
                        self.format_available_models()
                    )
                })?;
                Ok((alias, config.clone()))
            }
            None => {
                let alias = self.default_model.clone();
                let config = self
                    .models
                    .get(&self.default_model)
                    .cloned()
                    .ok_or_else(|| anyhow!("No default reranking model configured"))?;
                Ok((alias, config))
            }
        }
    }

    pub fn aliases(&self) -> Vec<String> {
        let mut keys = self.models.keys().cloned().collect::<Vec<_>>();
        keys.sort();
        keys
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub model: String,
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub index_backend: String,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            model: "bge-small".to_string(),
            chunk_size: 512,
            chunk_overlap: 128,
            index_backend: "hnsw".to_string(),
        }
    }
}

impl ProjectConfig {
    pub fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let data = std::fs::read_to_string(path)?;
            Ok(serde_json::from_str(&data)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }
}
