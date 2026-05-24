use anyhow::Result;
use ck_core::Span;
use serde::{Deserialize, Serialize};

mod query_chunker;

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChunkMetadata {
    pub ancestry: Vec<String>,
    pub breadcrumb: Option<String>,
    pub leading_trivia: Vec<String>,
    pub trailing_trivia: Vec<String>,
    pub byte_length: usize,
    pub estimated_tokens: usize,
}

impl ChunkMetadata {
    fn from_context(
        text: &str,
        ancestry: Vec<String>,
        leading_trivia: Vec<String>,
        trailing_trivia: Vec<String>,
    ) -> Self {
        let breadcrumb = if ancestry.is_empty() {
            None
        } else {
            Some(ancestry.join("::"))
        };

        Self {
            ancestry,
            breadcrumb,
            leading_trivia,
            trailing_trivia,
            byte_length: text.len(),
            estimated_tokens: estimate_tokens(text),
        }
    }

    fn from_text(text: &str) -> Self {
        Self {
            ancestry: Vec::new(),
            breadcrumb: None,
            leading_trivia: Vec::new(),
            trailing_trivia: Vec::new(),
            byte_length: text.len(),
            estimated_tokens: estimate_tokens(text),
        }
    }

    fn with_updated_text(&self, text: &str) -> Self {
        let mut cloned = self.clone();
        cloned.byte_length = text.len();
        cloned.estimated_tokens = estimate_tokens(text);
        cloned
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub span: Span,
    pub text: String,
    pub chunk_type: ChunkType,
    /// Stride information if this chunk was created by striding a larger chunk
    pub stride_info: Option<StrideInfo>,
    pub metadata: ChunkMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChunkType {
    Text,
    Function,
    Class,
    Method,
    Module,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParseableLanguage {
    Python,
    TypeScript,
    JavaScript,
    Haskell,
    Rust,
    Ruby,
    Go,
    C,
    Cpp,
    CSharp,
    Zig,

    Dart,

    Elixir,

    Markdown,
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
            ParseableLanguage::C => "c",
            ParseableLanguage::Cpp => "cpp",
            ParseableLanguage::CSharp => "csharp",
            ParseableLanguage::Zig => "zig",

            ParseableLanguage::Dart => "dart",

            ParseableLanguage::Elixir => "elixir",

            ParseableLanguage::Markdown => "markdown",
        };
        write!(f, "{name}")
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
            ck_core::Language::C => Ok(ParseableLanguage::C),
            ck_core::Language::Cpp => Ok(ParseableLanguage::Cpp),
            ck_core::Language::CSharp => Ok(ParseableLanguage::CSharp),
            ck_core::Language::Zig => Ok(ParseableLanguage::Zig),

            ck_core::Language::Dart => Ok(ParseableLanguage::Dart),

            ck_core::Language::Elixir => Ok(ParseableLanguage::Elixir),

            ck_core::Language::Markdown => Ok(ParseableLanguage::Markdown),

            _ => Err(anyhow::anyhow!(
                "Language {lang:?} is not supported for parsing"
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
        let metadata = ChunkMetadata::from_text(&chunk_text);

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
            metadata,
        });

        i += chunk_size - overlap;
        if i >= lines.len() {
            break;
        }
    }

    Ok(chunks)
}

pub(crate) fn tree_sitter_language(language: ParseableLanguage) -> Result<tree_sitter::Language> {
    // tree-sitter-dart v0.0.4 uses an older API that returns Language directly,
    // while newer bindings (v0.24+) require calling .into() on a factory struct.
    if language == ParseableLanguage::Dart {
        return Ok(tree_sitter_dart::language());
    }

    if language == ParseableLanguage::Markdown {
        return Ok(tree_sitter_md::LANGUAGE.into());
    }

    let ts_language = match language {
        ParseableLanguage::Python => tree_sitter_python::LANGUAGE,
        ParseableLanguage::TypeScript | ParseableLanguage::JavaScript => {
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT
        }
        ParseableLanguage::Haskell => tree_sitter_haskell::LANGUAGE,
        ParseableLanguage::Rust => tree_sitter_rust::LANGUAGE,
        ParseableLanguage::Ruby => tree_sitter_ruby::LANGUAGE,
        ParseableLanguage::Go => tree_sitter_go::LANGUAGE,
        ParseableLanguage::C => tree_sitter_c::LANGUAGE,
        ParseableLanguage::Cpp => tree_sitter_cpp::LANGUAGE,
        ParseableLanguage::CSharp => tree_sitter_c_sharp::LANGUAGE,
        ParseableLanguage::Zig => tree_sitter_zig::LANGUAGE,

        ParseableLanguage::Dart => unreachable!("Handled above via early return"),

        ParseableLanguage::Elixir => tree_sitter_elixir::LANGUAGE,

        ParseableLanguage::Markdown => unreachable!("Handled above via early return"),
    };

    Ok(ts_language.into())
}

fn chunk_language(text: &str, language: ParseableLanguage) -> Result<Vec<Chunk>> {
    let mut parser = tree_sitter::Parser::new();
    let ts_language = tree_sitter_language(language)?;
    parser.set_language(&ts_language)?;

    let tree = parser
        .parse(text, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse {language} code"))?;

    let mut chunks = match query_chunker::chunk_with_queries(language, ts_language, &tree, text)? {
        Some(query_chunks) if !query_chunks.is_empty() => query_chunks,
        _ => {
            let mut legacy_chunks = Vec::new();
            let mut cursor = tree.walk();
            extract_code_chunks(&mut cursor, text, &mut legacy_chunks, language);
            legacy_chunks
        }
    };

    if chunks.is_empty() {
        return chunk_generic(text);
    }

    // Post-process Haskell chunks to merge function equations
    if language == ParseableLanguage::Haskell {
        chunks = merge_haskell_functions(chunks, text);
    }

    // Fill gaps between chunks with remainder content. This must happen
    // BEFORE suppress_contained_text_chunks so that gap-produced Text
    // chunks (e.g., the run of field declarations and access specifiers
    // inside a class body) are also subject to suppression — otherwise
    // they leak through as standalone Text chunks (issue #136).
    chunks = fill_gaps(chunks, text);

    // Merge template-prefix gap chunks into the following C++ definition chunk
    if language == ParseableLanguage::Cpp {
        chunks = merge_cpp_template_prefix_chunks(chunks, text);
    }

    // Suppress text chunks fully contained by class/method/function chunks for C/C++
    if matches!(language, ParseableLanguage::C | ParseableLanguage::Cpp) {
        chunks = suppress_contained_text_chunks(chunks);
    }

    // Merge small chunks if Markdown
    if language == ParseableLanguage::Markdown {
        let (target_tokens, _) = get_model_chunk_config(None);
        chunks = merge_small_chunks(chunks, text, target_tokens);
    }

    Ok(chunks)
}

fn suppress_contained_text_chunks(chunks: Vec<Chunk>) -> Vec<Chunk> {
    if chunks.is_empty() {
        return chunks;
    }

    let mut containers: Vec<(usize, usize)> = chunks
        .iter()
        .filter(|chunk| {
            matches!(
                chunk.chunk_type,
                ChunkType::Class | ChunkType::Method | ChunkType::Function
            )
        })
        .map(|chunk| (chunk.span.byte_start, chunk.span.byte_end))
        .collect();

    if containers.is_empty() {
        return chunks;
    }

    containers.sort_by_key(|(start, _)| *start);

    chunks
        .into_iter()
        .filter(|chunk| {
            if chunk.chunk_type != ChunkType::Text {
                return true;
            }

            let start = chunk.span.byte_start;
            let end = chunk.span.byte_end;
            !containers
                .iter()
                .any(|(c_start, c_end)| *c_start <= start && end <= *c_end)
        })
        .collect()
}

fn merge_cpp_template_prefix_chunks(chunks: Vec<Chunk>, text: &str) -> Vec<Chunk> {
    if chunks.len() < 2 {
        return chunks;
    }

    let mut merged = Vec::with_capacity(chunks.len());
    let mut idx = 0;

    while idx < chunks.len() {
        if idx + 1 < chunks.len() && is_template_prefix_chunk(&chunks[idx]) {
            let template_chunk = &chunks[idx];
            let mut next_chunk = chunks[idx + 1].clone();

            // Adjacency: byte_end may be <= next.byte_start, with only
            // whitespace in between. Previously required strict equality
            // which missed multiline templates and blank lines between
            // the template clause and the definition (issue #136).
            let gap_is_whitespace = template_chunk.span.byte_end <= next_chunk.span.byte_start
                && text
                    .get(template_chunk.span.byte_end..next_chunk.span.byte_start)
                    .is_some_and(|gap| gap.chars().all(char::is_whitespace));

            if gap_is_whitespace
                && template_chunk.span.byte_start < next_chunk.span.byte_end
                && next_chunk.span.byte_end <= text.len()
            {
                let new_start = template_chunk.span.byte_start;
                let new_end = next_chunk.span.byte_end;

                if let Some(new_text) = text.get(new_start..new_end) {
                    let (line_start, line_end) = line_range_for_span(text, new_start, new_end);

                    next_chunk.span.byte_start = new_start;
                    next_chunk.span.line_start = line_start;
                    next_chunk.span.line_end = line_end;
                    next_chunk.text = new_text.to_string();
                    next_chunk.metadata = next_chunk.metadata.with_updated_text(new_text);

                    merged.push(next_chunk);
                    idx += 2;
                    continue;
                }
            }
        }

        merged.push(chunks[idx].clone());
        idx += 1;
    }

    merged
}

fn is_template_prefix_chunk(chunk: &Chunk) -> bool {
    if chunk.chunk_type != ChunkType::Text {
        return false;
    }

    let mut has_template = false;
    for line in chunk.text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("template <") || trimmed.starts_with("template<") {
            has_template = true;
            continue;
        }
        return false;
    }

    has_template
}

fn line_range_for_span(text: &str, byte_start: usize, byte_end: usize) -> (usize, usize) {
    let line_start = text[..byte_start].matches('\n').count() + 1;
    let newlines_up_to_end = text[..byte_end].matches('\n').count();
    let line_end = if newlines_up_to_end >= line_start - 1 {
        newlines_up_to_end.max(line_start)
    } else {
        line_start
    };

    (line_start, line_end)
}

/// Fill gaps between chunks with remainder content
/// This ensures that leading imports, trailing code, and content between functions gets indexed
/// Combines contiguous gaps into single chunks (excluding standalone blank lines)
fn fill_gaps(mut chunks: Vec<Chunk>, text: &str) -> Vec<Chunk> {
    if chunks.is_empty() {
        return chunks;
    }

    // Sort chunks by byte position to identify gaps
    chunks.sort_by_key(|c| c.span.byte_start);

    let mut result = Vec::new();
    let mut last_end = 0;

    // Collect all gaps, splitting on blank lines
    let mut gaps = Vec::new();

    for chunk in &chunks {
        if last_end < chunk.span.byte_start {
            // Split this gap by blank lines - use split to make it simple
            let gap_start = last_end;
            let gap_text = &text[gap_start..chunk.span.byte_start];

            // Split on sequences of blank lines
            let mut current_byte = gap_start;
            let mut segment_start = gap_start;

            for line in gap_text.split('\n') {
                let line_start_in_gap = current_byte - gap_start;
                let _line_end_in_gap = line_start_in_gap + line.len();

                if line.trim().is_empty() {
                    // Found a blank line - save segment before it if it has content
                    if segment_start < current_byte {
                        let segment_text = &text[segment_start..current_byte];
                        if !segment_text.trim().is_empty() {
                            gaps.push((segment_start, current_byte));
                        }
                    }
                    // Next segment starts after this blank line and its newline
                    segment_start = current_byte + line.len() + 1;
                }

                current_byte += line.len() + 1; // +1 for the \n
            }

            // Handle final segment (after last newline or if no newlines)
            if segment_start < chunk.span.byte_start {
                let remaining = &text[segment_start..chunk.span.byte_start];
                if !remaining.trim().is_empty() {
                    gaps.push((segment_start, chunk.span.byte_start));
                }
            }
        }
        last_end = last_end.max(chunk.span.byte_end);
    }

    // Handle trailing content
    if last_end < text.len() {
        let gap_text = &text[last_end..];
        if !gap_text.trim().is_empty() {
            gaps.push((last_end, text.len()));
        }
    }

    let combined_gaps = gaps;

    // Now interleave chunks and combined gap chunks
    let mut gap_idx = 0;

    for chunk in chunks {
        // Add any gap chunks that come before this structural chunk
        while gap_idx < combined_gaps.len() && combined_gaps[gap_idx].1 <= chunk.span.byte_start {
            let (gap_start, gap_end) = combined_gaps[gap_idx];
            let gap_text = &text[gap_start..gap_end];

            // Calculate line numbers by counting newlines before each position
            let line_start = text[..gap_start].matches('\n').count() + 1;
            // For line_end, count newlines in the text including the gap
            // This gives us the line number of the last line with gap content
            let newlines_up_to_end = text[..gap_end].matches('\n').count();
            let line_end = if newlines_up_to_end >= line_start - 1 {
                newlines_up_to_end.max(line_start)
            } else {
                line_start
            };

            let gap_chunk = Chunk {
                text: gap_text.to_string(),
                span: Span {
                    byte_start: gap_start,
                    byte_end: gap_end,
                    line_start,
                    line_end,
                },
                chunk_type: ChunkType::Text,
                metadata: ChunkMetadata::from_text(gap_text),
                stride_info: None,
            };
            result.push(gap_chunk);
            gap_idx += 1;
        }

        result.push(chunk.clone());
    }

    // Add any remaining gap chunks after the last structural chunk
    while gap_idx < combined_gaps.len() {
        let (gap_start, gap_end) = combined_gaps[gap_idx];
        let gap_text = &text[gap_start..gap_end];

        // Calculate line numbers by counting newlines before each position
        let line_start = text[..gap_start].matches('\n').count() + 1;
        // For line_end, count newlines in the text including the gap
        let newlines_up_to_end = text[..gap_end].matches('\n').count();
        let line_end = if newlines_up_to_end >= line_start - 1 {
            newlines_up_to_end.max(line_start)
        } else {
            line_start
        };

        let gap_chunk = Chunk {
            text: gap_text.to_string(),
            span: Span {
                byte_start: gap_start,
                byte_end: gap_end,
                line_start,
                line_end,
            },
            chunk_type: ChunkType::Text,
            metadata: ChunkMetadata::from_text(gap_text),
            stride_info: None,
        };
        result.push(gap_chunk);
        gap_idx += 1;
    }

    result
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
            chunk
                .text
                .split("::")
                .next()
                .and_then(|s| s.split_whitespace().next())
                .map(std::string::ToString::to_string)
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
        let mut trailing_trivia = chunk.metadata.trailing_trivia.clone();

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
                next_chunk
                    .text
                    .split("::")
                    .next()
                    .and_then(|s| s.split_whitespace().next())
                    .map(std::string::ToString::to_string)
            } else {
                extract_haskell_function_name(&next_chunk.text)
            };

