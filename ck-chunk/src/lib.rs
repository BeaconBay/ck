use anyhow::Result;
use ck_core::Span;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub span: Span,
    pub text: String,
    pub chunk_type: ChunkType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChunkType {
    Text,
    Function,
    Class,
    Method,
    Module,
}

pub fn chunk_text(text: &str, language: Option<&str>) -> Result<Vec<Chunk>> {
    tracing::debug!(
        "Chunking text with language: {:?}, length: {} chars",
        language,
        text.len()
    );

    let result = match language {
        Some("python") => {
            tracing::debug!("Using Python tree-sitter parser");
            chunk_python(text)
        }
        Some("typescript") | Some("javascript") => {
            tracing::debug!("Using TypeScript/JavaScript tree-sitter parser");
            chunk_typescript(text)
        }
        Some("haskell") => {
            tracing::debug!("Using Haskell tree-sitter parser");
            chunk_haskell(text)
        }
        _ => {
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

fn chunk_python(text: &str) -> Result<Vec<Chunk>> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_python::language())?;

    let tree = parser
        .parse(text, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse Python code"))?;

    let mut chunks = Vec::new();
    let mut cursor = tree.root_node().walk();

    extract_code_chunks(&mut cursor, text, &mut chunks, "python");

    if chunks.is_empty() {
        return chunk_generic(text);
    }

    Ok(chunks)
}

fn chunk_typescript(text: &str) -> Result<Vec<Chunk>> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_typescript::language_typescript())?;

    let tree = parser
        .parse(text, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse TypeScript code"))?;

    let mut chunks = Vec::new();
    let mut cursor = tree.root_node().walk();

    extract_code_chunks(&mut cursor, text, &mut chunks, "typescript");

    if chunks.is_empty() {
        return chunk_generic(text);
    }

    Ok(chunks)
}

fn chunk_haskell(text: &str) -> Result<Vec<Chunk>> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_haskell::language())?;

    let tree = parser
        .parse(text, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse Haskell code"))?;

    let mut chunks = Vec::new();
    let mut cursor = tree.root_node().walk();

    extract_code_chunks(&mut cursor, text, &mut chunks, "haskell");

    if chunks.is_empty() {
        return chunk_generic(text);
    }

    Ok(chunks)
}

fn extract_code_chunks(
    cursor: &mut tree_sitter::TreeCursor,
    source: &str,
    chunks: &mut Vec<Chunk>,
    language: &str,
) {
    let node = cursor.node();
    let node_kind = node.kind();

    let is_chunk = match language {
        "python" => matches!(node_kind, "function_definition" | "class_definition"),
        "typescript" | "javascript" => matches!(
            node_kind,
            "function_declaration" | "class_declaration" | "method_definition" | "arrow_function"
        ),
        "haskell" => matches!(
            node_kind,
            "signature"
                | "data_type"
                | "newtype"
                | "type_synomym"
                | "type_family"
                | "class"
                | "instance"
        ),
        _ => false,
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
            | "signature" => ChunkType::Function,
            "class_definition"
            | "class_declaration"
            | "instance_declaration"
            | "class"
            | "instance" => ChunkType::Class,
            "method_definition" => ChunkType::Method,
            "data_type" | "newtype" | "type_synomym" | "type_family" => ChunkType::Module,
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
}
