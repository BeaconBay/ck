use anyhow::Result;
use ck_core::Span;
use serde::{Deserialize, Serialize};

/// Import token estimation from ck-embed
pub use ck_embed::TokenEstimator;

/// Fallback to estimation if precise tokenization fails
fn estimate_tokens(text: &str) -> usize {
    TokenEstimator::estimate_tokens(text)
}

/// Get model-specific chunk configuration (target_tokens, overlap_tokens)
/// Balanced for precision vs context - larger models can handle bigger chunks but not too big
pub fn get_model_chunk_config(model_name: Option<&str>) -> (usize, usize) {
    let model = model_name.unwrap_or("nomic-embed-text-v1.5");

    match model {
        // Small models - keep chunks smaller for better precision
        "BAAI/bge-small-en-v1.5" | "sentence-transformers/all-MiniLM-L6-v2" => {
            (400, 80) // 400 tokens target, 80 token overlap (~20%)
        }

        // Large context models - can use bigger chunks while preserving precision
        // Sweet spot: enough context to be meaningful, small enough to be precise
        "nomic-embed-text-v1" | "nomic-embed-text-v1.5" | "jina-embeddings-v2-base-code" => {
            (1024, 200) // 1024 tokens target, 200 token overlap (~20%) - good balance
        }

        // BGE variants - stick to smaller for precision
        "BAAI/bge-base-en-v1.5" | "BAAI/bge-large-en-v1.5" => {
            (400, 80) // 400 tokens target, 80 token overlap (~20%)
        }

        // Default to large model config since nomic-v1.5 is default
        _ => (1024, 200), // Good balance of context vs precision
    }
}

/// Information about chunk striding for large chunks that exceed token limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrideInfo {
    /// Unique ID for the original chunk before striding
    pub original_chunk_id: String,
    /// Index of this stride (0-based)
    pub stride_index: usize,
    /// Total number of strides for the original chunk
    pub total_strides: usize,
    /// Byte offset where overlap with previous stride begins
    pub overlap_start: usize,
    /// Byte offset where overlap with next stride ends
    pub overlap_end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub span: Span,
    pub text: String,
    pub chunk_type: ChunkType,
    /// Stride information if this chunk was created by striding a larger chunk
    pub stride_info: Option<StrideInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChunkType {
    Text,
    Function,
    Class,
    Method,
    Module,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseableLanguage {
    Python,
    TypeScript,
    JavaScript,
    Haskell,
    Rust,
    Ruby,
    Go,
    CSharp,
    Zig,
}

impl std::fmt::Display for ParseableLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            ParseableLanguage::Python => "python",
            ParseableLanguage::TypeScript => "typescript",
            ParseableLanguage::JavaScript => "javascript",
            ParseableLanguage::Haskell => "haskell",
            ParseableLanguage::Rust => "rust",
            ParseableLanguage::Ruby => "ruby",
            ParseableLanguage::Go => "go",
            ParseableLanguage::CSharp => "csharp",
            ParseableLanguage::Zig => "zig",
        };
        write!(f, "{}", name)
    }
}

impl TryFrom<ck_core::Language> for ParseableLanguage {
    type Error = anyhow::Error;

    fn try_from(lang: ck_core::Language) -> Result<Self, Self::Error> {
        match lang {
            ck_core::Language::Python => Ok(ParseableLanguage::Python),
            ck_core::Language::TypeScript => Ok(ParseableLanguage::TypeScript),
            ck_core::Language::JavaScript => Ok(ParseableLanguage::JavaScript),
            ck_core::Language::Haskell => Ok(ParseableLanguage::Haskell),
            ck_core::Language::Rust => Ok(ParseableLanguage::Rust),
            ck_core::Language::Ruby => Ok(ParseableLanguage::Ruby),
            ck_core::Language::Go => Ok(ParseableLanguage::Go),
            ck_core::Language::CSharp => Ok(ParseableLanguage::CSharp),
            ck_core::Language::Zig => Ok(ParseableLanguage::Zig),
            _ => Err(anyhow::anyhow!(
                "Language {:?} is not supported for parsing",
                lang
            )),
        }
    }
}

pub fn chunk_text(text: &str, language: Option<ck_core::Language>) -> Result<Vec<Chunk>> {
    chunk_text_with_config(text, language, &ChunkConfig::default())
}

/// Configuration for chunking behavior
#[derive(Debug, Clone)]
pub struct ChunkConfig {
    /// Maximum tokens per chunk (for striding)
    pub max_tokens: usize,
    /// Overlap size for striding (in tokens)
    pub stride_overlap: usize,
    /// Enable striding for chunks that exceed max_tokens
    pub enable_striding: bool,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            max_tokens: 8192,     // Default to Nomic model limit
            stride_overlap: 1024, // 12.5% overlap
            enable_striding: true,
        }
    }
}

/// New function that accepts model name for model-specific chunking
pub fn chunk_text_with_model(
    text: &str,
    language: Option<ck_core::Language>,
    model_name: Option<&str>,
) -> Result<Vec<Chunk>> {
    let (target_tokens, overlap_tokens) = get_model_chunk_config(model_name);

    // Create a config based on model-specific parameters
    let config = ChunkConfig {
        max_tokens: target_tokens,
        stride_overlap: overlap_tokens,
        enable_striding: true,
    };

    chunk_text_with_config_and_model(text, language, &config, model_name)
}

pub fn chunk_text_with_config(
    text: &str,
    language: Option<ck_core::Language>,
    config: &ChunkConfig,
) -> Result<Vec<Chunk>> {
    chunk_text_with_config_and_model(text, language, config, None)
}