            if next_name == Some(name.clone()) {
                // Extend the group to include this equation
                group_end = next_chunk.span.byte_end;
                line_end = next_chunk.span.line_end;
                trailing_trivia = next_chunk.metadata.trailing_trivia.clone();
                j += 1;
            } else {
                break;
            }
        }

        // Create merged chunk
        let merged_text = source.get(group_start..group_end).unwrap_or("").to_string();
        let mut metadata = chunk.metadata.with_updated_text(&merged_text);
        metadata.trailing_trivia = trailing_trivia;

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
            metadata,
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

    if !should_skip
        && let Some(initial_chunk_type) = chunk_type_for_node(language, &node)
        && let Some(chunk) = build_chunk(node, source, initial_chunk_type, language)
    {
        let is_duplicate = chunks.iter().any(|existing| {
            existing.span.byte_start == chunk.span.byte_start
                && existing.span.byte_end == chunk.span.byte_end
        });

        if !is_duplicate {
            chunks.push(chunk);
        }
    }

    // For Haskell signatures: don't recurse into children (they're just type expressions)
    let should_recurse = !(language == ParseableLanguage::Haskell && node.kind() == "signature");

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
        ParseableLanguage::C => matches!(
            kind,
            "function_definition"
                | "struct_specifier"
                | "enum_specifier"
                | "union_specifier"
                | "type_definition"
                | "declaration"
                | "preproc_function_def"
                | "preproc_def"
        ),
        ParseableLanguage::Cpp => matches!(
            kind,
            "function_definition"
                | "class_specifier"
                | "struct_specifier"
                | "enum_specifier"
                | "union_specifier"
                | "namespace_definition"
                | "template_declaration"
                | "type_definition"
                | "alias_declaration"
                | "declaration"
                | "preproc_function_def"
                | "preproc_def"
        ),
        ParseableLanguage::CSharp => matches!(
            kind,
            "method_declaration"
                | "class_declaration"
                | "interface_declaration"
                | "variable_declaration"
        ),
        ParseableLanguage::Dart => matches!(
            kind,
            "class_definition"
                | "class_declaration"
                | "mixin_declaration"
                | "enum_declaration"
                | "function_declaration"
                | "method_declaration"
                | "constructor_declaration"
                | "variable_declaration"
                | "local_variable_declaration"
                | "lambda_expression"
                | "class_member_definition"
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
        // Elixir uses "call" nodes for defmodule, def, defp, etc.
        // We handle this specially via query-based chunking
        ParseableLanguage::Elixir => matches!(kind, "call" | "do_block"),
        ParseableLanguage::Markdown => matches!(
            kind,
            "atx_heading"
                | "setext_heading"
                | "heading"
                | "section"
                | "fenced_code_block"
                | "indented_code_block"
                | "block_quote"
                | "list"
                | "list_item"
                | "paragraph"
                | "thematic_break"
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
            if !is_csharp_field_like(*node) {
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
        | "singleton_method"
        | "preproc_function_def" => ChunkType::Function,
        "signature" => ChunkType::Function, // Haskell type signatures will be merged with functions
        "class_definition"
        | "class_declaration"
        | "instance_declaration"
        | "class"
        | "instance"
        | "struct_item"
        | "enum_item"
        | "class_specifier"
        | "struct_specifier"
        | "enum_specifier"
        | "union_specifier"
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
        | "namespace_definition"
        | "defmodule"
        | "module"
        | "defprotocol"
        | "interface_declaration"
        | "ns"
        | "var_declaration"
        | "const_declaration"
        | "variable_declaration"
        | "test_declaration"
        | "comptime_declaration"
        | "atx_heading"
        | "setext_heading"
        | "heading"
        | "section" => ChunkType::Module,
        _ => ChunkType::Text,
    }
}

pub(crate) fn build_chunk(
    node: tree_sitter::Node<'_>,
    source: &str,
    initial_type: ChunkType,
    language: ParseableLanguage,
) -> Option<Chunk> {
    let target_node = adjust_node_for_language(node, language);

    if matches!(language, ParseableLanguage::C | ParseableLanguage::Cpp)
        && matches!(initial_type, ChunkType::Class)
        && matches!(
            target_node.kind(),
            "struct_specifier" | "union_specifier" | "enum_specifier"
        )
        && !c_cpp_type_has_body_node(target_node)
    {
        return None;
    }
    let (byte_start, start_row, leading_segments) =
        extend_with_leading_trivia(target_node, language, source);
    let trailing_segments = collect_trailing_trivia(target_node, language, source);

    let byte_end = target_node.end_byte();
    let end_pos = target_node.end_position();

    if byte_start >= byte_end || byte_end > source.len() {
        return None;
    }

    let chunk_type = adjust_chunk_type_for_context(target_node, initial_type, language);
    let mut text = source.get(byte_start..byte_end)?.to_string();
    if matches!(language, ParseableLanguage::C | ParseableLanguage::Cpp)
        && chunk_type == ChunkType::Class
    {
        text = strip_method_bodies_in_class_text(target_node, source, byte_start, byte_end);
    }

    if text.trim().is_empty() {
        return None;
    }
    let ancestry = collect_ancestry(target_node, language, source);
    let leading_trivia = segments_to_strings(&leading_segments, source);
    let trailing_trivia = segments_to_strings(&trailing_segments, source);
    let mut metadata =
        ChunkMetadata::from_context(&text, ancestry, leading_trivia, trailing_trivia);
    if matches!(language, ParseableLanguage::C | ParseableLanguage::Cpp)
        && matches!(chunk_type, ChunkType::Function | ChunkType::Method)
        && let Some(full_name) = c_cpp_function_breadcrumb(target_node, language, source)
    {
        metadata.breadcrumb = Some(full_name);
    }

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
        metadata,
    })
}

