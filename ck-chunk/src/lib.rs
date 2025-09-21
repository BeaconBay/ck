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

    // Pre-compute cumulative byte offsets for O(1) lookup
    let mut line_byte_offsets = Vec::with_capacity(lines.len() + 1);
    line_byte_offsets.push(0);
    let mut cumulative_offset = 0;
    for line in &lines {
        cumulative_offset += line.len() + 1; // +1 for newline
        line_byte_offsets.push(cumulative_offset);
    }

    let mut i = 0;
    while i < lines.len() {
        let end = (i + chunk_size).min(lines.len());
        let chunk_lines = &lines[i..end];
        let chunk_text = chunk_lines.join("\n");

        let byte_start = line_byte_offsets[i];
        let byte_end = byte_start + chunk_text.len();

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
        ParseableLanguage::Python => parser.set_language(&tree_sitter_python::language())?,
        ParseableLanguage::TypeScript | ParseableLanguage::JavaScript => {
            parser.set_language(&tree_sitter_typescript::language_typescript())?
        }
        ParseableLanguage::Haskell => parser.set_language(&tree_sitter_haskell::language())?,
        ParseableLanguage::Rust => parser.set_language(&tree_sitter_rust::language())?,
        ParseableLanguage::Ruby => parser.set_language(&tree_sitter_ruby::language())?,
        ParseableLanguage::Go => parser.set_language(&tree_sitter_go::language())?,
        ParseableLanguage::CSharp => parser.set_language(&tree_sitter_c_sharp::language())?,
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

    Ok(chunks)
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
    let node_kind = node.kind();

    let is_chunk = match language {
        ParseableLanguage::Python => {
            matches!(node_kind, "function_definition" | "class_definition")
        }
        ParseableLanguage::TypeScript | ParseableLanguage::JavaScript => matches!(
            node_kind,
            "function_declaration" | "class_declaration" | "method_definition" | "arrow_function"
        ),
        ParseableLanguage::Haskell => matches!(
            node_kind,
            "signature"
                | "data_type"
                | "newtype"
                | "type_synomym"
                | "type_family"
                | "class"
                | "instance"
        ),
        ParseableLanguage::Rust => matches!(
            node_kind,
            "function_item" | "impl_item" | "struct_item" | "enum_item" | "trait_item" | "mod_item"
        ),
        ParseableLanguage::Ruby => matches!(
            node_kind,
            "method" | "class" | "module" | "singleton_method"
        ),
        ParseableLanguage::Go => matches!(
            node_kind,
            "function_declaration"
                | "method_declaration"
                | "type_declaration"
                | "var_declaration"
                | "const_declaration"
        ),
        ParseableLanguage::CSharp => matches!(
            node_kind,
            "method_declaration"
                | "class_declaration"
                | "interface_declaration"
                | "variable_declaration"
        ),
    };

    if is_chunk {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();
        let start_pos = node.start_position();
        let end_pos = node.end_position();

        let text = &source[start_byte..end_byte];

        let chunk_type = match node_kind {
            "function_definition"
            | "function_declaration"
            | "arrow_function"
            | "function"
            | "signature"
            | "function_item"
            | "def"
            | "defp"
            | "method"
            | "singleton_method"
            | "defn"
            | "defn-" => ChunkType::Function,
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
            | "type_declaration" => ChunkType::Class,
            "method_definition" | "method_declaration" | "defmacro" => ChunkType::Method,
            "data_type"
            | "newtype"
            | "type_synomym"
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
            | "variable_declaration" => ChunkType::Module,
            _ => ChunkType::Text,
        };

        chunks.push(Chunk {
            span: Span {
                byte_start: start_byte,
                byte_end: end_byte,
                line_start: start_pos.row + 1,
                line_end: end_pos.row + 1,
            },
            text: text.to_string(),
            chunk_type,
            stride_info: None,
        });
    }

    if cursor.goto_first_child() {
        loop {
            extract_code_chunks(cursor, source, chunks, language);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
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
    let text_len = text.len();

    // Early return for empty chunks to avoid divide-by-zero
    if text.is_empty() {
        return Ok(vec![chunk]);
    }

    // Calculate stride parameters in characters (approximate)
    // Use a conservative estimate to ensure we stay under token limits
    let estimated_tokens = estimate_tokens(text);
    // Guard against zero token estimate to prevent divide-by-zero panic
    let chars_per_token = if estimated_tokens == 0 {
        4.5 // Use default average if estimation fails
    } else {
        text_len as f32 / estimated_tokens as f32
    };
    let window_chars = ((config.max_tokens as f32 * 0.9) * chars_per_token) as usize; // 10% buffer
    let overlap_chars = (config.stride_overlap as f32 * chars_per_token) as usize;
    let stride_chars = window_chars.saturating_sub(overlap_chars);

    if stride_chars == 0 {
        return Err(anyhow::anyhow!("Stride size is too small"));
    }

    let mut strided_chunks = Vec::new();
    let original_chunk_id = format!("{}:{}", chunk.span.byte_start, chunk.span.byte_end);
    let mut start_pos = 0;
    let mut stride_index = 0;

    // Calculate total number of strides
    let total_strides = if text_len <= window_chars {
        1
    } else {
        ((text_len - overlap_chars) as f32 / stride_chars as f32).ceil() as usize
    };

    while start_pos < text_len {
        let end_pos = (start_pos + window_chars).min(text_len);
        let stride_text = &text[start_pos..end_pos];

        // Calculate overlap information
        let overlap_start = if stride_index > 0 { overlap_chars } else { 0 };
        let overlap_end = if end_pos < text_len { overlap_chars } else { 0 };

        // Calculate span for this stride
        let byte_offset_start = chunk.span.byte_start + start_pos;
        let byte_offset_end = chunk.span.byte_start + end_pos;

        // Estimate line numbers (approximate)
        let text_before_start = &text[..start_pos];
        let line_offset_start = text_before_start.lines().count().saturating_sub(1);
        let stride_lines = stride_text.lines().count();

        let stride_chunk = Chunk {
            span: Span {
                byte_start: byte_offset_start,
                byte_end: byte_offset_end,
                line_start: chunk.span.line_start + line_offset_start,
                // Fix: subtract 1 since stride_lines is a count but line_end should be inclusive
                line_end: chunk.span.line_start + line_offset_start + stride_lines.saturating_sub(1),
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
        if end_pos >= text_len {
            break;
        }

        start_pos += stride_chars;
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
            max_tokens: 100, // Force striding with reasonable limit
            stride_overlap: 10, // Small overlap for testing
            ..Default::default()
        };

        let result = stride_large_chunk(chunk, &config);
        if let Err(e) = &result {
            eprintln!("Stride error: {}", e);
        }
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(chunks.len() > 1, "Should create multiple chunks when striding");

        for chunk in chunks {
            // Verify line_end is not off by one
            // line_end should be inclusive and not exceed the actual content
            assert!(chunk.span.line_end >= chunk.span.line_start);

            // Check that line span makes sense for the content
            let line_count = chunk.text.lines().count();
            if line_count > 0 {
                let calculated_line_span = chunk.span.line_end - chunk.span.line_start + 1;

                // Allow some tolerance for striding logic
                assert!(calculated_line_span <= line_count + 1,
                    "Line span {} should not exceed content lines {} by more than 1",
                    calculated_line_span, line_count);
            }
        }
    }
}