fn chunk_text_with_config_and_model(
    text: &str,
    language: Option<ck_core::Language>,
    config: &ChunkConfig,
    model_name: Option<&str>,
) -> Result<Vec<Chunk>> {
    tracing::debug!(
        "Chunking text with language: {:?}, length: {} chars, config: {:?}",
        language,
        text.len(),
        config
    );

    let result = match language.map(ParseableLanguage::try_from) {
        Some(Ok(lang)) => {
            tracing::debug!("Using {} tree-sitter parser", lang);
            chunk_language_with_model(text, lang, model_name)
        }
        Some(Err(_)) => {
            tracing::debug!("Language not supported for parsing, using generic chunking strategy");
            chunk_generic_with_token_config(text, model_name)
        }
        None => {
            tracing::debug!("Using generic chunking strategy");
            chunk_generic_with_token_config(text, model_name)
        }
    };

    let mut chunks = result?;

    // Apply striding if enabled and necessary
    if config.enable_striding {
        chunks = apply_striding(chunks, config)?;
    }

    tracing::debug!("Successfully created {} final chunks", chunks.len());
    Ok(chunks)
}

fn chunk_generic(text: &str) -> Result<Vec<Chunk>> {
    chunk_generic_with_token_config(text, None)
}

fn chunk_generic_with_token_config(text: &str, model_name: Option<&str>) -> Result<Vec<Chunk>> {
    let mut chunks = Vec::new();
    let lines: Vec<&str> = text.lines().collect();

    // Get model-specific optimal chunk size in tokens
    let (target_tokens, overlap_tokens) = get_model_chunk_config(model_name);

    // Convert token targets to approximate line counts
    // This is a rough heuristic - we'll validate with actual token counting
    let avg_tokens_per_line = 10.0; // Rough estimate for code
    let target_lines = ((target_tokens as f32) / avg_tokens_per_line) as usize;
    let overlap_lines = ((overlap_tokens as f32) / avg_tokens_per_line) as usize;

    let chunk_size = target_lines.max(5); // Minimum 5 lines
    let overlap = overlap_lines.max(1); // Minimum 1 line overlap

    // Pre-compute cumulative byte offsets for O(1) lookup, accounting for different line endings
    let mut line_byte_offsets = Vec::with_capacity(lines.len() + 1);
    line_byte_offsets.push(0);
    let mut cumulative_offset = 0;
    let mut byte_pos = 0;

    for line in lines.iter() {
        cumulative_offset += line.len();

        // Find the actual line ending length in the original text
        let line_end_pos = byte_pos + line.len();
        let newline_len = if line_end_pos < text.len() && text.as_bytes()[line_end_pos] == b'\r' {
            if line_end_pos + 1 < text.len() && text.as_bytes()[line_end_pos + 1] == b'\n' {
                2 // CRLF
            } else {
                1 // CR only (old Mac)
            }
        } else if line_end_pos < text.len() && text.as_bytes()[line_end_pos] == b'\n' {
            1 // LF only (Unix)
        } else {
            0 // No newline at this position (could be last line without newline)
        };

        cumulative_offset += newline_len;
        byte_pos = cumulative_offset;
        line_byte_offsets.push(cumulative_offset);
    }

    let mut i = 0;
    while i < lines.len() {
        let end = (i + chunk_size).min(lines.len());
        let chunk_lines = &lines[i..end];
        let chunk_text = chunk_lines.join("\n");

        let byte_start = line_byte_offsets[i];
        let byte_end = line_byte_offsets[end];

        chunks.push(Chunk {
            span: Span {
                byte_start,
                byte_end,
                line_start: i + 1,
                line_end: end,
            },
            text: chunk_text,
            chunk_type: ChunkType::Text,
            stride_info: None,
        });

        i += chunk_size - overlap;
        if i >= lines.len() {
            break;
        }
    }

    Ok(chunks)
}

fn chunk_language(text: &str, language: ParseableLanguage) -> Result<Vec<Chunk>> {
    let mut parser = tree_sitter::Parser::new();

    match language {
        ParseableLanguage::Python => parser.set_language(&tree_sitter_python::LANGUAGE.into())?,
        ParseableLanguage::TypeScript | ParseableLanguage::JavaScript => {
            parser.set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())?
        }
        ParseableLanguage::Haskell => parser.set_language(&tree_sitter_haskell::LANGUAGE.into())?,
        ParseableLanguage::Rust => parser.set_language(&tree_sitter_rust::LANGUAGE.into())?,
        ParseableLanguage::Ruby => parser.set_language(&tree_sitter_ruby::LANGUAGE.into())?,
        ParseableLanguage::Go => parser.set_language(&tree_sitter_go::LANGUAGE.into())?,
        ParseableLanguage::CSharp => parser.set_language(&tree_sitter_c_sharp::LANGUAGE.into())?,
        ParseableLanguage::Zig => parser.set_language(&tree_sitter_zig::LANGUAGE.into())?,
    }

    let tree = parser
        .parse(text, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse {} code", language))?;

    let mut chunks = Vec::new();
    let mut cursor = tree.root_node().walk();

    extract_code_chunks(&mut cursor, text, &mut chunks, language);

    if chunks.is_empty() {
        return chunk_generic(text);
    }

    // Post-process Haskell chunks to merge function equations
    if language == ParseableLanguage::Haskell {
        chunks = merge_haskell_functions(chunks, text);
    }

    Ok(chunks)
}