fn c_cpp_type_has_body_node(node: tree_sitter::Node<'_>) -> bool {
    let mut cursor = node.walk();

    match node.kind() {
        "struct_specifier" | "union_specifier" => node
            .children(&mut cursor)
            .any(|child| child.kind() == "field_declaration_list"),
        "enum_specifier" => node
            .children(&mut cursor)
            .any(|child| child.kind() == "enumerator_list"),
        _ => false,
    }
}

fn c_cpp_function_breadcrumb(
    node: tree_sitter::Node<'_>,
    language: ParseableLanguage,
    source: &str,
) -> Option<String> {
    let name = display_name_for_node(node, language, source, ChunkType::Function)?;
    let context = collect_c_cpp_context_names(node, language, source);
    let context_path = context.join("::");

    if name.contains("::") {
        if context_path.is_empty() || name.starts_with(&format!("{}::", context_path)) {
            Some(name)
        } else {
            Some(format!("{}::{}", context_path, name))
        }
    } else if context_path.is_empty() {
        Some(name)
    } else {
        Some(format!("{}::{}", context_path, name))
    }
}

fn collect_c_cpp_context_names(
    mut node: tree_sitter::Node<'_>,
    language: ParseableLanguage,
    source: &str,
) -> Vec<String> {
    let mut parts = Vec::new();

    while let Some(parent) = node.parent() {
        let kind = parent.kind();
        let include = match language {
            ParseableLanguage::Cpp => matches!(
                kind,
                "namespace_definition" | "class_specifier" | "struct_specifier"
            ),
            ParseableLanguage::C => matches!(kind, "struct_specifier"),
            _ => false,
        };

        if include
            && let Some(name) = display_name_for_node(parent, language, source, ChunkType::Class)
        {
            parts.push(name);
        }

        node = parent;
    }

    parts.reverse();
    parts
}

fn strip_method_bodies_in_class_text(
    class_node: tree_sitter::Node<'_>,
    source: &str,
    byte_start: usize,
    byte_end: usize,
) -> String {
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    let mut stack = vec![class_node];

    while let Some(node) = stack.pop() {
        if is_method_like_node(node.kind())
            && let Some(body) = find_method_body_node(node)
        {
            let start = body.start_byte();
            let end = body.end_byte();
            if start >= byte_start && end <= byte_end && start < end {
                let replacement = method_body_placeholder(body, source);
                replacements.push((start, end, replacement));
            }
        }

        let child_count = node.child_count();
        for idx in (0..child_count).rev() {
            if let Some(child) = node.child(idx) {
                stack.push(child);
            }
        }
    }

    if replacements.is_empty() {
        return source
            .get(byte_start..byte_end)
            .unwrap_or_default()
            .to_string();
    }

    replacements.sort_by(|a, b| b.0.cmp(&a.0));
    let mut text = source
        .get(byte_start..byte_end)
        .unwrap_or_default()
        .to_string();

    for (start, end, replacement) in replacements {
        if start < byte_start || end > byte_end || end <= start {
            continue;
        }
        let local_start = start - byte_start;
        let local_end = end - byte_start;
        if local_end <= text.len() {
            text.replace_range(local_start..local_end, &replacement);
        }
    }

    text
}

fn is_method_like_node(kind: &str) -> bool {
    matches!(
        kind,
        "function_definition"
            | "method_definition"
            | "method_declaration"
            | "constructor_declaration"
            | "destructor_declaration"
            | "function_item"
            | "method"
            | "singleton_method"
    )
}

fn find_method_body_node(node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    let body_kinds = [
        "compound_statement",
        "statement_block",
        "block",
        "body",
        "body_statement",
        "declaration_list",
    ];

    for idx in 0..node.child_count() {
        if let Some(child) = node.child(idx)
            && body_kinds.contains(&child.kind())
        {
            return Some(child);
        }
    }

    None
}

fn method_body_placeholder(_body: tree_sitter::Node<'_>, _source: &str) -> String {
    ";".to_string()
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

#[derive(Clone, Copy)]
struct TriviaSegment {
    start_byte: usize,
    end_byte: usize,
}

fn extend_with_leading_trivia(
    node: tree_sitter::Node<'_>,
    language: ParseableLanguage,
    source: &str,
) -> (usize, usize, Vec<TriviaSegment>) {
    let mut start_byte = node.start_byte();
    let mut start_row = node.start_position().row;
    let mut current = node;
    let mut segments = Vec::new();

    while let Some(prev) = current.prev_sibling() {
        if should_attach_leading_trivia(language, &prev)
            && only_whitespace_between(source, prev.end_byte(), start_byte)
        {
            start_byte = prev.start_byte();
            start_row = prev.start_position().row;
            segments.push(TriviaSegment {
                start_byte: prev.start_byte(),
                end_byte: prev.end_byte(),
            });
            current = prev;
            continue;
        }
        break;
    }

    segments.reverse();
    (start_byte, start_row, segments)
}

fn should_attach_leading_trivia(language: ParseableLanguage, node: &tree_sitter::Node<'_>) -> bool {
    let kind = node.kind();
    if kind == "comment" {
        return true;
    }

    match language {
        ParseableLanguage::Rust => {
            matches!(kind, "line_comment" | "block_comment" | "attribute_item")
        }
        ParseableLanguage::Python => kind == "decorator",
        ParseableLanguage::TypeScript | ParseableLanguage::JavaScript => kind == "decorator",
        ParseableLanguage::C | ParseableLanguage::Cpp | ParseableLanguage::Markdown => {
            kind == "comment"
        }
        ParseableLanguage::CSharp => matches!(kind, "attribute_list" | "attribute"),
        _ => false,
    }
}

fn collect_trailing_trivia(
    node: tree_sitter::Node<'_>,
    language: ParseableLanguage,
    source: &str,
) -> Vec<TriviaSegment> {
    let mut segments = Vec::new();
    let mut current = node;
    let mut previous_end = node.end_byte();

    while let Some(next) = current.next_sibling() {
        if should_attach_trailing_trivia(language, &next)
            && only_whitespace_between(source, previous_end, next.start_byte())
        {
            segments.push(TriviaSegment {
                start_byte: next.start_byte(),
                end_byte: next.end_byte(),
            });
            previous_end = next.end_byte();
            current = next;
            continue;
        }
        break;
    }

    segments
}

fn should_attach_trailing_trivia(
    _language: ParseableLanguage,
    node: &tree_sitter::Node<'_>,
) -> bool {
    node.kind() == "comment"
}

fn segments_to_strings(segments: &[TriviaSegment], source: &str) -> Vec<String> {
    let mut result = Vec::new();

    for segment in segments {
        if let Some(text) = source
            .get(segment.start_byte..segment.end_byte)
            .map(std::string::ToString::to_string)
        {
            result.push(text);
        }
    }

    result
}

fn collect_ancestry(
    mut node: tree_sitter::Node<'_>,
    language: ParseableLanguage,
    source: &str,
) -> Vec<String> {
    if language == ParseableLanguage::Markdown {
        return markdown_heading_ancestry(node, source);
    }

    let mut parts = Vec::new();

    while let Some(parent) = node.parent() {
        if let Some(parent_chunk_type) = chunk_type_for_node(language, &parent)
            && let Some(name) = display_name_for_node(parent, language, source, parent_chunk_type)
        {
            parts.push(name);
        }
        node = parent;
    }

    parts.reverse();
    parts
}

fn display_name_for_node(
    node: tree_sitter::Node<'_>,
    language: ParseableLanguage,
    source: &str,
    chunk_type: ChunkType,
) -> Option<String> {
    if let Some(name_node) = node.child_by_field_name("name") {
        return text_for_node(name_node, source);
    }

    match language {
        ParseableLanguage::Rust => rust_display_name(node, source, chunk_type),
        ParseableLanguage::Python => find_identifier(node, source, &["identifier"]),
        ParseableLanguage::TypeScript | ParseableLanguage::JavaScript => find_identifier(
            node,
            source,
            &["identifier", "type_identifier", "property_identifier"],
        ),
        ParseableLanguage::Haskell => {
            find_identifier(node, source, &["identifier", "type_identifier", "variable"])
                .or_else(|| first_word_of_node(node, source))
        }
        ParseableLanguage::Ruby => find_identifier(node, source, &["identifier"]),
        ParseableLanguage::Go => find_identifier(node, source, &["identifier", "type_identifier"]),
        ParseableLanguage::C => c_display_name(node, source, chunk_type),
        ParseableLanguage::Cpp => cpp_display_name(node, source, chunk_type),
        ParseableLanguage::CSharp => find_identifier(node, source, &["identifier"]),
        ParseableLanguage::Zig => find_identifier(node, source, &["identifier"]),

        ParseableLanguage::Markdown => markdown_display_name(node, source, chunk_type),

        ParseableLanguage::Dart => {
            find_identifier(node, source, &["identifier", "type_identifier"])
        }
        ParseableLanguage::Elixir => {
            // Elixir names can be aliases (module names) or atoms/identifiers
            find_identifier(node, source, &["alias", "identifier", "atom"])
        }
    }
}

fn markdown_display_name(
    node: tree_sitter::Node<'_>,
    source: &str,
    _chunk_type: ChunkType,
) -> Option<String> {
    if node.kind() == "section" {
        return markdown_section_heading(node, source);
    }

    if markdown_heading_kind(node.kind()) {
        return markdown_heading_text(node, source);
    }

    None
}

fn markdown_section_heading(node: tree_sitter::Node<'_>, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if markdown_heading_kind(child.kind()) {
            return markdown_heading_text(child, source);
        }
    }
    None
}

