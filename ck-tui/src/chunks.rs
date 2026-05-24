use ck_core::{Language, Span};
use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

#[derive(Clone)]
#[allow(dead_code)]
pub struct IndexedChunkMeta {
    pub span: Span,
    pub chunk_type: Option<String>,
    pub breadcrumb: Option<String>,
    pub ancestry: Vec<String>,
    pub estimated_tokens: Option<usize>,
    pub byte_length: Option<usize>,
    pub leading_trivia: Option<Vec<String>>,
    pub trailing_trivia: Option<Vec<String>>,
}

#[derive(Clone)]
pub struct ChunkColumnChar {
    pub ch: char,
    pub is_match: bool,
}

pub enum ChunkDisplayLine {
    Label {
        prefix: usize,
        text: String,
    },
    Content {
        columns: Vec<ChunkColumnChar>,
        line_num: usize,
        text: String,
        is_match_line: bool,
        in_matched_chunk: bool,
        has_any_chunk: bool,
    },
    Message(String),
}

/// Calculate the global depth for each chunk across the entire file
pub fn calculate_chunk_depths(all_chunks: &[IndexedChunkMeta]) -> HashMap<(usize, usize), usize> {
    let mut depth_map: HashMap<(usize, usize), usize> = HashMap::new();
    let mut stack: Vec<(usize, usize, usize)> = Vec::new(); // (start, end, depth)

    // Sort chunks by start line, then by end line (descending) for consistent ordering
    let mut sorted_chunks: Vec<_> = all_chunks.iter().collect();
    sorted_chunks.sort_by_key(|meta| (meta.span.line_start, Reverse(meta.span.line_end)));

    for meta in sorted_chunks {
        let start = meta.span.line_start;
        let end = meta.span.line_end;

        // Remove chunks from stack that have ended before this chunk starts
        // Use > instead of >= so chunks ending at the same line don't affect depth
        stack.retain(|(_, stack_end, _)| *stack_end > start);

        // Current depth is the stack size
        let depth = stack.len();
        depth_map.insert((start, end), depth);

        // Add current chunk to stack
        stack.push((start, end, depth));
    }

    depth_map
}

/// Calculate the maximum nesting depth across all chunks
pub fn calculate_max_depth(all_chunks: &[IndexedChunkMeta]) -> usize {
    let depth_map = calculate_chunk_depths(all_chunks);
    depth_map.values().copied().max().unwrap_or(0) + 1 // +1 because depth is 0-indexed
}

