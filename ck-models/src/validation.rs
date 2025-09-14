pub const VALID_MODELS: &[&str] = &[
    "BAAI/bge-small-en-v1.5",
    "nomic-embed-text-v1.5",
    "jina-embeddings-v2-base-code",
    "sentence-transformers/all-MiniLM-L6-v2",
    "BAAI/bge-base-en-v1.5",
    "BAAI/bge-large-en-v1.5",
];

pub fn is_valid_model(model: &str) -> bool {
    VALID_MODELS.contains(&model)
}

pub fn get_valid_models() -> Vec<String> {
    VALID_MODELS.iter().map(|s| s.to_string()).collect()
}
