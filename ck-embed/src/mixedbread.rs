use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use hf_hub::{Repo, RepoType, api::sync::ApiBuilder};
use ndarray::{Array2, ArrayView, ArrayViewD, Axis, Ix1, Ix2, Ix3};
use ort::session::{Session, builder::GraphOptimizationLevel};
use ort::value::Value;
use tokenizers::{EncodeInput, Tokenizer};

use crate::{
    Embedder, ModelDownloadCallback, model_cache_root,
    reranker::{RerankModelDownloadCallback, RerankResult, Reranker},
};
use ck_models::{ModelConfig, RerankModelConfig};

const EMBED_TOKENIZER_PATH: &str = "tokenizer.json";
const EMBED_MODEL_PATH: &str = "onnx/model_quantized.onnx";
const RERANK_TOKENIZER_PATH: &str = "tokenizer.json";
const RERANK_MODEL_PATH: &str = "onnx/model_quantized.onnx";

pub struct MixedbreadEmbedder {
    session: Session,
    tokenizer: Tokenizer,
    dim: usize,
    max_length: usize,
    model_name: String,
    requires_token_type_ids: bool,
}

impl MixedbreadEmbedder {
    pub fn new(
        config: &ModelConfig,
        progress_callback: Option<ModelDownloadCallback>,
    ) -> Result<Self> {
        if let Some(cb) = progress_callback.as_ref() {
            cb(&format!(
                "Downloading Mixedbread embedding model ({}) if needed...",
                config.name
            ));
        }

        let (model_path, tokenizer_path) =
            download_assets(&config.name, EMBED_MODEL_PATH, EMBED_TOKENIZER_PATH)?;

        if let Some(cb) = progress_callback.as_ref() {
            cb("Loading Mixedbread embedder session...");
        }

        let session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(num_cpus::get().max(1))?
            .commit_from_file(&model_path)?;

        let tokenizer =
            Tokenizer::from_file(tokenizer_path).map_err(|e| anyhow!("Tokenizer error: {e}"))?;

        let requires_token_type_ids = session
            .inputs()
            .iter()
            .any(|input| input.name() == "token_type_ids");

        Ok(Self {
            session,
            tokenizer,
            dim: config.dimensions,
            max_length: config.max_tokens,
            model_name: config.name.clone(),
            requires_token_type_ids,
        })
    }

    #[allow(clippy::type_complexity)]
    fn build_inputs(
        &self,
        texts: &[String],
    ) -> Result<(Array2<i64>, Array2<i64>, Option<Array2<i64>>)> {
        let mut encodings = Vec::with_capacity(texts.len());
        for text in texts {
            let encoding = self
                .tokenizer
                .encode(text.as_str(), true)
                .map_err(|e| anyhow!("Tokenizer encode failed: {e}"))?;
            encodings.push(encoding);
        }

        let seq_len = encodings
            .iter()
            .map(|encoding| encoding.len())
            .max()
            .unwrap_or(1)
            .min(self.max_length)
            .max(1);

        let batch = encodings.len();
        let mut input_ids = vec![0i64; batch * seq_len];
        let mut attention_mask = vec![0i64; batch * seq_len];
        let mut token_types = if self.requires_token_type_ids {
            Some(vec![0i64; batch * seq_len])
        } else {
            None
        };

        for (row, encoding) in encodings.iter().enumerate() {
            let ids = encoding.get_ids();
            let mask = encoding.get_attention_mask();
            let type_ids = encoding.get_type_ids();
            let len = ids.len().min(seq_len);

            let row_offset = row * seq_len;
            for idx in 0..len {
                input_ids[row_offset + idx] = ids[idx] as i64;
                attention_mask[row_offset + idx] = mask[idx] as i64;
            }

            if let Some(ref mut token_types_buf) = token_types
                && !type_ids.is_empty()
            {
                for idx in 0..len {
                    token_types_buf[row_offset + idx] = type_ids[idx] as i64;
                }
            }
        }

        let token_type_array =
            token_types.map(|buf| Array2::from_shape_vec((batch, seq_len), buf).unwrap());

        Ok((
            Array2::from_shape_vec((batch, seq_len), input_ids)
                .expect("validated dimensions for input ids"),
            Array2::from_shape_vec((batch, seq_len), attention_mask)
                .expect("validated dimensions for attention mask"),
            token_type_array,
        ))
    }

