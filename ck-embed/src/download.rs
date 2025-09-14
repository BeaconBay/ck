use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use std::time::Duration;

type ProgressCallback = Box<dyn Fn(&str) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct ModelDownloadConfig {
    pub max_retries: u32,
    pub timeout: Duration,
    pub cache_dir: PathBuf,
    pub offline_mode: bool,
    pub verbose: bool,
}

impl Default for ModelDownloadConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            timeout: Duration::from_secs(300),
            cache_dir: Self::default_cache_dir(),
            offline_mode: false,
            verbose: false,
        }
    }
}

impl ModelDownloadConfig {
    pub fn default_cache_dir() -> PathBuf {
        #[cfg(target_os = "linux")]
        {
            if let Ok(xdg_cache) = std::env::var("XDG_CACHE_HOME") {
                return PathBuf::from(xdg_cache).join("ck").join("models");
            }
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home).join(".cache").join("ck").join("models");
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home).join(".cache").join("ck").join("models");
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                return PathBuf::from(local_app_data)
                    .join("ck")
                    .join("cache")
                    .join("models");
            }
        }

        PathBuf::from(".ck_models").join("models")
    }
}

pub struct ModelDownloader {
    config: ModelDownloadConfig,
}

impl ModelDownloader {
    pub fn new(config: ModelDownloadConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(ModelDownloadConfig::default())
    }

    pub fn check_model_cached(&self, model_name: &str) -> Result<Option<PathBuf>> {
        let model_path = self.config.cache_dir.join(model_name);

        if model_path.exists() {
            let onnx_file = model_path.join("model.onnx");
            let optimized_file = model_path.join("model_optimized.onnx");

            if onnx_file.exists() || optimized_file.exists() {
                if self.config.verbose {
                    eprintln!(
                        "✅ Model '{}' found in cache at {}",
                        model_name,
                        model_path.display()
                    );
                }
                return Ok(Some(model_path));
            }
        }

        if self.config.offline_mode {
            bail!(
                "Model '{}' not found in cache at {}. \
                Cannot download in offline mode. \
                Please download the model first using: ck --download-model {}",
                model_name,
                self.config.cache_dir.display(),
                model_name
            );
        }

        Ok(None)
    }

    pub async fn download_with_retry(
        &self,
        model_name: &str,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<PathBuf> {
        if let Some(cached_path) = self.check_model_cached(model_name)? {
            return Ok(cached_path);
        }

        let mut last_error = None;

        for attempt in 1..=self.config.max_retries {
            if attempt > 1 {
                let backoff = Duration::from_secs(2_u64.pow(attempt - 1));
                if let Some(ref cb) = progress_callback {
                    cb(&format!(
                        "⏳ Waiting {}s before retry {}/{}...",
                        backoff.as_secs(),
                        attempt,
                        self.config.max_retries
                    ));
                }
                tokio::time::sleep(backoff).await;
            }

            if let Some(ref cb) = progress_callback {
                cb(&format!(
                    "📥 Downloading model '{}' (attempt {}/{})...",
                    model_name, attempt, self.config.max_retries
                ));
            }

            match self.try_download(model_name, &progress_callback).await {
                Ok(path) => {
                    if let Some(ref cb) = progress_callback {
                        cb(&format!(
                            "✅ Model '{}' downloaded successfully",
                            model_name
                        ));
                    }
                    return Ok(path);
                }
                Err(e) => {
                    last_error = Some(e);
                    if let Some(ref cb) = progress_callback {
                        cb(&format!("⚠️  Download attempt {} failed", attempt));
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            anyhow::anyhow!(
                "Failed to download model after {} attempts",
                self.config.max_retries
            )
        }))
    }

    async fn try_download(
        &self,
        model_name: &str,
        _progress_callback: &Option<ProgressCallback>,
    ) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.config.cache_dir)
            .context("Failed to create model cache directory")?;

        #[cfg(feature = "fastembed")]
        {
            use fastembed::{InitOptions, TextEmbedding};

            let model = Self::parse_model_name(model_name)?;

            let init_options =
                InitOptions::new(model).with_cache_dir(self.config.cache_dir.clone());

            let timeout_result = tokio::time::timeout(
                self.config.timeout,
                tokio::task::spawn_blocking(move || TextEmbedding::try_new(init_options)),
            )
            .await;

            match timeout_result {
                Ok(Ok(Ok(_))) => {
                    let model_path = self.config.cache_dir.join(model_name);
                    Ok(model_path)
                }
                Ok(Ok(Err(e))) => {
                    bail!("Failed to initialize model: {}", e)
                }
                Ok(Err(e)) => {
                    bail!("Task panicked: {}", e)
                }
                Err(_) => {
                    bail!("Download timeout after {:?}", self.config.timeout)
                }
            }
        }

        #[cfg(not(feature = "fastembed"))]
        {
            let _ = (model_name, progress_callback);
            bail!("FastEmbed feature not enabled. Cannot download models.")
        }
    }

    #[cfg(feature = "fastembed")]
    fn parse_model_name(model_name: &str) -> Result<fastembed::EmbeddingModel> {
        use fastembed::EmbeddingModel;

        Ok(match model_name {
            "BAAI/bge-small-en-v1.5" => EmbeddingModel::BGESmallENV15,
            "nomic-embed-text-v1.5" => EmbeddingModel::NomicEmbedTextV15,
            "jina-embeddings-v2-base-code" => EmbeddingModel::JinaEmbeddingsV2BaseCode,
            _ => bail!("Unknown model: {}", model_name),
        })
    }

    pub fn list_cached_models(&self) -> Result<Vec<String>> {
        let mut models = Vec::new();

        if !self.config.cache_dir.exists() {
            return Ok(models);
        }

        for entry in std::fs::read_dir(&self.config.cache_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let model_dir = entry.path();
                let onnx_file = model_dir.join("model.onnx");
                let optimized_file = model_dir.join("model_optimized.onnx");

                if (onnx_file.exists() || optimized_file.exists())
                    && let Some(name) = entry.file_name().to_str()
                {
                    models.push(name.to_string());
                }
            }
        }

        Ok(models)
    }

    pub fn validate_offline_setup(&self) -> Result<()> {
        let cached = self.list_cached_models()?;

        if cached.is_empty() {
            bail!(
                "No models found in cache at {}. \
                Please download at least one model first:\n\
                  ck --download-model BAAI/bge-small-en-v1.5\n\
                  ck --download-model nomic-embed-text-v1.5",
                self.config.cache_dir.display()
            );
        }

        if self.config.verbose {
            eprintln!("📦 Found {} cached models:", cached.len());
            for model in &cached {
                eprintln!("  • {}", model);
            }
        }

        Ok(())
    }
}
