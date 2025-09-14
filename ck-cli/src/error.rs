use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum CkError {
    IndexingFailed {
        path: PathBuf,
        reason: String,
        suggestion: Option<String>,
    },
    ModelDownloadFailed {
        model: String,
        reason: String,
        offline_fallback: Option<String>,
    },
    ModelNotFound {
        model: String,
        available_models: Vec<String>,
    },
    InvalidConfiguration {
        setting: String,
        value: String,
        expected: String,
    },
    FileAccessError {
        path: PathBuf,
        operation: String,
        reason: String,
    },
    NetworkError {
        operation: String,
        retry_possible: bool,
        fallback: Option<String>,
    },
}

impl fmt::Display for CkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CkError::IndexingFailed { path, reason, suggestion } => {
                write!(f, "❌ Indexing failed for {}: {}", path.display(), reason)?;
                if let Some(sugg) = suggestion {
                    write!(f, "\n💡 Suggestion: {}", sugg)?;
                }
                Ok(())
            }
            CkError::ModelDownloadFailed { model, reason, offline_fallback } => {
                write!(f, "❌ Failed to download model '{}': {}", model, reason)?;
                if let Some(fallback) = offline_fallback {
                    write!(f, "\n💡 Offline fallback: {}", fallback)?;
                }
                Ok(())
            }
            CkError::ModelNotFound { model, available_models } => {
                write!(f, "❌ Model '{}' not found", model)?;
                if !available_models.is_empty() {
                    write!(f, "\n📋 Available models:\n")?;
                    for m in available_models {
                        write!(f, "  • {}\n", m)?;
                    }
                }
                Ok(())
            }
            CkError::InvalidConfiguration { setting, value, expected } => {
                write!(
                    f,
                    "❌ Invalid configuration: {} = '{}'\n📋 Expected: {}",
                    setting, value, expected
                )
            }
            CkError::FileAccessError { path, operation, reason } => {
                write!(
                    f,
                    "❌ Cannot {} file {}: {}",
                    operation,
                    path.display(),
                    reason
                )
            }
            CkError::NetworkError { operation, retry_possible, fallback } => {
                write!(f, "❌ Network error during {}", operation)?;
                if *retry_possible {
                    write!(f, "\n🔄 Retry with: ck --retry-downloads")?;
                }
                if let Some(fb) = fallback {
                    write!(f, "\n💡 Fallback: {}", fb)?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for CkError {}

pub type Result<T> = std::result::Result<T, CkError>;

pub trait ErrorContext<T> {
    fn context_path(self, path: &PathBuf, operation: &str) -> Result<T>;
    fn context_model(self, model: &str) -> Result<T>;
    fn with_suggestion(self, suggestion: String) -> Result<T>;
}

impl<T, E> ErrorContext<T> for std::result::Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn context_path(self, path: &PathBuf, operation: &str) -> Result<T> {
        self.map_err(|e| CkError::FileAccessError {
            path: path.clone(),
            operation: operation.to_string(),
            reason: e.to_string(),
        })
    }

    fn context_model(self, model: &str) -> Result<T> {
        self.map_err(|e| CkError::ModelDownloadFailed {
            model: model.to_string(),
            reason: e.to_string(),
            offline_fallback: Some(format!(
                "Use --offline or pre-download to ~/.cache/ck/models/"
            )),
        })
    }

    fn with_suggestion(self, suggestion: String) -> Result<T> {
        self.map_err(|e| {
            if let Ok(ck_err) = e.downcast::<CkError>() {
                match ck_err {
                    CkError::IndexingFailed { path, reason, .. } => CkError::IndexingFailed {
                        path,
                        reason,
                        suggestion: Some(suggestion),
                    },
                    other => other,
                }
            } else {
                CkError::IndexingFailed {
                    path: PathBuf::from("."),
                    reason: e.to_string(),
                    suggestion: Some(suggestion),
                }
            }
        })
    }
}