    fn normalize(rows: ArrayViewD<'_, f32>, dim: usize) -> Result<Vec<Vec<f32>>> {
        let ndim = rows.ndim();
        match ndim {
            2 => {
                let view = rows.into_dimensionality::<Ix2>()?;
                Ok(view
                    .rows()
                    .into_iter()
                    .map(|row| normalize_row(row, dim))
                    .collect())
            }
            3 => {
                let view = rows.into_dimensionality::<Ix3>()?;
                Ok(view
                    .outer_iter()
                    .map(|matrix| normalize_row(matrix.index_axis(Axis(0), 0), dim))
                    .collect())
            }
            other => Err(anyhow!("Unexpected embedding tensor rank: {other}")),
        }
    }
}

impl Embedder for MixedbreadEmbedder {
    fn id(&self) -> &'static str {
        "mixedbread"
    }

    fn dim(&self) -> usize {
        self.dim
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let (input_ids, attention_mask, token_types) = self.build_inputs(texts)?;

        let outputs = if self.requires_token_type_ids {
            let token_types = token_types.expect("token type ids required but missing");
            self.session.run(ort::inputs![
                Value::from_array(input_ids)?,
                Value::from_array(attention_mask)?,
                Value::from_array(token_types)?
            ])?
        } else {
            self.session.run(ort::inputs![
                Value::from_array(input_ids)?,
                Value::from_array(attention_mask)?
            ])?
        };

        let embedding_tensor = outputs[0]
            .try_extract_array::<f32>()
            .context("Failed to extract embedding tensor")?;

        Self::normalize(embedding_tensor, self.dim)
    }
}

pub struct MixedbreadReranker {
    session: Session,
    tokenizer: Tokenizer,
    max_length: usize,
    requires_token_type_ids: bool,
}

impl MixedbreadReranker {
    pub fn new(
        config: &RerankModelConfig,
        progress_callback: Option<RerankModelDownloadCallback>,
    ) -> Result<Self> {
        if let Some(cb) = progress_callback.as_ref() {
            cb(&format!(
                "Downloading Mixedbread reranker model ({}) if needed...",
                config.name
            ));
        }

        let (model_path, tokenizer_path) =
            download_assets(&config.name, RERANK_MODEL_PATH, RERANK_TOKENIZER_PATH)?;

        if let Some(cb) = progress_callback.as_ref() {
            cb("Loading Mixedbread reranker session...");
        }

        let session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(num_cpus::get().max(1))?
            .commit_from_file(&model_path)?;

        let tokenizer =
            Tokenizer::from_file(tokenizer_path).map_err(|e| anyhow!("Tokenizer error: {e}"))?;

        let requires_token_type_ids = session
            .inputs()
            .iter()
            .any(|input| input.name() == "token_type_ids");

        Ok(Self {
            session,
            tokenizer,
            max_length: 512,
            requires_token_type_ids,
        })
    }