/// Merge Haskell function equations that belong to the same function definition
fn merge_haskell_functions(chunks: Vec<Chunk>, source: &str) -> Vec<Chunk> {
    if chunks.is_empty() {
        return chunks;
    }

    let mut merged = Vec::new();
    let mut i = 0;

    while i < chunks.len() {
        let chunk = &chunks[i];

        // Skip chunks that are just fragments or comments
        let trimmed = chunk.text.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("--")
            || trimmed.starts_with("{-")
            || !chunk.text.contains(|c: char| c.is_alphanumeric())
        {
            i += 1;
            continue;
        }

        // Extract function name from the chunk text
        // Check if it's a signature first (contains ::)
        let is_signature = chunk.text.contains("::");
        let function_name = if is_signature {
            // For signatures like "factorial :: Integer -> Integer", extract "factorial"
            chunk.text.split("::").next()
                .and_then(|s| s.trim().split_whitespace().next())
                .map(|s| s.to_string())
        } else {
            extract_haskell_function_name(&chunk.text)
        };

        if function_name.is_none() {
            // Not a function (might be data, newtype, etc.), keep as-is
            merged.push(chunk.clone());
            i += 1;
            continue;
        }

        let name = function_name.unwrap();
        let group_start = chunk.span.byte_start;
        let mut group_end = chunk.span.byte_end;
        let line_start = chunk.span.line_start;
        let mut line_end = chunk.span.line_end;

        // Look ahead for function equations with the same name
        let mut j = i + 1;
        while j < chunks.len() {
            let next_chunk = &chunks[j];

            // Skip comments
            let next_trimmed = next_chunk.text.trim();
            if next_trimmed.starts_with("--") || next_trimmed.starts_with("{-") {
                j += 1;
                continue;
            }

            let next_is_signature = next_chunk.text.contains("::");
            let next_name = if next_is_signature {
                next_chunk.text.split("::").next()
                    .and_then(|s| s.trim().split_whitespace().next())
                    .map(|s| s.to_string())
            } else {
                extract_haskell_function_name(&next_chunk.text)
            };

            if next_name == Some(name.clone()) {
                // Extend the group to include this equation
                group_end = next_chunk.span.byte_end;
                line_end = next_chunk.span.line_end;
                j += 1;
            } else {
                break;
            }
        }

        // Create merged chunk
        let merged_text = source
            .get(group_start..group_end)
            .unwrap_or("")
            .to_string();

        merged.push(Chunk {
            span: Span {
                byte_start: group_start,
                byte_end: group_end,
                line_start,
                line_end,
            },
            text: merged_text,
            chunk_type: ChunkType::Function,
            stride_info: None,
        });

        i = j; // Skip past all merged chunks
    }

    merged
}

/// Extract the function name from a Haskell function equation
fn extract_haskell_function_name(text: &str) -> Option<String> {
    // Haskell function equations start with the function name followed by patterns or =
    // Examples: "factorial 0 = 1", "map f [] = []"
    let trimmed = text.trim();

    // Find the first word (function name)
    let first_word = trimmed
        .split_whitespace()
        .next()?
        .trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '\'');

    // Validate it's a valid Haskell identifier (starts with lowercase or underscore)
    if first_word.is_empty() {
        return None;
    }

    let first_char = first_word.chars().next()?;
    if first_char.is_lowercase() || first_char == '_' {
        Some(first_word.to_string())
    } else {
        None
    }
}

fn chunk_language_with_model(
    text: &str,
    language: ParseableLanguage,
    _model_name: Option<&str>,
) -> Result<Vec<Chunk>> {
    // For now, language-based chunking doesn't need model-specific behavior
    // since it's based on semantic code boundaries rather than token counts
    // We could potentially optimize this in the future by validating chunk token counts
    chunk_language(text, language)
}