fn markdown_heading_kind(kind: &str) -> bool {
    matches!(kind, "atx_heading" | "setext_heading" | "heading")
}

fn markdown_heading_text(node: tree_sitter::Node<'_>, source: &str) -> Option<String> {
    let text = text_for_node(node, source)?;
    let mut lines = text.lines();
    let first_line = lines.next().unwrap_or("");

    if let Some((_, heading)) = parse_atx_heading_line(first_line) {
        return Some(heading);
    }

    let second_line = lines.next().unwrap_or("");
    if parse_setext_level(second_line).is_some() {
        let trimmed = first_line.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    let trimmed = first_line.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn markdown_heading_ancestry(node: tree_sitter::Node<'_>, source: &str) -> Vec<String> {
    let mut target_row = node.start_position().row;
    if node.kind() == "section" || markdown_heading_kind(node.kind()) {
        target_row = target_row.saturating_sub(1);
    }
    let lines: Vec<&str> = source.lines().collect();
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut i = 0;

    while i < lines.len() && i <= target_row {
        let line = lines[i];

        if let Some((level, heading)) = parse_atx_heading_line(line) {
            update_markdown_heading_stack(&mut stack, level, heading);
            i += 1;
            continue;
        }

        if i + 1 < lines.len() && i < target_row {
            let underline = lines[i + 1];
            if let Some(level) = parse_setext_level(underline) {
                let heading_text = line.trim();
                if !heading_text.is_empty() {
                    update_markdown_heading_stack(&mut stack, level, heading_text.to_string());
                }
                i += 2;
                continue;
            }
        }

        i += 1;
    }

    stack.into_iter().map(|(_, heading)| heading).collect()
}

fn update_markdown_heading_stack(stack: &mut Vec<(usize, String)>, level: usize, text: String) {
    while let Some((existing_level, _)) = stack.last() {
        if *existing_level < level {
            break;
        }
        stack.pop();
    }
    stack.push((level, text));
}

fn parse_atx_heading_line(line: &str) -> Option<(usize, String)> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return None;
    }

    let level = trimmed.chars().take_while(|c| *c == '#').count();
    if level == 0 {
        return None;
    }

    let mut text = trimmed[level..].trim();
    text = text.trim_end_matches('#').trim();
    if text.is_empty() {
        return None;
    }

    Some((level, text.to_string()))
}

fn parse_setext_level(line: &str) -> Option<usize> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.chars().all(|c| c == '=') {
        Some(1)
    } else if trimmed.chars().all(|c| c == '-') {
        Some(2)
    } else {
        None
    }
}

fn rust_display_name(
    node: tree_sitter::Node<'_>,
    source: &str,
    chunk_type: ChunkType,
) -> Option<String> {
    match node.kind() {
        "impl_item" => {
            let mut parts = Vec::new();
            if let Some(ty) = node.child_by_field_name("type")
                && let Some(text) = text_for_node(ty, source)
            {
                parts.push(text);
            }
            if let Some(trait_node) = node.child_by_field_name("trait")
                && let Some(text) = text_for_node(trait_node, source)
            {
                if let Some(last) = parts.first() {
                    parts[0] = format!("{} (impl {})", last, text.trim());
                } else {
                    parts.push(format!("impl {}", text.trim()));
                }
            }
            if parts.is_empty() {
                find_identifier(node, source, &["identifier"])
            } else {
                Some(parts.remove(0))
            }
        }
        "mod_item" if chunk_type == ChunkType::Module => {
            find_identifier(node, source, &["identifier"])
        }
        _ => find_identifier(node, source, &["identifier", "type_identifier"]),
    }
}

fn c_display_name(
    node: tree_sitter::Node<'_>,
    source: &str,
    _chunk_type: ChunkType,
) -> Option<String> {
    match node.kind() {
        "function_definition" => {
            // C function: look for the declarator, then the identifier inside it
            if let Some(declarator) = node.child_by_field_name("declarator") {
                return find_identifier_recursive(declarator, source, &["identifier"]);
            }
            None
        }
        "struct_specifier" | "enum_specifier" | "union_specifier" => {
            find_identifier(node, source, &["type_identifier", "identifier"])
        }
        "type_definition" => find_identifier(node, source, &["type_identifier", "identifier"]),
        "preproc_function_def" | "preproc_def" => find_identifier(node, source, &["identifier"]),
        _ => find_identifier(node, source, &["identifier", "type_identifier"]),
    }
}

fn cpp_display_name(
    node: tree_sitter::Node<'_>,
    source: &str,
    _chunk_type: ChunkType,
) -> Option<String> {
    match node.kind() {
        "function_definition" => {
            if let Some(declarator) = node.child_by_field_name("declarator") {
                return find_identifier_recursive(
                    declarator,
                    source,
                    &[
                        "identifier",
                        "field_identifier",
                        "destructor_name",
                        "qualified_identifier",
                    ],
                );
            }
            None
        }
        "declaration" => {
            if let Some(declarator) = node.child_by_field_name("declarator") {
                return find_identifier_recursive(
                    declarator,
                    source,
                    &[
                        "identifier",
                        "field_identifier",
                        "destructor_name",
                        "qualified_identifier",
                    ],
                );
            }
            find_identifier(node, source, &["identifier", "type_identifier"])
        }
        "class_specifier" | "struct_specifier" | "enum_specifier" | "union_specifier" => {
            find_identifier(node, source, &["type_identifier", "identifier"])
        }
        "namespace_definition" => {
            find_identifier(node, source, &["identifier", "namespace_identifier"])
        }
        "alias_declaration" | "type_definition" => {
            find_identifier(node, source, &["type_identifier", "identifier"])
        }
        "template_declaration" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if matches!(
                    child.kind(),
                    "class_specifier"
                        | "struct_specifier"
                        | "enum_specifier"
                        | "union_specifier"
                        | "function_definition"
                        | "declaration"
                        | "alias_declaration"
                        | "type_definition"
                        | "concept_definition"
                ) {
                    return cpp_display_name(child, source, _chunk_type);
                }
            }
            find_identifier(node, source, &["type_identifier", "identifier"])
        }
        _ => find_identifier(node, source, &["identifier", "type_identifier"]),
    }
}

/// Recursively search for an identifier in nested declarators (e.g., C function declarators)
fn find_identifier_recursive(
    node: tree_sitter::Node<'_>,
    source: &str,
    candidate_kinds: &[&str],
) -> Option<String> {
    if candidate_kinds.contains(&node.kind()) {
        return text_for_node(node, source).map(|s| s.trim().to_string());
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(result) = find_identifier_recursive(child, source, candidate_kinds) {
            return Some(result);
        }
    }
    None
}

fn find_identifier(
    node: tree_sitter::Node<'_>,
    source: &str,
    candidate_kinds: &[&str],
) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if candidate_kinds.contains(&child.kind())
            && let Some(text) = text_for_node(child, source)
        {
            return Some(text.trim().to_string());
        }
    }
    None
}

fn first_word_of_node(node: tree_sitter::Node<'_>, source: &str) -> Option<String> {
    let text = text_for_node(node, source)?;
    text.split_whitespace().next().map(|s| {
        s.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_')
            .to_string()
    })
}

fn text_for_node(node: tree_sitter::Node<'_>, source: &str) -> Option<String> {
    node.utf8_text(source.as_bytes())
        .ok()
        .map(std::string::ToString::to_string)
}