    #[allow(clippy::type_complexity)]
    fn build_inputs(
        &self,
        query: &str,
        documents: &[String],
    ) -> Result<(Array2<i64>, Array2<i64>, Option<Array2<i64>>)> {
        let mut encodings = Vec::with_capacity(documents.len());
        for doc in documents {
            let encoding = self
                .tokenizer
                .encode(EncodeInput::Dual(query.into(), doc.as_str().into()), true)
                .map_err(|e| anyhow!("Tokenizer encode failed: {e}"))?;
            encodings.push(encoding);
        }

        let seq_len = encodings
            .iter()
            .map(|encoding| encoding.len())
            .max()
            .unwrap_or(1)
            .min(self.max_length)
            .max(1);

        let batch = encodings.len();
        let mut input_ids = vec![0i64; batch * seq_len];
        let mut attention_mask = vec![0i64; batch * seq_len];
        let mut token_types = if self.requires_token_type_ids {
            Some(vec![0i64; batch * seq_len])
        } else {
            None
        };

        for (row, encoding) in encodings.iter().enumerate() {
            let ids = encoding.get_ids();
            let mask = encoding.get_attention_mask();
            let type_ids = encoding.get_type_ids();
            let len = ids.len().min(seq_len);
            let offset = row * seq_len;

            for idx in 0..len {
                input_ids[offset + idx] = ids[idx] as i64;
                attention_mask[offset + idx] = mask[idx] as i64;
            }

            if let Some(ref mut token_types_buf) = token_types
                && !type_ids.is_empty()
            {
                for idx in 0..len {
                    token_types_buf[offset + idx] = type_ids[idx] as i64;
                }
            }
        }

        let token_type_array =
            token_types.map(|buf| Array2::from_shape_vec((batch, seq_len), buf).unwrap());

        Ok((
            Array2::from_shape_vec((batch, seq_len), input_ids)
                .expect("validated dimensions for input ids"),
            Array2::from_shape_vec((batch, seq_len), attention_mask)
                .expect("validated dimensions for attention mask"),
            token_type_array,
        ))
    }
}

impl Reranker for MixedbreadReranker {
    fn id(&self) -> &'static str {
        "mixedbread_reranker"
    }

    fn rerank(&mut self, query: &str, documents: &[String]) -> Result<Vec<RerankResult>> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }

        let (input_ids, attention_mask, token_types) = self.build_inputs(query, documents)?;

        let outputs = if self.requires_token_type_ids {
            let token_types = token_types.expect("token type ids required but missing");
            self.session.run(ort::inputs![
                Value::from_array(input_ids)?,
                Value::from_array(attention_mask)?,
                Value::from_array(token_types)?
            ])?
        } else {
            self.session.run(ort::inputs![
                Value::from_array(input_ids)?,
                Value::from_array(attention_mask)?
            ])?
        };

        let logits = outputs[0]
            .try_extract_array::<f32>()
            .context("Failed to extract reranker logits")?
            .into_dimensionality::<Ix2>()?;

        let mut results = Vec::with_capacity(documents.len());
        for (i, row) in logits.rows().into_iter().enumerate() {
            let logit = row
                .get(0)
                .copied()
                .unwrap_or_else(|| row.iter().copied().next().unwrap_or(0.0));
            let score = 1.0 / (1.0 + (-logit).exp());
            results.push(RerankResult {
                query: query.to_string(),
                document: documents[i].clone(),
                score,
            });
        }

        Ok(results)
    }
}

fn normalize_row(row: ArrayView<'_, f32, Ix1>, dim: usize) -> Vec<f32> {
    let take = row.len().min(dim);
    let mut values = vec![0f32; dim];
    let mut norm = 0.0;
    for (idx, value) in row.iter().take(take).enumerate() {
        values[idx] = *value;
        norm += value * value;
    }

    if norm > 0.0 {
        let inv = norm.sqrt().recip();
        for value in values.iter_mut().take(take) {
            *value *= inv;
        }
    }

    values
}

fn download_assets(
    model_id: &str,
    model_path: &str,
    tokenizer_path: &str,
) -> Result<(PathBuf, PathBuf)> {
    let cache_dir = model_cache_root()?;
    std::fs::create_dir_all(&cache_dir)?;

    let api = ApiBuilder::new()
        .with_cache_dir(cache_dir)
        .build()
        .context("Failed to initialize Hugging Face Hub client")?;

    let repo = Repo::with_revision(model_id.to_string(), RepoType::Model, "main".to_string());
    let tokenizer = api
        .repo(Repo::with_revision(
            model_id.to_string(),
            RepoType::Model,
            "main".to_string(),
        ))
        .get(tokenizer_path)
        .with_context(|| format!("Failed to download tokenizer for {model_id}"))?;
    let model = api
        .repo(repo)
        .get(model_path)
        .with_context(|| format!("Failed to download ONNX model for {model_id}"))?;

    Ok((model, tokenizer))
}
