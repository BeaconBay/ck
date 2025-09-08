use anyhow::Result;
use ck_core::Span;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub span: Span,
    pub text: String,
    pub chunk_type: ChunkType,
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
            _ => Err(anyhow::anyhow!("Language {:?} is not supported for parsing", lang)),
        }
    }
}

pub fn chunk_text(text: &str, language: Option<ck_core::Language>) -> Result<Vec<Chunk>> {
    tracing::debug!("Chunking text with language: {:?}, length: {} chars", language, text.len());
    
    let result = match language.map(ParseableLanguage::try_from) {
        Some(Ok(lang)) => {
            tracing::debug!("Using {} tree-sitter parser", lang);
            chunk_language(text, lang)
        },
        Some(Err(_)) => {
            tracing::debug!("Language not supported for parsing, using generic chunking strategy");
            chunk_generic(text)
        },
        None => {
            tracing::debug!("Using generic chunking strategy");
            chunk_generic(text)
        }
    };

    match &result {
        Ok(chunks) => tracing::debug!("Successfully created {} chunks", chunks.len()),
        Err(e) => tracing::warn!("Chunking failed: {}", e),
    }

    result
}

fn chunk_generic(text: &str) -> Result<Vec<Chunk>> {
    let mut chunks = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let chunk_size = 20;
    let overlap = 5;

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
        ParseableLanguage::TypeScript | ParseableLanguage::JavaScript => parser.set_language(&tree_sitter_typescript::language_typescript())?,
        ParseableLanguage::Haskell => parser.set_language(&tree_sitter_haskell::language())?,
        ParseableLanguage::Rust => parser.set_language(&tree_sitter_rust::language())?,
        ParseableLanguage::Ruby => parser.set_language(&tree_sitter_ruby::language())?,
        ParseableLanguage::Go => parser.set_language(&tree_sitter_go::language())?,
    }
    
    let tree = parser.parse(text, None).ok_or_else(|| {
        anyhow::anyhow!("Failed to parse {} code", language)
    })?;
    
    let mut chunks = Vec::new();
    let mut cursor = tree.root_node().walk();
    
    extract_code_chunks(&mut cursor, text, &mut chunks, language);
    
    if chunks.is_empty() {
        return chunk_generic(text);
    }

    Ok(chunks)
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
        ParseableLanguage::Python => matches!(node_kind, "function_definition" | "class_definition"),
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
            "function_declaration" | "method_declaration" | "type_declaration" | "var_declaration" | "const_declaration"
        ),
    };

    if is_chunk {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();
        let start_pos = node.start_position();
        let end_pos = node.end_position();

        let text = &source[start_byte..end_byte];

        let chunk_type = match node_kind {
            "function_definition" | "function_declaration" | "arrow_function" | "function" | "signature" | "function_item" | "def" | "defp" | "method" | "singleton_method" | "defn" | "defn-" => ChunkType::Function,
            "class_definition" | "class_declaration" | "instance_declaration" | "class" | "instance" | "struct_item" | "enum_item" | "defstruct" | "defrecord" | "deftype" | "type_declaration" => ChunkType::Class,
            "method_definition" | "method_declaration" | "defmacro" => ChunkType::Method,
            "data_type" | "newtype" | "type_synomym" | "type_family" | "impl_item" | "trait_item" | "mod_item" | "defmodule" | "module" | "defprotocol" | "ns" | "var_declaration" | "const_declaration" => ChunkType::Module,
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
        assert!(chunk_types.contains(&&ChunkType::Class));  // struct and interface
        assert!(chunk_types.contains(&&ChunkType::Function)); // functions
        assert!(chunk_types.contains(&&ChunkType::Method));   // methods
    }
}
