use super::{Command, CommandContext};
use crate::error::{CkError, ErrorContext, Result};
use crate::progress::StatusReporter;
use anyhow::Result as AnyhowResult;
use console::style;
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

    async fn download_model_with_retry(&self, model_name: &str) -> Result<()> {
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

        Err(CkError::ModelDownloadFailed {
            model: model_name.to_string(),
            reason: last_error.map(|e| e.to_string()).unwrap_or_else(|| "Unknown error".to_string()),
            offline_fallback: Some(format!(
                "Pre-download the model manually or use --offline mode with cached models"
            )),
        })
    }

    async fn try_download_model(&self, model_name: &str) -> Result<()> {
        let status = StatusReporter::new(self.context.verbose);

        let progress_callback = if !self.context.no_progress {
            Some(Box::new(move |msg: &str| {
                status.info(msg);
            }) as ck_embed::ModelDownloadCallback)
        } else {
            None
        };

        ck_embed::create_embedder_with_progress(Some(model_name), progress_callback)
            .context_model(model_name)?;

        Ok(())
    }

    async fn validate_prerequisites(&self) -> Result<()> {
        if !self.path.exists() {
            return Err(CkError::FileAccessError {
                path: self.path.clone(),
                operation: "index".to_string(),
                reason: "Path does not exist".to_string(),
            });
        }

        if !self.path.is_dir() && !self.path.is_file() {
            return Err(CkError::FileAccessError {
                path: self.path.clone(),
                operation: "index".to_string(),
                reason: "Path is neither a file nor a directory".to_string(),
            });
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

        let stats = ck_index::smart_update_index_with_detailed_progress(
            &self.path,
            self.force_rebuild,
            progress_callback,
            detailed_callback,
            true,
            &self.exclude_patterns,
            Some(model_name),
        ).map_err(|e| CkError::IndexingFailed {
            path: self.path.clone(),
            reason: e.to_string(),
            suggestion: Some("Try running with --force-rebuild or check file permissions".to_string()),
        }).map_err(|e| anyhow::anyhow!(e))?;

        overall_pb.finish_and_clear();
        file_pb.finish_and_clear();

        status.success(&format!(
            "✅ Indexed {} files ({} chunks) in {:.2}s",
            stats.files_indexed,
            stats.chunks_created,
            stats.duration.as_secs_f64()
        ));

        if stats.files_skipped > 0 {
            status.info(&format!(
                "ℹ️  Skipped {} unchanged files",
                stats.files_skipped
            ));
        }

        Ok(())
    }

    fn validate(&self) -> AnyhowResult<()> {
        if let Some(ref model) = self.model {
            if !ck_models::is_valid_model(model) {
                return Err(anyhow::anyhow!(CkError::ModelNotFound {
                    model: model.clone(),
                    available_models: ck_models::get_valid_models(),
                }));
            }
        }
        Ok(())
    }
}