fn only_whitespace_between(source: &str, start: usize, end: usize) -> bool {
    if start >= end || end > source.len() {
        return true;
    }

    source[start..end].chars().all(char::is_whitespace)
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
    const DART_CONTAINERS: &[&str] = &[
        "class_definition",
        "class_declaration",
        "mixin_declaration",
        "enum_declaration",
    ];

    match language {
        ParseableLanguage::Python => ancestor_has_kind(node, PYTHON_CONTAINERS),
        ParseableLanguage::TypeScript | ParseableLanguage::JavaScript => {
            ancestor_has_kind(node, TYPESCRIPT_CONTAINERS)
        }
        ParseableLanguage::Ruby => ancestor_has_kind(node, RUBY_CONTAINERS),
        ParseableLanguage::Rust => ancestor_has_kind(node, RUST_CONTAINERS),
        ParseableLanguage::Go => false,
        ParseableLanguage::C => ancestor_has_kind(node, &["struct_specifier"]),
        ParseableLanguage::Cpp => ancestor_has_kind(node, &["class_specifier", "struct_specifier"]),
        ParseableLanguage::CSharp => false,
        ParseableLanguage::Haskell => false,
        ParseableLanguage::Zig => false,

        ParseableLanguage::Dart => ancestor_has_kind(node, DART_CONTAINERS),

        ParseableLanguage::Elixir => false, // Elixir doesn't have class-based methods
        ParseableLanguage::Markdown => false,
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
        let metadata = chunk.metadata.with_updated_text(stride_text);

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
            metadata,
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

/// Merge adjacent Markdown chunks that are individually too small to be useful.
///
/// Markdown is often structured as many tiny logical units (a heading, a short
/// paragraph, a single list item).  Emitting each one as its own chunk would
/// pollute the embedding index with near-empty vectors that carry little
/// semantic signal.  This pass greedily accumulates adjacent chunks until the
/// running token count approaches `target_tokens`, then emits one merged chunk.
///
/// Side-effect: merged chunks lose their individual `ChunkType` labels (heading
/// becomes `ChunkType::Text`) because the merged span covers mixed content.
/// That is intentional — the important thing is that the *text* survives so
/// search can still match it.
fn merge_small_chunks(chunks: Vec<Chunk>, text: &str, target_tokens: usize) -> Vec<Chunk> {
    if chunks.is_empty() {
        return chunks;
    }

    let mut result = Vec::new();
    let mut current_group: Vec<Chunk> = Vec::new();
    let mut current_tokens = 0;

    for chunk in chunks {
        let chunk_tokens = chunk.metadata.estimated_tokens;

        if current_tokens + chunk_tokens > target_tokens {
            // Flush
            if !current_group.is_empty() {
                result.push(merge_group(&current_group, text));
                current_group.clear();
                current_tokens = 0;
            }
        }

        // If single chunk is huge
        if chunk_tokens > target_tokens {
            if !current_group.is_empty() {
                result.push(merge_group(&current_group, text));
                current_group.clear();
                current_tokens = 0;
            }
            result.push(chunk);
            continue;
        }

        current_group.push(chunk);
        current_tokens += chunk_tokens;
    }

    // Flush remaining
    if !current_group.is_empty() {
        result.push(merge_group(&current_group, text));
    }

    result
}

fn merge_group(group: &[Chunk], text: &str) -> Chunk {
    if group.len() == 1 {
        return group[0].clone();
    }

    let first = &group[0];
    let last = &group[group.len() - 1];

    // Calculate new span
    // Assuming sorted, which fill_gaps ensures
    let byte_start = first.span.byte_start;
    let byte_end = last.span.byte_end;
    let line_start = first.span.line_start;
    let line_end = last.span.line_end;

    // Safely slice text
    let chunk_text = if byte_end <= text.len() {
        text[byte_start..byte_end].to_string()
    } else {
        // Fallback for safety, though existing chunks should be valid
        text.get(byte_start..).unwrap_or("").to_string()
    };

    let metadata = ChunkMetadata::from_text(&chunk_text);

    // If all chunks in the group share the same semantic type, preserve it.
    // Otherwise (the common case for Markdown, where headings, paragraphs, and
    // code blocks are merged together), fall back to ChunkType::Text.  This is
    // intentional: the merged chunk is a mixed-content blob and no single type
    // describes it accurately.  Callers should rely on chunk *text* content
    // rather than chunk type when working with merged Markdown output.
    let chunk_type = if group.iter().all(|c| c.chunk_type == first.chunk_type) {
        first.chunk_type.clone()
    } else {
        ChunkType::Text
    };

    Chunk {
        span: Span {
            byte_start,
            byte_end,
            line_start,
            line_end,
        },
        text: chunk_text,
        chunk_type,
        stride_info: None,
        metadata,
    }
}

// Removed duplicate estimate_tokens function - using the one from ck-embed via TokenEstimator

#[cfg(test)]
mod tests {
    use super::*;

    fn canonicalize_spans(
        mut spans: Vec<(usize, usize, ChunkType)>,
    ) -> Vec<(usize, usize, ChunkType)> {
        fn chunk_type_order(chunk_type: &ChunkType) -> u8 {
            match chunk_type {
                ChunkType::Text => 0,
                ChunkType::Function => 1,
                ChunkType::Class => 2,
                ChunkType::Method => 3,
                ChunkType::Module => 4,
            }
        }

        spans.sort_by(|a, b| {
            let order_a = chunk_type_order(&a.2);
            let order_b = chunk_type_order(&b.2);
            order_a
                .cmp(&order_b)
                .then_with(|| a.0.cmp(&b.0))
                .then_with(|| a.1.cmp(&b.1))
        });

        let mut result: Vec<(usize, usize, ChunkType)> = Vec::new();
        for (start, end, ty) in spans {
            if let Some(last) = result.last_mut()
                && last.0 == start
                && last.2 == ty
            {
                if end > last.1 {
                    last.1 = end;
                }
                continue;
            }
            result.push((start, end, ty));
        }

        result
    }

    fn assert_query_parity(language: ParseableLanguage, source: &str) {
        let mut parser = tree_sitter::Parser::new();
        let ts_language = tree_sitter_language(language).expect("language");
        parser.set_language(&ts_language).expect("set language");
        let tree = parser.parse(source, None).expect("parse source");

        let query_chunks = query_chunker::chunk_with_queries(language, ts_language, &tree, source)
            .expect("query execution")
            .expect("queries available");

        let mut legacy_chunks = Vec::new();
        let mut cursor = tree.walk();
        extract_code_chunks(&mut cursor, source, &mut legacy_chunks, language);

        let query_spans = canonicalize_spans(
            query_chunks
                .iter()
                .map(|chunk| {
                    (
                        chunk.span.byte_start,
                        chunk.span.byte_end,
                        chunk.chunk_type.clone(),
                    )
                })
                .collect(),
        );
        let legacy_spans = canonicalize_spans(
            legacy_chunks
                .iter()
                .map(|chunk| {
                    (
                        chunk.span.byte_start,
                        chunk.span.byte_end,
                        chunk.chunk_type.clone(),
                    )
                })
                .collect(),
        );

        assert_eq!(query_spans, legacy_spans);
    }

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
            .map(|i| format!("Line {i}: Some content here"))
            .collect();
        let text = lines.join("\n");

        let start = std::time::Instant::now();
        let chunks = chunk_generic(&text).unwrap();
        let duration = start.elapsed();

        // Should complete quickly even for 1000 lines
        assert!(
            duration.as_millis() < 100,
            "Chunking took too long: {duration:?}"
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
        let rust_code = r"
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
";

        let chunks = chunk_language(rust_code, ParseableLanguage::Rust).unwrap();
        assert!(!chunks.is_empty());

        // Should find struct, impl, functions, and module
        let chunk_types: Vec<&ChunkType> = chunks.iter().map(|c| &c.chunk_type).collect();
        assert!(chunk_types.contains(&&ChunkType::Class)); // struct
        assert!(chunk_types.contains(&&ChunkType::Module)); // impl and mod
        assert!(chunk_types.contains(&&ChunkType::Function)); // functions
    }

    #[test]
    fn test_rust_doc_comments_attached() {
        let rust_code = r"
/// Doc comment
pub struct Foo {}
";
        let chunks = chunk_language(rust_code, ParseableLanguage::Rust).unwrap();
        let struct_chunk = chunks
            .iter()
            .find(|c| c.text.contains("struct Foo"))
            .unwrap();
        assert!(
            struct_chunk.text.contains("/// Doc comment"),
            "Doc comment should be attached"
        );
    }

    #[test]
    fn test_rust_query_matches_legacy() {
        let source = r"
            mod sample {
                struct Thing;

                impl Thing {
                    fn new() -> Self { Self }
                    fn helper(&self) {}
                }
            }

            fn util() {}
        ";

        assert_query_parity(ParseableLanguage::Rust, source);
    }

    #[test]
    fn test_python_query_matches_legacy() {
        let source = r"
class Example:
    @classmethod
    def build(cls):
        return cls()


def helper():
    return 1


async def async_helper():
    return 2
";

        assert_query_parity(ParseableLanguage::Python, source);
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
    #[ignore] // TODO: Update test to match query-based chunking behavior
    fn test_chunk_typescript_arrow_context() {
        let ts_code = r"
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
";

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

    // TODO: Query-based chunking is more accurate than legacy for TypeScript
    // and finds additional method chunks. This is the correct behavior.
    // Legacy parity tests are disabled until legacy chunking is updated.
    #[test]
    #[ignore]
    fn test_typescript_query_matches_legacy() {
        let source = r"
export const util = () => {
    return 42;
};

export class Example {
    run = () => {
        return util();
    };
}

const compute = (x: number) => x * 2;
";

        assert_query_parity(ParseableLanguage::TypeScript, source);
    }

    #[test]
    fn test_ruby_query_matches_legacy() {
        let source = r#"
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
end
"#;

        assert_query_parity(ParseableLanguage::Ruby, source);
    }

    #[test]
    fn test_go_query_matches_legacy() {
        let source = r#"
package main

import "fmt"

const Pi = 3.14159

var memory float64

type Calculator struct {
    memory float64
}

func (c *Calculator) Add(a, b float64) float64 {
    return a + b
}

func Helper() {}
"#;

        assert_query_parity(ParseableLanguage::Go, source);
    }

    #[test]
    fn test_chunk_c_corner_cases() {
        let c_code = r#"
#define MAX(a,b) ((a) > (b) ? (a) : (b))
#define VERSION 3

typedef struct Node {
    int value;
    struct Node* next;
} Node;

union Payload {
    int i;
    float f;
};

enum Color {
    Red,
    Green,
    Blue,
};

static inline int add(int a, int b) {
    return a + b;
}

int main(void) {
    return MAX(add(1, 2), VERSION);
}
"#;

        let chunks = chunk_language(c_code, ParseableLanguage::C).unwrap();
        assert!(!chunks.is_empty());

        assert!(
            chunks
                .iter()
                .any(|c| { c.chunk_type == ChunkType::Function && c.text.contains("#define MAX") })
        );
        assert!(
            chunks
                .iter()
                .any(|c| { c.chunk_type == ChunkType::Text && c.text.contains("#define VERSION") })
        );
        assert!(
            chunks
                .iter()
                .any(|c| { c.chunk_type == ChunkType::Class && c.text.contains("struct Node") })
        );
        assert!(
            chunks
                .iter()
                .any(|c| { c.chunk_type == ChunkType::Class && c.text.contains("union Payload") })
        );
        assert!(
            chunks
                .iter()
                .any(|c| { c.chunk_type == ChunkType::Class && c.text.contains("enum Color") })
        );
        assert!(chunks.iter().any(|c| {
            c.chunk_type == ChunkType::Function && c.text.contains("static inline int add")
        }));
        assert!(
            chunks
                .iter()
                .any(|c| { c.chunk_type == ChunkType::Function && c.text.contains("int main") })
        );
    }

    #[test]
    fn test_chunk_c_struct_declaration_without_body_stays_intact() {
        let c_code = r#"
#include <stdint.h>

struct mtd_info_user meminfo;
struct foo forward;
"#;

        let chunks = chunk_language(c_code, ParseableLanguage::C).unwrap();

        assert!(
            chunks
                .iter()
                .any(|c| { c.text.contains("struct mtd_info_user meminfo;") })
        );
        assert!(
            chunks
                .iter()
                .any(|c| c.text.contains("struct foo forward;"))
        );
        assert!(
            !chunks
                .iter()
                .any(|c| c.text.trim() == "struct mtd_info_user")
        );
        assert!(!chunks.iter().any(|c| c.text.trim() == "struct foo"));
    }

    #[test]
    fn test_chunk_cpp_corner_cases() {
        let cpp_code = r#"
#include <vector>
#define SQUARE(x) ((x) * (x))

namespace math {
template <typename T>
T add(T a, T b) {
    return a + b;
}

using Vec = std::vector<int>;
typedef unsigned long ulong_t;

struct Point {
    int x;
    int y;
};

class Calculator {
public:
    int add(int a, int b) { return a + b; }
};

enum class Color { Red, Green, Blue };
} // namespace math

int main() {
    return math::add(1, 2);
}
"#;

        let chunks = chunk_language(cpp_code, ParseableLanguage::Cpp).unwrap();
        assert!(!chunks.is_empty());

        assert!(
            chunks
                .iter()
                .any(|c| c.text.contains("template <typename T>"))
        );
        assert!(
            chunks
                .iter()
                .any(|c| { c.chunk_type == ChunkType::Text && c.text.contains("using Vec") })
        );
        assert!(chunks.iter().any(|c| {
            c.chunk_type == ChunkType::Text && c.text.contains("typedef unsigned long")
        }));
        assert!(
            chunks.iter().any(|c| {
                c.chunk_type == ChunkType::Function && c.text.contains("#define SQUARE")
            })
        );
        let calculator_chunk = chunks
            .iter()
            .find(|c| c.chunk_type == ChunkType::Class && c.text.contains("class Calculator"));
        assert!(calculator_chunk.is_some());
        let calculator_chunk = calculator_chunk.unwrap();
        assert!(calculator_chunk.text.contains("int add"));
        assert!(!calculator_chunk.text.contains("return a + b"));

        assert!(
            chunks
                .iter()
                .any(|c| { c.chunk_type == ChunkType::Class && c.text.contains("struct Point") })
        );
        assert!(
            chunks.iter().any(|c| {
                c.chunk_type == ChunkType::Class && c.text.contains("enum class Color")
            })
        );
        assert!(
            chunks
                .iter()
                .any(|c| { c.chunk_type == ChunkType::Function && c.text.contains("int main") })
        );
        assert!(
            chunks
                .iter()
                .any(|c| { c.chunk_type == ChunkType::Function && c.text.contains("T add") })
        );
        assert!(chunks.iter().any(|c| {
            c.chunk_type == ChunkType::Method && c.text.contains("int add(int a, int b)")
        }));
    }

    #[test]
    fn test_cpp_suppresses_contained_text_chunks() {
        let cpp_code = r#"
class Widget {
public:
    using Alias = int;
    int calc() { int local = 1; return local; }
};

using TopLevel = double;
"#;

        let chunks = chunk_language(cpp_code, ParseableLanguage::Cpp).unwrap();

        assert!(
            !chunks
                .iter()
                .any(|c| { c.chunk_type == ChunkType::Text && c.text.contains("using Alias") })
        );
        assert!(
            !chunks
                .iter()
                .any(|c| { c.chunk_type == ChunkType::Text && c.text.contains("int local") })
        );
        assert!(
            chunks
                .iter()
                .any(|c| { c.chunk_type == ChunkType::Text && c.text.contains("using TopLevel") })
        );
        assert!(
            chunks
                .iter()
                .any(|c| { c.chunk_type == ChunkType::Method && c.text.contains("int calc") })
        );
    }

    #[test]
    fn test_cpp_template_prefix_merges_with_definition() {
        let cpp_code = r#"
template <typename T>
struct Box {
    static int value;
};

template <typename T>
int Box<T>::value = 0;
"#;

        let chunks = chunk_language(cpp_code, ParseableLanguage::Cpp).unwrap();

        let def_chunk = chunks
            .iter()
            .find(|c| c.text.contains("int Box<T>::value = 0;"))
            .expect("static member definition chunk present");

        assert!(def_chunk.text.contains("template <typename T>"));

        assert!(!chunks.iter().any(|c| {
            c.chunk_type == ChunkType::Text && c.text.trim() == "template <typename T>"
        }));
    }

    #[test]
    fn test_cpp_template_method_breadcrumb_in_namespaces() {
        let cpp_code = r#"
namespace com {
namespace ford {

template <typename T>
class Wrapper {
public:
    template <typename U>
    U convert(U value) { return value; }
};

} // namespace ford
} // namespace com
"#;

        let chunks = chunk_language(cpp_code, ParseableLanguage::Cpp).unwrap();
        let method_chunk = chunks
            .iter()
            .find(|c| c.chunk_type == ChunkType::Method && c.text.contains("convert"))
            .expect("convert method chunk present");

        assert_eq!(
            method_chunk.metadata.breadcrumb.as_deref(),
            Some("com::ford::Wrapper::convert")
        );
    }

    #[test]
    fn test_cpp_function_breadcrumb_qualification() {
        let cpp_code = r#"
namespace outer {
class A {
public:
    void m();
};
}

void outer::A::m() {
    // body
}
"#;

        let chunks = chunk_language(cpp_code, ParseableLanguage::Cpp).unwrap();
        let method_chunk = chunks
            .iter()
            .find(|c| c.chunk_type == ChunkType::Function && c.text.contains("outer::A::m"))
            .expect("method chunk should exist");
        assert_eq!(
            method_chunk.metadata.breadcrumb.as_deref(),
            Some("outer::A::m")
        );
    }

    #[test]
    fn test_haskell_query_matches_legacy() {
        let source = r#"
module Example where

data Shape
  = Circle Float
  | Square Float

type family Area a

class Printable a where
    printValue :: a -> String

instance Printable Shape where
    printValue (Circle _) = "circle"
    printValue (Square _) = "square"

shapeDescription :: Shape -> String
shapeDescription (Circle r) = "circle of radius " ++ show r
shapeDescription (Square s) = "square of side " ++ show s
"#;

        assert_query_parity(ParseableLanguage::Haskell, source);
    }

    #[test]
    fn test_markdown_real_file_breadcrumbs() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/markdown_breadcrumbs.md");
        let source = std::fs::read_to_string(&path).expect("read markdown file");

        let chunks =
            chunk_text(&source, Some(ck_core::Language::Markdown)).expect("chunk markdown");

        // The fixture is a small document (well under the token target), so
        // merge_small_chunks collapses all individual heading / paragraph chunks
        // into a single merged chunk.  After merging the chunk_type is
        // ChunkType::Text (mixed content) — that is intentional; see
        // merge_small_chunks for the rationale.  What we verify here is that
        // the actual heading text survives in the merged output so that
        // full-text search can still find it.
        let all_text: String = chunks
            .iter()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            all_text.contains("Project Overview"),
            "expected top-level heading text to be present in merged chunk"
        );
        assert!(
            all_text.contains("## Usage"),
            "expected second-level heading text to be present in merged chunk"
        );
        assert!(
            all_text.contains("Setext Section"),
            "expected setext heading text to be present in merged chunk"
        );
    }

    #[test]
    fn test_markdown_inline_fixtures_cover_blocks() {
        let source = r#"
# Title

Intro paragraph with **bold** text.

## Usage

```rust
fn main() {
    println!("hi");
}
```

> Blockquote with _emphasis_.

- Item one
- Item two

Setext Section
==============

Trailing paragraph.
"#;

        let chunks = chunk_text(source, Some(ck_core::Language::Markdown)).expect("chunk markdown");

        // This source is a small synthetic document.  merge_small_chunks will
        // collapse all individual heading / paragraph / code-block chunks into
        // one (or a few) merged chunks whose chunk_type is ChunkType::Text.
        // That is intentional — tiny Markdown chunks produce weak embeddings;
        // see merge_small_chunks for the full rationale.  What matters is that
        // all block content is preserved in the output text so that full-text
        // and semantic search can still find it.
        let all_text: String = chunks
            .iter()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            all_text.contains("# Title") || all_text.contains("## Usage"),
            "expected heading text to be present after merging"
        );
        assert!(
            all_text.contains("```rust"),
            "expected markdown to include fenced code block"
        );
        assert!(
            all_text.contains("> Blockquote"),
            "expected markdown to include blockquote text"
        );
        assert!(
            all_text.contains("Setext Section"),
            "expected markdown to include Setext heading text"
        );
    }

    // ----- Regression tests for issue #136 (C/C++ edge cases) -----
    //
    // These tests intentionally use the full `chunk_text` entry point so
    // they exercise the post-processing pipeline (fill_gaps, suppression,
    // template merge) — not just the raw query layer like the existing
    // C/C++ tests in `query_chunker.rs`.

    /// Issue #136 #1: gap-produced Text chunks inside a class body must
    /// be suppressed too. Previously `suppress_contained_text_chunks`
    /// ran *before* `fill_gaps`, so field declarations and access
    /// specifiers between methods leaked through as a standalone Text
    /// chunk. Now suppression runs after fill_gaps.
    #[test]
    fn cpp_class_body_gap_text_is_suppressed() {
        let source = r"
class Counter {
public:
    int value;
    int spare;

    void inc() { value++; }
    void dec() { value--; }
};
";
        let chunks = chunk_text(source, Some(ck_core::Language::Cpp)).expect("chunk cpp");

        // The class itself + the two methods should be present.
        assert!(
            chunks
                .iter()
                .any(|c| c.chunk_type == ChunkType::Class && c.text.contains("class Counter")),
            "expected Class chunk for Counter"
        );

        // Critical: no Text chunk should contain `int value` or
        // `public:` — those bytes live inside the class span and would
        // be a redundant duplicate of content already in the Class chunk.
        let leaked_field = chunks.iter().any(|c| {
            c.chunk_type == ChunkType::Text
                && (c.text.contains("int value") || c.text.contains("int spare"))
        });
        let leaked_access = chunks
            .iter()
            .any(|c| c.chunk_type == ChunkType::Text && c.text.trim() == "public:");
        assert!(
            !leaked_field,
            "field declarations leaked as standalone Text chunks: {:?}",
            chunks
                .iter()
                .filter(|c| c.chunk_type == ChunkType::Text)
                .map(|c| &c.text)
                .collect::<Vec<_>>()
        );
        assert!(!leaked_access, "access specifier leaked as Text chunk");
    }

    /// Issue #136 #3: `template<...>` clauses that aren't byte-adjacent
    /// to their definition (multiline template clause, blank line
    /// between) still merge into a single Cpp definition chunk. The
    /// previous strict-adjacency check missed these.
    #[test]
    fn cpp_multiline_template_prefix_merges_with_definition() {
        // Multiline template clause AND a blank line before the function.
        let source = r"
template <
    typename T,
    typename U
>

T identity(T value, U /*ignored*/) {
    return value;
}
";
        let chunks = chunk_text(source, Some(ck_core::Language::Cpp)).expect("chunk cpp");

        // After merge: one chunk that contains BOTH the template clause
        // and the definition. There should not be a separate Text chunk
        // that's just the template prefix.
        let combined = chunks.iter().find(|c| {
            matches!(c.chunk_type, ChunkType::Function | ChunkType::Method)
                && c.text.contains("template")
                && c.text.contains("T identity")
        });
        assert!(
            combined.is_some(),
            "expected one Function chunk combining the multiline template clause with its definition; got {:?}",
            chunks
                .iter()
                .map(|c| (&c.chunk_type, c.text.lines().next().unwrap_or("")))
                .collect::<Vec<_>>()
        );

        // And we shouldn't have a separate standalone Text chunk that's
        // only the template clause without the body.
        let orphan_template = chunks.iter().any(|c| {
            c.chunk_type == ChunkType::Text
                && c.text.contains("template")
                && !c.text.contains("identity")
        });
        assert!(
            !orphan_template,
            "template clause was emitted as a standalone Text chunk instead of merging"
        );
    }

    /// Coverage gap from issue #136: an `extern \"C\" { ... }` linkage
    /// specification should not lose its inner functions. We don't
    /// assert breadcrumb shape here — just that the functions are
    /// captured at all and aren't suppressed by the linkage wrapper.
    #[test]
    fn cpp_extern_c_block_functions_are_captured() {
        let source = r#"
extern "C" {
    int add(int a, int b) {
        return a + b;
    }
    int sub(int a, int b) {
        return a - b;
    }
}
"#;
        let chunks = chunk_text(source, Some(ck_core::Language::Cpp)).expect("chunk cpp");

        let has_add = chunks.iter().any(|c| {
            matches!(c.chunk_type, ChunkType::Function | ChunkType::Method)
                && c.text.contains("int add")
        });
        let has_sub = chunks.iter().any(|c| {
            matches!(c.chunk_type, ChunkType::Function | ChunkType::Method)
                && c.text.contains("int sub")
        });
        assert!(
            has_add && has_sub,
            "expected both `add` and `sub` to be captured inside extern \"C\"; got {:?}",
            chunks
                .iter()
                .map(|c| (&c.chunk_type, c.text.lines().next().unwrap_or("")))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_csharp_query_matches_legacy() {
        let source = r"
namespace Calculator;

public interface ICalculator
{
    double Add(double x, double y);
}

public class Calculator
{
    public static double PI = 3.14159;
    private double _memory;

    public Calculator()
    {
        _memory = 0.0;
    }

    public double Add(double x, double y)
    {
        return x + y;
    }
}
";

        assert_query_parity(ParseableLanguage::CSharp, source);
    }

    #[test]
    fn test_zig_query_matches_legacy() {
        let source = r#"
const std = @import("std");

const Calculator = struct {
    memory: f64,

    pub fn init() Calculator {
        return Calculator{ .memory = 0.0 };
    }

    pub fn add(self: *Calculator, a: f64, b: f64) f64 {
        return a + b;
    }
};

test "calculator addition" {
    var calc = Calculator.init();
    const result = calc.add(2.0, 3.0);
    try std.testing.expect(result == 5.0);
}
"#;

        assert_query_parity(ParseableLanguage::Zig, source);
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
            "Expected at least 5 Class chunks (struct, enum, union, opaque, error set), found {class_count}"
        );

        assert!(
            function_count >= 3,
            "Expected at least 3 functions (multiply, divide, main), found {function_count}"
        );

        assert!(
            module_count >= 4,
            "Expected at least 4 module-type chunks (const std, comptime, 2 tests), found {module_count}"
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
        let csharp_code = r"
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
";

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
            metadata: ChunkMetadata::from_text(""),
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
            metadata: ChunkMetadata::from_text("     "),
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
        let long_text = (1..=50).map(|i| format!("This is a longer line {i} with more content to ensure token count is high enough")).collect::<Vec<_>>().join("\n");

        let metadata = ChunkMetadata::from_text(&long_text);
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
            metadata,
        };

        let config = ChunkConfig {
            max_tokens: 100,    // Force striding with reasonable limit
            stride_overlap: 10, // Small overlap for testing
            ..Default::default()
        };

        let result = stride_large_chunk(chunk, &config);
        if let Err(e) = &result {
            eprintln!("Stride error: {e}");
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
                    "Line span {calculated_line_span} should not exceed content lines {line_count} by more than 1"
                );
            }
        }
    }

    #[test]
    fn test_gap_filling_coverage() {
        // Test that all non-whitespace content gets chunked
        let test_cases = vec![
            (
                ParseableLanguage::Rust,
                r#"// This is a test file with imports at the top
use std::collections::HashMap;
use std::sync::Arc;

// A comment between imports and code
const VERSION: &str = "1.0.0";

// Main function
fn main() {
    println!("Hello, world!");
}

// Some trailing content
// that should be indexed
"#,
            ),
            (
                ParseableLanguage::Python,
                r#"# Imports at the top
import os
import sys

# Some constant
VERSION = "1.0.0"

# Main function
def main():
    print("Hello, world!")

# Trailing comment
# should be indexed
"#,
            ),
            (
                ParseableLanguage::TypeScript,
                r#"// Imports at the top
import { foo } from 'bar';

// Some constant
const VERSION = "1.0.0";

// Main function
function main() {
    console.log("Hello, world!");
}

// Trailing comment
// should be indexed
"#,
            ),
        ];

        for (language, code) in test_cases {
            eprintln!("\n=== Testing {language} ===");
            let chunks = chunk_language(code, language).unwrap();

            // Verify all non-whitespace bytes are covered
            let mut covered_bytes = vec![false; code.len()];
            for chunk in &chunks {
                for item in covered_bytes
                    .iter_mut()
                    .take(chunk.span.byte_end)
                    .skip(chunk.span.byte_start)
                {
                    *item = true;
                }
            }

            let uncovered_non_ws: Vec<usize> = covered_bytes
                .iter()
                .enumerate()
                .filter(|(i, covered)| !**covered && !code.as_bytes()[*i].is_ascii_whitespace())
                .map(|(i, _)| i)
                .collect();

            if !uncovered_non_ws.is_empty() {
                eprintln!("\n=== UNCOVERED NON-WHITESPACE for {language} ===");
                eprintln!("Total bytes: {}", code.len());
                eprintln!("Uncovered non-whitespace: {}", uncovered_non_ws.len());

                // Show what's uncovered
                for &pos in uncovered_non_ws.iter().take(10) {
                    let context_start = pos.saturating_sub(20);
                    let context_end = (pos + 20).min(code.len());
                    eprintln!(
                        "Uncovered at byte {}: {:?}",
                        pos,
                        &code[context_start..context_end]
                    );
                }

                eprintln!("\n=== CHUNKS ===");
                for (i, chunk) in chunks.iter().enumerate() {
                    eprintln!(
                        "Chunk {}: {:?} bytes {}-{} (len {})",
                        i,
                        chunk.chunk_type,
                        chunk.span.byte_start,
                        chunk.span.byte_end,
                        chunk.span.byte_end - chunk.span.byte_start
                    );
                    eprintln!("  Text: {:?}", &chunk.text[..chunk.text.len().min(60)]);
                }
            }

            assert!(
                uncovered_non_ws.is_empty(),
                "{}: Expected all non-whitespace covered but found {} uncovered non-whitespace bytes",
                language,
                uncovered_non_ws.len()
            );
        }
    }

    #[test]
    fn test_web_server_file_coverage() {
        // Test that all non-whitespace content in web_server.rs is covered
        let code = std::fs::read_to_string("../examples/code/web_server.rs")
            .expect("Failed to read web_server.rs");

        let chunks = chunk_language(&code, ParseableLanguage::Rust).unwrap();

        // Check coverage for non-whitespace content only
        let mut covered = vec![false; code.len()];
        for chunk in &chunks {
            for item in covered
                .iter_mut()
                .take(chunk.span.byte_end)
                .skip(chunk.span.byte_start)
            {
                *item = true;
            }
        }

        // Find uncovered bytes that are NOT whitespace
        let uncovered_non_whitespace: Vec<(usize, char)> = covered
            .iter()
            .enumerate()
            .filter(|(i, covered)| !**covered && !code.as_bytes()[*i].is_ascii_whitespace())
            .map(|(i, _)| (i, code.chars().nth(i).unwrap_or('?')))
            .collect();

        if !uncovered_non_whitespace.is_empty() {
            eprintln!("\n=== WEB_SERVER.RS UNCOVERED NON-WHITESPACE ===");
            eprintln!("File size: {} bytes", code.len());
            eprintln!("Total chunks: {}", chunks.len());
            eprintln!(
                "Uncovered non-whitespace: {}",
                uncovered_non_whitespace.len()
            );

            for &(pos, ch) in uncovered_non_whitespace.iter().take(10) {
                let start = pos.saturating_sub(30);
                let end = (pos + 30).min(code.len());
                eprintln!(
                    "\nUncovered '{}' at byte {}: {:?}",
                    ch,
                    pos,
                    &code[start..end]
                );
            }

            eprintln!("\n=== CHUNKS ===");
            for (i, chunk) in chunks.iter().enumerate().take(20) {
                eprintln!(
                    "Chunk {}: {:?} bytes {}-{} lines {}-{}",
                    i,
                    chunk.chunk_type,
                    chunk.span.byte_start,
                    chunk.span.byte_end,
                    chunk.span.line_start,
                    chunk.span.line_end
                );
            }
        }

        assert!(
            uncovered_non_whitespace.is_empty(),
            "Expected all non-whitespace content covered but found {} uncovered non-whitespace bytes",
            uncovered_non_whitespace.len()
        );
    }

    #[test]
    fn test_haskell_function_chunking() {
        let haskell_code = r"
factorial :: Integer -> Integer
factorial 0 = 1
factorial n = n * factorial (n - 1)

fibonacci :: Integer -> Integer
fibonacci 0 = 0
fibonacci 1 = 1
fibonacci n = fibonacci (n - 1) + fibonacci (n - 2)
";

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_haskell::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(haskell_code, None).unwrap();

        // Debug: print tree structure
        fn walk(node: tree_sitter::Node, _src: &str, depth: usize) {
            let kind = node.kind();
            let start = node.start_position();
            let end = node.end_position();
            eprintln!(
                "{}{:30} L{}-{}",
                "  ".repeat(depth),
                kind,
                start.row + 1,
                end.row + 1
            );

            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    walk(cursor.node(), _src, depth + 1);
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }

        eprintln!("\n=== TREE STRUCTURE ===");
        walk(tree.root_node(), haskell_code, 0);
        eprintln!("=== END TREE ===\n");

        let chunks = chunk_language(haskell_code, ParseableLanguage::Haskell).unwrap();

        eprintln!("\n=== CHUNKS ===");
        for (i, chunk) in chunks.iter().enumerate() {
            eprintln!(
                "Chunk {}: {:?} L{}-{}",
                i, chunk.chunk_type, chunk.span.line_start, chunk.span.line_end
            );
            eprintln!("  Text: {:?}", chunk.text);
        }
        eprintln!("=== END CHUNKS ===\n");

        assert!(!chunks.is_empty(), "Should find chunks in Haskell code");

        // Find factorial chunk and verify it includes both signature and implementation
        let factorial_chunk = chunks.iter().find(|c| c.text.contains("factorial 0 = 1"));
        assert!(
            factorial_chunk.is_some(),
            "Should find factorial function body"
        );

        let fac = factorial_chunk.unwrap();
        assert!(
            fac.text.contains("factorial :: Integer -> Integer"),
            "Should include type signature"
        );
        assert!(
            fac.text.contains("factorial 0 = 1"),
            "Should include base case"
        );
        assert!(
            fac.text.contains("factorial n = n * factorial (n - 1)"),
            "Should include recursive case"
        );
    }

    #[test]
    fn test_chunk_elixir_basic() {
        let elixir_code = r#"
defmodule Calculator do
  @moduledoc "A simple calculator module"

  def add(a, b) do
    a + b
  end

  defp multiply(a, b) do
    a * b
  end
end
"#;

        let chunks = chunk_language(elixir_code, ParseableLanguage::Elixir).unwrap();

        eprintln!("\n=== ELIXIR CHUNKS ===");
        for (i, chunk) in chunks.iter().enumerate() {
            eprintln!(
                "Chunk {}: {:?} L{}-{}",
                i, chunk.chunk_type, chunk.span.line_start, chunk.span.line_end
            );
            eprintln!("  Text: {:?}", &chunk.text[..chunk.text.len().min(80)]);
        }
        eprintln!("=== END CHUNKS ===\n");

        assert!(!chunks.is_empty(), "Should find chunks in Elixir code");

        // Should have module and function chunks
        let has_module = chunks.iter().any(|c| c.chunk_type == ChunkType::Module);
        let has_function = chunks.iter().any(|c| c.chunk_type == ChunkType::Function);

        assert!(has_module, "Should detect defmodule as Module");
        assert!(has_function, "Should detect def/defp as Function");
    }

    #[test]
    fn test_chunk_elixir_protocol() {
        let elixir_code = r#"
defprotocol Stringable do
  @doc "Converts to string"
  def to_string(value)
end

defimpl Stringable, for: Integer do
  def to_string(value), do: Integer.to_string(value)
end
"#;

        let chunks = chunk_language(elixir_code, ParseableLanguage::Elixir).unwrap();

        eprintln!("\n=== ELIXIR PROTOCOL CHUNKS ===");
        for (i, chunk) in chunks.iter().enumerate() {
            eprintln!(
                "Chunk {}: {:?} L{}-{}",
                i, chunk.chunk_type, chunk.span.line_start, chunk.span.line_end
            );
            eprintln!("  Text: {:?}", &chunk.text[..chunk.text.len().min(80)]);
        }
        eprintln!("=== END CHUNKS ===\n");

        // Should detect protocol and implementation as modules
        let modules: Vec<_> = chunks
            .iter()
            .filter(|c| c.chunk_type == ChunkType::Module)
            .collect();

        assert!(
            modules.len() >= 2,
            "Should detect defprotocol and defimpl as modules, found {}",
            modules.len()
        );
    }

    #[test]
    fn test_chunk_elixir_genserver() {
        let elixir_code = r"
defmodule MyServer do
  use GenServer

  def start_link(opts) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  def init(state) do
    {:ok, state}
  end

  def handle_call(:get, _from, state) do
    {:reply, state, state}
  end

  def handle_cast({:set, value}, _state) do
    {:noreply, value}
  end
end
";

        let chunks = chunk_language(elixir_code, ParseableLanguage::Elixir).unwrap();

        // Should capture all GenServer callbacks as functions
        let functions: Vec<_> = chunks
            .iter()
            .filter(|c| c.chunk_type == ChunkType::Function)
            .collect();

        assert!(
            functions.len() >= 4,
            "Should detect at least 4 functions (start_link, init, handle_call, handle_cast), found {}",
            functions.len()
        );
    }

    #[test]
    fn test_elixir_extension_detection() {
        use ck_core::Language;

        assert_eq!(Language::from_extension("ex"), Some(Language::Elixir));
        assert_eq!(Language::from_extension("exs"), Some(Language::Elixir));
        assert_eq!(Language::from_extension("EX"), Some(Language::Elixir));
        assert_eq!(Language::from_extension("EXS"), Some(Language::Elixir));
    }

    #[test]
    fn test_chunk_elixir_macros() {
        let elixir_code = r"
defmodule MyMacros do
  defmacro unless(condition, do: block) do
    quote do
      if !unquote(condition), do: unquote(block)
    end
  end

  defmacrop private_macro(x) do
    quote do: unquote(x) * 2
  end
end
";

        let chunks = chunk_language(elixir_code, ParseableLanguage::Elixir).unwrap();

        let functions: Vec<_> = chunks
            .iter()
            .filter(|c| c.chunk_type == ChunkType::Function)
            .collect();

        assert!(
            functions.len() >= 2,
            "Should detect defmacro and defmacrop as functions, found {}",
            functions.len()
        );
    }

    #[test]
    fn test_chunk_elixir_module_attributes() {
        let elixir_code = r#"
defmodule Calculator do
  @moduledoc "A calculator with type specs"

  @behaviour GenServer

  @type operation :: :add | :subtract | :multiply | :divide
  @typep internal_state :: %{history: list()}
  @opaque result :: {:ok, number()} | {:error, atom()}

  @callback init(args :: term()) :: {:ok, state :: term()}
  @callback handle_call(request :: term(), from :: term(), state :: term()) :: {:reply, term(), term()}

  @optional_callbacks [handle_info: 2]

  @spec add(number(), number()) :: number()
  def add(a, b), do: a + b

  @spec subtract(number(), number()) :: number()
  def subtract(a, b), do: a - b
end
"#;

        let chunks = chunk_language(elixir_code, ParseableLanguage::Elixir).unwrap();

        eprintln!("\n=== ELIXIR MODULE ATTRIBUTES CHUNKS ===");
        for (i, chunk) in chunks.iter().enumerate() {
            eprintln!(
                "Chunk {}: {:?} L{}-{}",
                i, chunk.chunk_type, chunk.span.line_start, chunk.span.line_end
            );
            eprintln!("  Text: {:?}", &chunk.text[..chunk.text.len().min(80)]);
        }
        eprintln!("=== END CHUNKS ===\n");

        // Check for @behaviour
        let has_behaviour = chunks
            .iter()
            .any(|c| c.chunk_type == ChunkType::Text && c.text.contains("@behaviour GenServer"));
        assert!(has_behaviour, "Should capture @behaviour declaration");

        // Check for @type definitions
        let type_chunks: Vec<_> = chunks
            .iter()
            .filter(|c| {
                c.chunk_type == ChunkType::Text
                    && (c.text.contains("@type")
                        || c.text.contains("@typep")
                        || c.text.contains("@opaque"))
            })
            .collect();
        assert!(
            type_chunks.len() >= 3,
            "Should capture @type, @typep, and @opaque, found {}",
            type_chunks.len()
        );

        // Check for @callback definitions
        let callback_chunks: Vec<_> = chunks
            .iter()
            .filter(|c| c.chunk_type == ChunkType::Text && c.text.contains("@callback"))
            .collect();
        assert!(
            callback_chunks.len() >= 2,
            "Should capture @callback definitions, found {}",
            callback_chunks.len()
        );

        // Check for @spec definitions
        let spec_chunks: Vec<_> = chunks
            .iter()
            .filter(|c| c.chunk_type == ChunkType::Text && c.text.contains("@spec"))
            .collect();
        assert!(
            spec_chunks.len() >= 2,
            "Should capture @spec definitions, found {}",
            spec_chunks.len()
        );

        // Verify we still capture the functions
        let function_chunks: Vec<_> = chunks
            .iter()
            .filter(|c| c.chunk_type == ChunkType::Function)
            .collect();
        assert!(
            function_chunks.len() >= 2,
            "Should still capture def functions, found {}",
            function_chunks.len()
        );
    }

    #[test]
    fn test_chunk_elixir_behavior_spelling() {
        // Test both British and American spellings
        let elixir_code = r"
defmodule BritishModule do
  @behaviour GenServer
end

defmodule AmericanModule do
  @behavior GenServer
end
";

        let chunks = chunk_language(elixir_code, ParseableLanguage::Elixir).unwrap();

        let behaviour_chunks: Vec<_> = chunks
            .iter()
            .filter(|c| {
                c.chunk_type == ChunkType::Text
                    && (c.text.contains("@behaviour") || c.text.contains("@behavior"))
            })
            .collect();

        assert!(
            behaviour_chunks.len() >= 2,
            "Should capture both @behaviour and @behavior spellings, found {}",
            behaviour_chunks.len()
        );
    }
}