#[allow(clippy::too_many_arguments)]
pub fn collect_chunk_display_lines(
    lines: &[String],
    context_start: usize,
    context_end: usize,
    match_line: usize,
    chunk_meta: Option<&IndexedChunkMeta>,
    all_chunks: &[IndexedChunkMeta],
    full_file_mode: bool,
) -> Vec<ChunkDisplayLine> {
    let mut rows = Vec::new();

    let first_line = context_start + 1;
    let last_line = context_end;

    // Filter out text chunks for depth calculation - they're not structural elements
    let structural_chunks: Vec<_> = all_chunks
        .iter()
        .filter(|meta| {
            meta.chunk_type
                .as_deref()
                .map(|t| t != "text")
                .unwrap_or(true)
        })
        .cloned()
        .collect();

    // Collect text chunks separately (imports, comments, etc.)
    let text_chunks: Vec<_> = all_chunks
        .iter()
        .filter(|meta| {
            meta.chunk_type
                .as_deref()
                .map(|t| t == "text")
                .unwrap_or(false)
        })
        .collect();

    // Calculate global depth for structural chunks only
    let depth_map = calculate_chunk_depths(&structural_chunks);
    let max_depth = calculate_max_depth(&structural_chunks);

    // Track chunks by their assigned depth
    let mut depth_slots: Vec<Option<&IndexedChunkMeta>> = vec![None; max_depth];
    let mut start_map: BTreeMap<usize, Vec<&IndexedChunkMeta>> = BTreeMap::new();

    // Always show all structural chunks in the visible range (like --dump-chunks)
    // The chunk_meta parameter is only used for highlighting/coloring the matched chunk
    let source_chunks: Vec<&IndexedChunkMeta> = structural_chunks
        .iter()
        .filter(|meta| {
            // Include chunks that end just before the visible window (for closing brackets)
            meta.span.line_end >= first_line.saturating_sub(1) && meta.span.line_start <= last_line
        })
        .collect();

    // Pre-populate chunks that start before the visible range
    for meta in &structural_chunks {
        if meta.span.line_start < first_line
            && meta.span.line_end >= first_line
            && let Some(&depth) = depth_map.get(&(meta.span.line_start, meta.span.line_end))
            && depth < max_depth
        {
            depth_slots[depth] = Some(meta);
        }
    }

    // Build start map for chunks starting within the visible range
    for meta in source_chunks {
        if meta.span.line_start >= first_line {
            start_map
                .entry(meta.span.line_start)
                .or_default()
                .push(meta);
        }
    }

    // Sort chunks at each start line by length (longest first)
    for starts in start_map.values_mut() {
        starts.sort_by_key(|meta| Reverse(meta.span.line_end.saturating_sub(meta.span.line_start)));
    }

    for (idx, line_text) in lines[context_start..context_end].iter().enumerate() {
        let line_num = context_start + idx + 1;
        let is_match_line = line_num == match_line;

        // Remove chunks that have ended before this line
        for slot in depth_slots.iter_mut() {
            if let Some(meta) = slot
                && meta.span.line_end < line_num
            {
                *slot = None;
            }
        }

        // Add chunks starting at this line
        if let Some(starting) = start_map.remove(&line_num) {
            for meta in starting {
                if let Some(&depth) = depth_map.get(&(meta.span.line_start, meta.span.line_end))
                    && depth < max_depth
                {
                    depth_slots[depth] = Some(meta);
                }
            }
        }

        // Add label for matched chunk at its start line
        if let Some(meta) = chunk_meta
            && line_num == meta.span.line_start
        {
            let chunk_kind = meta.chunk_type.as_deref().unwrap_or("chunk");
            let breadcrumb_text = meta
                .breadcrumb
                .as_deref()
                .filter(|crumb| !crumb.is_empty())
                .map(|crumb| format!(" ({crumb})"))
                .unwrap_or_else(|| {
                    if !meta.ancestry.is_empty() {
                        format!(" ({})", meta.ancestry.join("::"))
                    } else {
                        String::new()
                    }
                });
            let token_hint = meta
                .estimated_tokens
                .map(|tokens| format!(" • {tokens} tokens"))
                .unwrap_or_default();

            // Create a more bar-like header design with better spacing
            let bar_text = format!("{chunk_kind} {breadcrumb_text}{token_hint}");
            rows.push(ChunkDisplayLine::Label {
                prefix: max_depth,
                text: bar_text,
            });
        }

        // Handle files with no chunks
        if all_chunks.is_empty() {
            let is_boundary = line_text.trim_start().starts_with("fn ")
                || line_text.trim_start().starts_with("func ")
                || line_text.trim_start().starts_with("def ")
                || line_text.trim_start().starts_with("class ")
                || line_text.trim_start().starts_with("impl ")
                || line_text.trim_start().starts_with("struct ")
                || line_text.trim_start().starts_with("enum ");

            let columns_chars = if is_boundary {
                vec![
                    ChunkColumnChar {
                        ch: '┣',
                        is_match: false,
                    },
                    ChunkColumnChar {
                        ch: '━',
                        is_match: false,
                    },
                ]
            } else {
                Vec::new()
            };

            rows.push(ChunkDisplayLine::Content {
                columns: columns_chars,
                line_num,
                text: line_text.clone(),
                is_match_line,
                in_matched_chunk: false,
                has_any_chunk: is_boundary,
            });

            continue;
        }

        // Check if this line is covered by a text chunk (import, comment, etc.)
        let text_chunk_here = text_chunks
            .iter()
            .find(|meta| line_num >= meta.span.line_start && line_num <= meta.span.line_end);

        let has_any_structural = depth_slots.iter().any(std::option::Option::is_some);
        let has_any_chunk = has_any_structural || text_chunk_here.is_some();
        let in_matched_chunk = chunk_meta
            .map(|meta| line_num >= meta.span.line_start && line_num <= meta.span.line_end)
            .unwrap_or(false);

        // Build column characters for all depth levels (fixed width)
        let mut column_chars: Vec<ChunkColumnChar> = depth_slots
            .iter()
            .map(|slot| {
                if let Some(meta) = slot {
                    let span = &meta.span;
                    let ch = if span.line_start == span.line_end {
                        '─'
                    } else if line_num == span.line_start {
                        '┌'
                    } else if line_num == span.line_end {
                        '└'
                    } else {
                        '│'
                    };
                    let is_match = chunk_meta
                        .map(|m| {
                            m.span.line_start == span.line_start && m.span.line_end == span.line_end
                        })
                        .unwrap_or(false);
                    ChunkColumnChar { ch, is_match }
                } else {
                    ChunkColumnChar {
                        ch: ' ',
                        is_match: false,
                    }
                }
            })
            .collect();

        // If line is ONLY in text chunk (no structural chunks), show with bracket indicator
        if !has_any_structural && let Some(text_meta) = text_chunk_here {
            let ch = if text_meta.span.line_start == text_meta.span.line_end {
                // Single-line text chunk
                '·'
            } else if line_num == text_meta.span.line_start {
                // Start of multi-line text chunk
                '┌'
            } else if line_num == text_meta.span.line_end {
                // End of multi-line text chunk
                '└'
            } else {
                // Middle of multi-line text chunk
                '│'
            };

            if column_chars.is_empty() {
                column_chars.push(ChunkColumnChar {
                    ch,
                    is_match: false,
                });
            } else {
                column_chars[0].ch = ch;
            }
        }

        rows.push(ChunkDisplayLine::Content {
            columns: column_chars,
            line_num,
            text: line_text.clone(),
            is_match_line,
            in_matched_chunk,
            has_any_chunk,
        });

        // Remove chunks that end at this line
        for slot in depth_slots.iter_mut() {
            if let Some(meta) = slot
                && meta.span.line_end == line_num
            {
                *slot = None;
            }
        }
    }

    // Only show this message in single-chunk mode (not full file mode)
    if !full_file_mode && chunk_meta.is_none() && !all_chunks.is_empty() {
        rows.push(ChunkDisplayLine::Message(
            "Chunk metadata available but no matching chunk found for this line.".to_string(),
        ));
    }

    rows
}

