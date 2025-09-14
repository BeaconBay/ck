use super::{Command, CommandContext};
// use crate::error::CkError;
use crate::progress::StatusReporter;
use anyhow::Result as AnyhowResult;
// use console::style;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::time::Duration;

pub struct IndexCommand {
    pub path: PathBuf,
    pub model: Option<String>,
    pub exclude_patterns: Vec<String>,
    pub force_rebuild: bool,
    pub context: CommandContext,
    pub max_retries: u32,
}

impl IndexCommand {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            model: None,
            exclude_patterns: Vec::new(),
            force_rebuild: false,
            context: CommandContext::default(),
            max_retries: 3,
        }
    }

    async fn download_model_with_retry(&self, model_name: &str) -> AnyhowResult<()> {
        // In offline mode, skip download entirely and validate cached model exists
        if self.max_retries == 0 {
            let downloader = ck_embed::ModelDownloader::new(ck_embed::ModelDownloadConfig {
                offline_mode: true,
                verbose: self.context.verbose,
                ..Default::default()
            });

            match downloader.check_model_cached(model_name).map_err(|e| anyhow::anyhow!(e))? {
                Some(_) => return Ok(()), // Model exists in cache
                None => {
                    anyhow::bail!(
                        "❌ Model '{}' not found in cache.\n💡 Download the model first: ck --download-model {}",
                        model_name, model_name
                    );
                }
            }
        }

        let mut attempts = 0;
        let mut last_error = None;

        while attempts < self.max_retries {
            attempts += 1;

            if attempts > 1 && !self.context.quiet {
                eprintln!("🔄 Retry attempt {}/{} for model download", attempts, self.max_retries);
                tokio::time::sleep(Duration::from_secs(2_u64.pow(attempts - 1))).await;
            }

            match self.try_download_model(model_name).await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    last_error = Some(e);
                    if !self.context.quiet {
                        eprintln!("⚠️  Download attempt {} failed", attempts);
                    }
                }
            }
        }

        Err(anyhow::anyhow!(
            "❌ Failed to download model '{}' after {} attempts: {}\n💡 Pre-download the model manually or use --offline mode with cached models",
            model_name,
            self.max_retries,
            last_error.map(|e| e.to_string()).unwrap_or_else(|| "Unknown error".to_string())
        ))
    }

    async fn try_download_model(&self, model_name: &str) -> AnyhowResult<()> {
        let status = StatusReporter::new(self.context.verbose);

        let progress_callback = if !self.context.no_progress {
            Some(Box::new(move |msg: &str| {
                status.info(msg);
            }) as ck_embed::ModelDownloadCallback)
        } else {
            None
        };

        ck_embed::create_embedder_with_progress(Some(model_name), progress_callback)?;

        Ok(())
    }

    async fn validate_prerequisites(&self) -> AnyhowResult<()> {
        if !self.path.exists() {
            anyhow::bail!("❌ Path does not exist: {}", self.path.display());
        }

        if !self.path.is_dir() && !self.path.is_file() {
            anyhow::bail!("❌ Path is neither a file nor directory: {}", self.path.display());
        }

        Ok(())
    }

    fn setup_progress_bars(&self) -> (MultiProgress, ProgressBar, ProgressBar) {
        let multi_progress = MultiProgress::new();

        // Start with unknown length - will be set when we know file count
        let overall_pb = multi_progress.add(ProgressBar::new_spinner());
        overall_pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} Files: {pos} processed {msg}")
                .unwrap()
        );

        // Start with unknown length - will be set when we know chunk count
        let file_pb = multi_progress.add(ProgressBar::new_spinner());
        file_pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} Chunks: {pos} processed {msg}")
                .unwrap()
        );

        (multi_progress, overall_pb, file_pb)
    }
}

#[async_trait::async_trait]
impl Command for IndexCommand {
    fn name(&self) -> &'static str {
        "index"
    }

    async fn execute(&self) -> AnyhowResult<()> {
        self.validate_prerequisites().map_err(|e| anyhow::anyhow!(e))?;

        let status = StatusReporter::new(self.context.verbose);
        status.section_header("Indexing Repository");
        status.info(&format!("📁 Target: {}", self.path.display()));

        let model_name = self.model.as_deref().unwrap_or("nomic-embed-text-v1.5");
        status.info(&format!("🤖 Model: {}", model_name));

        if let Err(e) = self.download_model_with_retry(model_name).await {
            if !self.context.quiet {
                eprintln!("{}", e);
            }
            return Err(anyhow::anyhow!(e));
        }

        let (multi_progress, overall_pb, file_pb) = if !self.context.no_progress {
            self.setup_progress_bars()
        } else {
            (MultiProgress::new(), ProgressBar::hidden(), ProgressBar::hidden())
        };

        let overall_pb_clone = overall_pb.clone();
        let file_pb_clone = file_pb.clone();

        let progress_callback = Some(Box::new(move |file_name: &str| {
            let short_name = file_name.split('/').last().unwrap_or(file_name);
            overall_pb_clone.set_message(format!("Processing {}", short_name));
            overall_pb_clone.inc(1);
        }) as ck_index::ProgressCallback);

        let detailed_callback = Some(Box::new(move |progress: ck_index::EmbeddingProgress| {
            if file_pb_clone.length().unwrap_or(0) != progress.total_chunks as u64 {
                file_pb_clone.set_length(progress.total_chunks as u64);
                file_pb_clone.reset();
            }
            file_pb_clone.set_position(progress.chunk_index as u64);
            file_pb_clone.set_message(format!(
                "{} (chunk {}/{})",
                progress.file_name,
                progress.chunk_index + 1,
                progress.total_chunks
            ));
        }) as ck_index::DetailedProgressCallback);

        let stats = match ck_index::smart_update_index_with_detailed_progress(
            &self.path,
            self.force_rebuild,
            progress_callback,
            detailed_callback,
            true,
            &self.exclude_patterns,
            Some(model_name),
        ).await {
            Ok(stats) => stats,
            Err(e) => anyhow::bail!("Indexing failed: {}", e),
        };

        overall_pb.finish_and_clear();
        file_pb.finish_and_clear();

        status.success(&format!(
            "✅ Indexed {} files",
            stats.files_indexed
        ));

        Ok(())
    }

    fn validate(&self) -> AnyhowResult<()> {
        if let Some(ref model) = self.model {
            if !ck_models::is_valid_model(model) {
                anyhow::bail!(
                    "❌ Model '{}' not found\n📋 Available models: {}",
                    model,
                    ck_models::get_valid_models().join(", ")
                );
            }
        }
        Ok(())
    }
}