fn extract_code_chunks(
    cursor: &mut tree_sitter::TreeCursor,
    source: &str,
    chunks: &mut Vec<Chunk>,
    language: ParseableLanguage,
) {
    let node = cursor.node();

    // For Haskell: skip "function" nodes that are nested anywhere inside "signature" nodes
    // (these are type expressions, not actual function definitions)
    let should_skip = if language == ParseableLanguage::Haskell && node.kind() == "function" {
        // Walk up parent chain to check if we're inside a signature
        let mut current = node.parent();
        while let Some(parent) = current {
            if parent.kind() == "signature" {
                return; // Skip this node and don't recurse
            }
            current = parent.parent();
        }
        false
    } else {
        false
    };

    if !should_skip {
        if let Some(initial_chunk_type) = chunk_type_for_node(language, &node) {
            if let Some(chunk) = build_chunk(node, source, initial_chunk_type, language) {
                let is_duplicate = chunks.iter().any(|existing| {
                    existing.span.byte_start == chunk.span.byte_start
                        && existing.span.byte_end == chunk.span.byte_end
                });

                if !is_duplicate {
                    chunks.push(chunk);
                }
            }
        }
    }

    // For Haskell signatures: don't recurse into children (they're just type expressions)
    let should_recurse = if language == ParseableLanguage::Haskell && node.kind() == "signature" {
        false
    } else {
        true
    };

    if should_recurse && cursor.goto_first_child() {
        loop {
            extract_code_chunks(cursor, source, chunks, language);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

fn chunk_type_for_node(
    language: ParseableLanguage,
    node: &tree_sitter::Node<'_>,
) -> Option<ChunkType> {
    let kind = node.kind();

    let supported = match language {
        ParseableLanguage::Python => matches!(kind, "function_definition" | "class_definition"),
        ParseableLanguage::TypeScript | ParseableLanguage::JavaScript => matches!(
            kind,
            "function_declaration" | "class_declaration" | "method_definition" | "arrow_function"
        ),
        ParseableLanguage::Haskell => matches!(
            kind,
            "function" // Capture function equations
                | "signature" // Capture type signatures (will be merged with functions)
                | "data_type"
                | "newtype"
                | "type_synonym"
                | "type_family"
                | "class"
                | "instance"
        ),
        ParseableLanguage::Rust => matches!(
            kind,
            "function_item" | "impl_item" | "struct_item" | "enum_item" | "trait_item" | "mod_item"
        ),
        ParseableLanguage::Ruby => {
            matches!(kind, "method" | "class" | "module" | "singleton_method")
        }
        ParseableLanguage::Go => matches!(
            kind,
            "function_declaration"
                | "method_declaration"
                | "type_declaration"
                | "var_declaration"
                | "const_declaration"
        ),
        ParseableLanguage::CSharp => matches!(
            kind,
            "method_declaration"
                | "class_declaration"
                | "interface_declaration"
                | "variable_declaration"
        ),
        ParseableLanguage::Zig => matches!(
            kind,
            "function_declaration"
                | "test_declaration"
                | "variable_declaration"
                | "struct_declaration"
                | "enum_declaration"
                | "union_declaration"
                | "opaque_declaration"
                | "error_set_declaration"
                | "comptime_declaration"
        ),
    };

    if !supported {
        return None;
    }

    match language {
        ParseableLanguage::Go
            if matches!(node.kind(), "var_declaration" | "const_declaration")
                && node.parent().is_some_and(|p| p.kind() == "block") =>
        {
            return None;
        }
        ParseableLanguage::CSharp if node.kind() == "variable_declaration" => {
            if !is_csharp_field_like(node.clone()) {
                return None;
            }
        }
        _ => {}
    }

    Some(classify_chunk_kind(kind))
}

fn classify_chunk_kind(kind: &str) -> ChunkType {
    match kind {
        "function_definition"
        | "function_declaration"
        | "arrow_function"
        | "function"
        | "function_item"
        | "def"
        | "defp"
        | "defn"
        | "defn-"
        | "method"
        | "singleton_method" => ChunkType::Function,
        "signature" => ChunkType::Function, // Haskell type signatures will be merged with functions
        "class_definition"
        | "class_declaration"
        | "instance_declaration"
        | "class"
        | "instance"
        | "struct_item"
        | "enum_item"
        | "defstruct"
        | "defrecord"
        | "deftype"
        | "type_declaration"
        | "struct_declaration"
        | "enum_declaration"
        | "union_declaration"
        | "opaque_declaration"
        | "error_set_declaration" => ChunkType::Class,
        "method_definition" | "method_declaration" | "defmacro" => ChunkType::Method,
        "data_type"
        | "newtype"
        | "type_synonym"
        | "type_family"
        | "impl_item"
        | "trait_item"
        | "mod_item"
        | "defmodule"
        | "module"
        | "defprotocol"
        | "interface_declaration"
        | "ns"
        | "var_declaration"
        | "const_declaration"
        | "variable_declaration"
        | "test_declaration"
        | "comptime_declaration" => ChunkType::Module,
        _ => ChunkType::Text,
    }
}

fn build_chunk(
    node: tree_sitter::Node<'_>,
    source: &str,
    initial_type: ChunkType,
    language: ParseableLanguage,
) -> Option<Chunk> {
    let target_node = adjust_node_for_language(node, language);
    let (byte_start, start_row) = extend_with_leading_trivia(target_node, language, source);

    let byte_end = target_node.end_byte();
    let end_pos = target_node.end_position();

    if byte_start >= byte_end || byte_end > source.len() {
        return None;
    }

    let text = source.get(byte_start..byte_end)?.to_string();

    if text.trim().is_empty() {
        return None;
    }

    let chunk_type = adjust_chunk_type_for_context(target_node, initial_type, language);

    Some(Chunk {
        span: Span {
            byte_start,
            byte_end,
            line_start: start_row + 1,
            line_end: end_pos.row + 1,
        },
        text,
        chunk_type,
        stride_info: None,
    })
}

fn adjust_node_for_language(
    node: tree_sitter::Node<'_>,
    language: ParseableLanguage,
) -> tree_sitter::Node<'_> {
    match language {
        ParseableLanguage::TypeScript | ParseableLanguage::JavaScript => {
            if node.kind() == "arrow_function" {
                return expand_arrow_function_context(node);
            }
            node
        }
        _ => node,
    }
}

fn expand_arrow_function_context(mut node: tree_sitter::Node<'_>) -> tree_sitter::Node<'_> {
    const PARENTS: &[&str] = &[
        "parenthesized_expression",
        "variable_declarator",
        "variable_declaration",
        "lexical_declaration",
        "assignment_expression",
        "expression_statement",
        "public_field_definition",
        "export_statement",
    ];

    while let Some(parent) = node.parent() {
        let kind = parent.kind();
        if PARENTS.contains(&kind) {
            node = parent;
            continue;
        }
        break;
    }

    node
}

fn extend_with_leading_trivia(
    node: tree_sitter::Node<'_>,
    language: ParseableLanguage,
    source: &str,
) -> (usize, usize) {
    let mut start_byte = node.start_byte();
    let mut start_row = node.start_position().row;
    let mut current = node;

    while let Some(prev) = current.prev_sibling() {
        if should_attach_leading_trivia(language, &prev)
            && only_whitespace_between(source, prev.end_byte(), start_byte)
        {
            start_byte = prev.start_byte();
            start_row = prev.start_position().row;
            current = prev;
            continue;
        }
        break;
    }

    (start_byte, start_row)
}

fn should_attach_leading_trivia(language: ParseableLanguage, node: &tree_sitter::Node<'_>) -> bool {
    let kind = node.kind();
    if kind == "comment" {
        return true;
    }

    match language {
        ParseableLanguage::Rust => kind == "attribute_item",
        ParseableLanguage::Python => kind == "decorator",
        ParseableLanguage::TypeScript | ParseableLanguage::JavaScript => kind == "decorator",
        ParseableLanguage::CSharp => matches!(kind, "attribute_list" | "attribute"),
        _ => false,
    }
}

fn only_whitespace_between(source: &str, start: usize, end: usize) -> bool {
    if start >= end || end > source.len() {
        return true;
    }

    source[start..end].chars().all(|c| c.is_whitespace())
}

fn adjust_chunk_type_for_context(
    node: tree_sitter::Node<'_>,
    chunk_type: ChunkType,
    language: ParseableLanguage,
) -> ChunkType {
    if chunk_type != ChunkType::Function {
        return chunk_type;
    }

    if is_method_context(node, language) {
        ChunkType::Method
    } else {
        chunk_type
    }
}

fn is_method_context(node: tree_sitter::Node<'_>, language: ParseableLanguage) -> bool {
    const PYTHON_CONTAINERS: &[&str] = &["class_definition"];
    const TYPESCRIPT_CONTAINERS: &[&str] = &["class_body", "class_declaration"];
    const RUBY_CONTAINERS: &[&str] = &["class", "module"];
    const RUST_CONTAINERS: &[&str] = &["impl_item", "trait_item"];

    match language {
        ParseableLanguage::Python => ancestor_has_kind(node, PYTHON_CONTAINERS),
        ParseableLanguage::TypeScript | ParseableLanguage::JavaScript => {
            ancestor_has_kind(node, TYPESCRIPT_CONTAINERS)
        }
        ParseableLanguage::Ruby => ancestor_has_kind(node, RUBY_CONTAINERS),
        ParseableLanguage::Rust => ancestor_has_kind(node, RUST_CONTAINERS),
        ParseableLanguage::Go => false,
        ParseableLanguage::CSharp => false,
        ParseableLanguage::Haskell => false,
        ParseableLanguage::Zig => false,
    }
}

fn ancestor_has_kind(node: tree_sitter::Node<'_>, kinds: &[&str]) -> bool {
    let mut current = node;
    while let Some(parent) = current.parent() {
        if kinds.contains(&parent.kind()) {
            return true;
        }
        current = parent;
    }
    false
}

fn is_csharp_field_like(node: tree_sitter::Node<'_>) -> bool {
    if let Some(parent) = node.parent() {
        return matches!(
            parent.kind(),
            "field_declaration" | "event_field_declaration"
        );
    }
    false
}

/// Apply striding to chunks that exceed the token limit
fn apply_striding(chunks: Vec<Chunk>, config: &ChunkConfig) -> Result<Vec<Chunk>> {
    let mut result = Vec::new();

    for chunk in chunks {
        let estimated_tokens = estimate_tokens(&chunk.text);

        if estimated_tokens <= config.max_tokens {
            // Chunk fits within limit, no striding needed
            result.push(chunk);
        } else {
            // Chunk exceeds limit, apply striding
            tracing::debug!(
                "Chunk with {} tokens exceeds limit of {}, applying striding",
                estimated_tokens,
                config.max_tokens
            );

            let strided_chunks = stride_large_chunk(chunk, config)?;
            result.extend(strided_chunks);
        }
    }

    Ok(result)
}

/// Create strided chunks from a large chunk that exceeds token limits
fn stride_large_chunk(chunk: Chunk, config: &ChunkConfig) -> Result<Vec<Chunk>> {
    let text = &chunk.text;

    // Early return for empty chunks to avoid divide-by-zero
    if text.is_empty() {
        return Ok(vec![chunk]);
    }

    // Calculate stride parameters in characters (not bytes!)
    // Use a conservative estimate to ensure we stay under token limits
    let char_count = text.chars().count();
    let estimated_tokens = estimate_tokens(text);
    // Guard against zero token estimate to prevent divide-by-zero panic
    let chars_per_token = if estimated_tokens == 0 {
        4.5 // Use default average if estimation fails
    } else {
        char_count as f32 / estimated_tokens as f32
    };
    let window_chars = ((config.max_tokens as f32 * 0.9) * chars_per_token) as usize; // 10% buffer
    let overlap_chars = (config.stride_overlap as f32 * chars_per_token) as usize;
    let stride_chars = window_chars.saturating_sub(overlap_chars);

    if stride_chars == 0 {
        return Err(anyhow::anyhow!("Stride size is too small"));
    }

    // Build char to byte index mapping to handle UTF-8 safely
    let char_byte_indices: Vec<(usize, char)> = text.char_indices().collect();
    // Note: char_count is already calculated above, just reference it here

    let mut strided_chunks = Vec::new();
    let original_chunk_id = format!("{}:{}", chunk.span.byte_start, chunk.span.byte_end);
    let mut start_char_idx = 0;
    let mut stride_index = 0;

    // Calculate total number of strides
    let total_strides = if char_count <= window_chars {
        1
    } else {
        ((char_count - overlap_chars) as f32 / stride_chars as f32).ceil() as usize
    };

    while start_char_idx < char_count {
        let end_char_idx = (start_char_idx + window_chars).min(char_count);

        // Get byte positions from char indices
        let start_byte_pos = char_byte_indices[start_char_idx].0;
        let end_byte_pos = if end_char_idx < char_count {
            char_byte_indices[end_char_idx].0
        } else {
            text.len()
        };

        let stride_text = &text[start_byte_pos..end_byte_pos];

        // Calculate overlap information
        let overlap_start = if stride_index > 0 { overlap_chars } else { 0 };
        let overlap_end = if end_char_idx < char_count {
            overlap_chars
        } else {
            0
        };

        // Calculate span for this stride
        let byte_offset_start = chunk.span.byte_start + start_byte_pos;
        let byte_offset_end = chunk.span.byte_start + end_byte_pos;

        // Estimate line numbers (approximate)
        let text_before_start = &text[..start_byte_pos];
        let line_offset_start = text_before_start.lines().count().saturating_sub(1);
        let stride_lines = stride_text.lines().count();

        let stride_chunk = Chunk {
            span: Span {
                byte_start: byte_offset_start,
                byte_end: byte_offset_end,
                line_start: chunk.span.line_start + line_offset_start,
                // Fix: subtract 1 since stride_lines is a count but line_end should be inclusive
                line_end: chunk.span.line_start
                    + line_offset_start
                    + stride_lines.saturating_sub(1),
            },
            text: stride_text.to_string(),
            chunk_type: chunk.chunk_type.clone(),
            stride_info: Some(StrideInfo {
                original_chunk_id: original_chunk_id.clone(),
                stride_index,
                total_strides,
                overlap_start,
                overlap_end,
            }),
        };

        strided_chunks.push(stride_chunk);

        // Move to next stride
        if end_char_idx >= char_count {
            break;
        }

        start_char_idx += stride_chars;
        stride_index += 1;
    }

    tracing::debug!(
        "Created {} strides from chunk of {} tokens",
        strided_chunks.len(),
        estimate_tokens(text)
    );

    Ok(strided_chunks)
}

// Removed duplicate estimate_tokens function - using the one from ck-embed via TokenEstimator

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_generic_byte_offsets() {
        // Test that byte offsets are calculated correctly using O(n) algorithm
        let text = "line 1\nline 2\nline 3\nline 4\nline 5";
        let chunks = chunk_generic(text).unwrap();

        assert!(!chunks.is_empty());

        // First chunk should start at byte 0
        assert_eq!(chunks[0].span.byte_start, 0);

        // Each chunk's byte_end should match the actual text length
        for chunk in &chunks {
            let expected_len = chunk.text.len();
            let actual_len = chunk.span.byte_end - chunk.span.byte_start;
            assert_eq!(actual_len, expected_len);
        }
    }

    #[test]
    fn test_chunk_generic_large_file_performance() {
        // Create a large text to ensure O(n) performance
        let lines: Vec<String> = (0..1000)
            .map(|i| format!("Line {}: Some content here", i))
            .collect();
        let text = lines.join("\n");

        let start = std::time::Instant::now();
        let chunks = chunk_generic(&text).unwrap();
        let duration = start.elapsed();

        // Should complete quickly even for 1000 lines
        assert!(
            duration.as_millis() < 100,
            "Chunking took too long: {:?}",
            duration
        );
        assert!(!chunks.is_empty());

        // Verify chunks have correct line numbers
        for chunk in &chunks {
            assert!(chunk.span.line_start > 0);
            assert!(chunk.span.line_end >= chunk.span.line_start);
        }
    }

    #[test]
    fn test_chunk_rust() {
        let rust_code = r#"
pub struct Calculator {
    memory: f64,
}

impl Calculator {
    pub fn new() -> Self {
        Calculator { memory: 0.0 }
    }
    
    pub fn add(&mut self, a: f64, b: f64) -> f64 {
        a + b
    }
}

fn main() {
    let calc = Calculator::new();
}

pub mod utils {
    pub fn helper() {}
}
"#;

        let chunks = chunk_language(rust_code, ParseableLanguage::Rust).unwrap();
        assert!(!chunks.is_empty());

        // Should find struct, impl, functions, and module
        let chunk_types: Vec<&ChunkType> = chunks.iter().map(|c| &c.chunk_type).collect();
        assert!(chunk_types.contains(&&ChunkType::Class)); // struct
        assert!(chunk_types.contains(&&ChunkType::Module)); // impl and mod
        assert!(chunk_types.contains(&&ChunkType::Function)); // functions
    }

    #[test]
    fn test_chunk_ruby() {
        let ruby_code = r#"
class Calculator
  def initialize
    @memory = 0.0
  end

  def add(a, b)
    a + b
  end

  def self.class_method
    "class method"
  end

  private

  def private_method
    "private"
  end
end

module Utils
  def self.helper
    "helper"
  end
end

def main
  calc = Calculator.new
end
"#;

        let chunks = chunk_language(ruby_code, ParseableLanguage::Ruby).unwrap();
        assert!(!chunks.is_empty());

        // Should find class, module, and methods
        let chunk_types: Vec<&ChunkType> = chunks.iter().map(|c| &c.chunk_type).collect();
        assert!(chunk_types.contains(&&ChunkType::Class)); // class
        assert!(chunk_types.contains(&&ChunkType::Module)); // module
        assert!(chunk_types.contains(&&ChunkType::Function)); // methods
    }

    #[test]
    fn test_language_detection_fallback() {
        // Test that unknown languages fall back to generic chunking
        let generic_text = "Some text\nwith multiple lines\nto chunk generically";

        let chunks_unknown = chunk_text(generic_text, None).unwrap();
        let chunks_generic = chunk_generic(generic_text).unwrap();

        // Should produce the same result
        assert_eq!(chunks_unknown.len(), chunks_generic.len());
        assert_eq!(chunks_unknown[0].text, chunks_generic[0].text);
    }

    #[test]
    fn test_chunk_go() {
        let go_code = r#"
package main

import "fmt"

const Pi = 3.14159

var memory float64

type Calculator struct {
    memory float64
}

type Operation interface {
    Calculate(a, b float64) float64
}

func NewCalculator() *Calculator {
    return &Calculator{memory: 0.0}
}

func (c *Calculator) Add(a, b float64) float64 {
    return a + b
}

func main() {
    calc := NewCalculator()
}
"#;

        let chunks = chunk_language(go_code, ParseableLanguage::Go).unwrap();
        assert!(!chunks.is_empty());

        // Should find const, var, type declarations, functions, and methods
        let chunk_types: Vec<&ChunkType> = chunks.iter().map(|c| &c.chunk_type).collect();
        assert!(chunk_types.contains(&&ChunkType::Module)); // const and var
        assert!(chunk_types.contains(&&ChunkType::Class)); // struct and interface
        assert!(chunk_types.contains(&&ChunkType::Function)); // functions
        assert!(chunk_types.contains(&&ChunkType::Method)); // methods
    }

    #[test]
    fn test_chunk_typescript_arrow_context() {
        let ts_code = r#"
// Utility function
export const util = () => {
    // comment about util
    return 42;
};

export class Example {
    // leading comment for method
    constructor() {}

    // Another comment
    run = () => {
        return util();
    };
}

const compute = (x: number) => x * 2;
"#;

        let chunks = chunk_language(ts_code, ParseableLanguage::TypeScript).unwrap();

        let util_chunk = chunks
            .iter()
            .find(|chunk| chunk.text.contains("export const util"))
            .expect("Expected chunk for util arrow function");
        assert_eq!(util_chunk.chunk_type, ChunkType::Function);
        assert!(
            util_chunk.text.contains("// Utility function"),
            "expected leading comment to be included"
        );
        assert!(util_chunk.text.contains("export const util ="));

        // The class field arrow function should be classified as a method and include its comment
        let method_chunk = chunks
            .iter()
            .find(|chunk| {
                chunk.chunk_type == ChunkType::Method && chunk.text.contains("run = () =>")
            })
            .expect("Expected chunk for class field arrow function");

        assert_eq!(method_chunk.chunk_type, ChunkType::Method);
        assert!(
            method_chunk.text.contains("// Another comment"),
            "expected inline comment to be included"
        );

        let compute_chunk = chunks
            .iter()
            .find(|chunk| chunk.text.contains("const compute"))
            .expect("Expected chunk for compute arrow function");
        assert_eq!(compute_chunk.chunk_type, ChunkType::Function);
        assert!(
            compute_chunk
                .text
                .contains("const compute = (x: number) => x * 2;")
        );

        // Ensure we don't create bare arrow-expression chunks without context
        assert!(
            chunks
                .iter()
                .all(|chunk| !chunk.text.trim_start().starts_with("() =>"))
        );
        assert!(
            chunks
                .iter()
                .all(|chunk| !chunk.text.trim_start().starts_with("(x: number) =>"))
        );
    }

    #[test]
    fn test_chunk_zig() {
        let zig_code = r#"
const std = @import("std");

const Calculator = struct {
    memory: f64,

    pub fn init() Calculator {
        return Calculator{ .memory = 0.0 };
    }

    pub fn add(self: *Calculator, a: f64, b: f64) f64 {
        const result = a + b;
        self.memory = result;
        return result;
    }
};

const Color = enum {
    Red,
    Green,
    Blue,
};

const Value = union(enum) {
    int: i32,
    float: f64,
};

const Handle = opaque {};

const MathError = error{
    DivisionByZero,
    Overflow,
};

pub fn multiply(a: i32, b: i32) i32 {
    return a * b;
}

pub fn divide(a: i32, b: i32) MathError!i32 {
    if (b == 0) return error.DivisionByZero;
    return @divTrunc(a, b);
}

comptime {
    @compileLog("Compile-time validation");
}

pub fn main() !void {
    var calc = Calculator.init();
    const result = calc.add(2.0, 3.0);
    std.debug.print("Result: {}\n", .{result});
}

test "calculator addition" {
    var calc = Calculator.init();
    const result = calc.add(2.0, 3.0);
    try std.testing.expect(result == 5.0);
}

test "multiply function" {
    const result = multiply(3, 4);
    try std.testing.expect(result == 12);
}
"#;

        let chunks = chunk_language(zig_code, ParseableLanguage::Zig).unwrap();
        assert!(!chunks.is_empty());

        let chunk_types: Vec<&ChunkType> = chunks.iter().map(|c| &c.chunk_type).collect();

        let class_count = chunk_types
            .iter()
            .filter(|&&t| t == &ChunkType::Class)
            .count();
        let function_count = chunk_types
            .iter()
            .filter(|&&t| t == &ChunkType::Function)
            .count();
        let module_count = chunk_types
            .iter()
            .filter(|&&t| t == &ChunkType::Module)
            .count();

        assert!(
            class_count >= 5,
            "Expected at least 5 Class chunks (struct, enum, union, opaque, error set), found {}",
            class_count
        );

        assert!(
            function_count >= 3,
            "Expected at least 3 functions (multiply, divide, main), found {}",
            function_count
        );

        assert!(
            module_count >= 4,
            "Expected at least 4 module-type chunks (const std, comptime, 2 tests), found {}",
            module_count
        );

        assert!(
            chunk_types.contains(&&ChunkType::Class),
            "Expected to find Class chunks"
        );
        assert!(
            chunk_types.contains(&&ChunkType::Function),
            "Expected to find Function chunks"
        );
        assert!(
            chunk_types.contains(&&ChunkType::Module),
            "Expected to find Module chunks"
        );
    }

    #[test]
    fn test_chunk_csharp() {
        let csharp_code = r#"
namespace Calculator;

public interface ICalculator 
{
    double Add(double x, double y);
}

public class Calculator 
{
    public static const double PI = 3.14159;
    private double _memory;

    public Calculator() 
    {
        _memory = 0.0;
    }

    public double Add(double x, double y) 
    {
        return x + y;
    }

    public static void Main(string[] args)
    {
        var calc = new Calculator();
    }
}
"#;

        let chunks = chunk_language(csharp_code, ParseableLanguage::CSharp).unwrap();
        assert!(!chunks.is_empty());

        // Should find variable, class, method and interface declarations
        let chunk_types: Vec<&ChunkType> = chunks.iter().map(|c| &c.chunk_type).collect();
        assert!(chunk_types.contains(&&ChunkType::Module)); // var, interface
        assert!(chunk_types.contains(&&ChunkType::Class)); // class
        assert!(chunk_types.contains(&&ChunkType::Method)); // methods
    }

    #[test]
    fn test_stride_large_chunk_empty_text() {
        // Regression test for divide-by-zero bug in stride_large_chunk
        let empty_chunk = Chunk {
            span: Span {
                byte_start: 0,
                byte_end: 0,
                line_start: 1,
                line_end: 1,
            },
            text: String::new(), // Empty text should not panic
            chunk_type: ChunkType::Text,
            stride_info: None,
        };

        let config = ChunkConfig::default();
        let result = stride_large_chunk(empty_chunk.clone(), &config);

        // Should not panic and return the original chunk
        assert!(result.is_ok());
        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "");
    }

    #[test]
    fn test_stride_large_chunk_zero_token_estimate() {
        // Regression test for zero token estimate causing divide-by-zero
        let chunk = Chunk {
            span: Span {
                byte_start: 0,
                byte_end: 5,
                line_start: 1,
                line_end: 1,
            },
            text: "     ".to_string(), // Whitespace that might return 0 tokens
            chunk_type: ChunkType::Text,
            stride_info: None,
        };

        let config = ChunkConfig::default();
        let result = stride_large_chunk(chunk, &config);

        // Should not panic and handle gracefully
        assert!(result.is_ok());
    }

    #[test]
    fn test_strided_chunk_line_calculation() {
        // Regression test for line_end calculation in strided chunks
        // Create a chunk large enough to force striding
        let long_text = (1..=50).map(|i| format!("This is a longer line {} with more content to ensure token count is high enough", i)).collect::<Vec<_>>().join("\n");

        let chunk = Chunk {
            span: Span {
                byte_start: 0,
                byte_end: long_text.len(),
                line_start: 1,
                line_end: 50,
            },
            text: long_text,
            chunk_type: ChunkType::Text,
            stride_info: None,
        };

        let config = ChunkConfig {
            max_tokens: 100,    // Force striding with reasonable limit
            stride_overlap: 10, // Small overlap for testing
            ..Default::default()
        };

        let result = stride_large_chunk(chunk, &config);
        if let Err(e) = &result {
            eprintln!("Stride error: {}", e);
        }
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(
            chunks.len() > 1,
            "Should create multiple chunks when striding"
        );

        for chunk in chunks {
            // Verify line_end is not off by one
            // line_end should be inclusive and not exceed the actual content
            assert!(chunk.span.line_end >= chunk.span.line_start);

            // Check that line span makes sense for the content
            let line_count = chunk.text.lines().count();
            if line_count > 0 {
                let calculated_line_span = chunk.span.line_end - chunk.span.line_start + 1;

                // Allow some tolerance for striding logic
                assert!(
                    calculated_line_span <= line_count + 1,
                    "Line span {} should not exceed content lines {} by more than 1",
                    calculated_line_span,
                    line_count
                );
            }
        }
    }

    #[test]
    fn test_haskell_function_chunking() {
        let haskell_code = r#"
factorial :: Integer -> Integer
factorial 0 = 1
factorial n = n * factorial (n - 1)

fibonacci :: Integer -> Integer
fibonacci 0 = 0
fibonacci 1 = 1
fibonacci n = fibonacci (n - 1) + fibonacci (n - 2)
"#;

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_haskell::LANGUAGE.into()).unwrap();
        let tree = parser.parse(haskell_code, None).unwrap();

        // Debug: print tree structure
        fn walk(node: tree_sitter::Node, src: &str, depth: usize) {
            let kind = node.kind();
            let start = node.start_position();
            let end = node.end_position();
            eprintln!("{}{:30} L{}-{}", "  ".repeat(depth), kind, start.row + 1, end.row + 1);

            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    walk(cursor.node(), src, depth + 1);
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }

        eprintln!("\n=== TREE STRUCTURE ===");
        walk(tree.root_node(), haskell_code, 0);
        eprintln!("=== END TREE ===\n");

        let chunks = chunk_language(haskell_code, ParseableLanguage::Haskell).unwrap();

        eprintln!("\n=== CHUNKS ===");
        for (i, chunk) in chunks.iter().enumerate() {
            eprintln!("Chunk {}: {:?} L{}-{}", i, chunk.chunk_type, chunk.span.line_start, chunk.span.line_end);
            eprintln!("  Text: {:?}", chunk.text);
        }
        eprintln!("=== END CHUNKS ===\n");

        assert!(!chunks.is_empty(), "Should find chunks in Haskell code");

        // Find factorial chunk and verify it includes both signature and implementation
        let factorial_chunk = chunks.iter().find(|c| c.text.contains("factorial 0 = 1"));
        assert!(factorial_chunk.is_some(), "Should find factorial function body");

        let fac = factorial_chunk.unwrap();
        assert!(fac.text.contains("factorial :: Integer -> Integer"), "Should include type signature");
        assert!(fac.text.contains("factorial 0 = 1"), "Should include base case");
        assert!(fac.text.contains("factorial n = n * factorial (n - 1)"), "Should include recursive case");
    }
}