/// Convert ChunkDisplayLine to plain text string
pub fn chunk_display_line_to_string(line: &ChunkDisplayLine) -> String {
    match line {
        ChunkDisplayLine::Label { prefix, text } => {
            format!("{}{}", " ".repeat(*prefix), text)
        }
        ChunkDisplayLine::Content {
            columns,
            line_num,
            text,
            ..
        } => {
            let mut output = String::new();

            // Render bracket columns
            for col in columns {
                output.push(col.ch);
            }

            // Add spacing
            output.push(' ');

            // Add line number with fixed width (at least 4 chars)
            output.push_str(&format!("{line_num:4} | "));

            // Add line text
            output.push_str(text);

            output
        }
        ChunkDisplayLine::Message(msg) => msg.clone(),
    }
}

/// Convert ck_chunk::Chunk to IndexedChunkMeta format
pub fn convert_chunks_to_meta(chunks: Vec<ck_chunk::Chunk>) -> Vec<IndexedChunkMeta> {
    chunks
        .iter()
        .map(|chunk| IndexedChunkMeta {
            span: chunk.span.clone(),
            chunk_type: Some(match chunk.chunk_type {
                ck_chunk::ChunkType::Function => "function".to_string(),
                ck_chunk::ChunkType::Class => "class".to_string(),
                ck_chunk::ChunkType::Method => "method".to_string(),
                ck_chunk::ChunkType::Module => "module".to_string(),
                ck_chunk::ChunkType::Text => "text".to_string(),
            }),
            breadcrumb: chunk.metadata.breadcrumb.clone(),
            ancestry: chunk.metadata.ancestry.clone(),
            byte_length: Some(chunk.metadata.byte_length),
            estimated_tokens: Some(chunk.metadata.estimated_tokens),
            leading_trivia: Some(chunk.metadata.leading_trivia.clone()),
            trailing_trivia: Some(chunk.metadata.trailing_trivia.clone()),
        })
        .collect()
}

/// Shared function to perform live chunking on a file (used by both --dump-chunks and TUI)
pub fn chunk_file_live(file_path: &Path) -> Result<(Vec<String>, Vec<IndexedChunkMeta>), String> {
    use std::fs;

    if !file_path.exists() {
        return Err(format!("File does not exist: {}", file_path.display()));
    }

    let detected_lang = Language::from_path(file_path);
    let content = fs::read_to_string(file_path)
        .map_err(|err| format!("Could not read {}: {}", file_path.display(), err))?;
    let lines: Vec<String> = content.lines().map(String::from).collect();

    // Use model-aware chunking (same approach as --dump-chunks)
    let default_model = "nomic-embed-text-v1.5";
    let chunks = ck_chunk::chunk_text_with_model(&content, detected_lang, Some(default_model))
        .map_err(|err| format!("Failed to chunk file: {err}"))?;

    // Convert chunks to IndexedChunkMeta format
    let chunk_metas = convert_chunks_to_meta(chunks);

    Ok((lines, chunk_metas))
}
