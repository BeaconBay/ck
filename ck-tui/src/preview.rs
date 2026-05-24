use crate::chunks::{
    ChunkDisplayLine, IndexedChunkMeta, chunk_file_live, collect_chunk_display_lines,
};
use crate::colors::*;
use crate::utils::{
    apply_heatmap_color_to_token, calculate_token_similarity, find_repo_root, split_into_tokens,
    syntax_set, theme_set,
};
use ck_core::pdf;
use ck_index::load_index_entry;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::fs;
use std::path::{Path, PathBuf};
use syntect::easy::HighlightLines;

pub fn load_preview_lines(
    path: &Path,
) -> Result<(Vec<String>, bool, Vec<IndexedChunkMeta>), String> {
    let resolved_path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let repo_root = find_repo_root(&resolved_path);
    let is_pdf = pdf::is_pdf_file(&resolved_path);

    let (_content, lines) = if is_pdf {
        let root = repo_root.clone().ok_or_else(|| {
            "PDF preview unavailable (missing .ck index). Run `ck --index .` first.".to_string()
        })?;

        let cache_path = pdf::get_content_cache_path(&root, &resolved_path);
        let content = fs::read_to_string(&cache_path).map_err(|err| {
            format!("PDF preview unavailable ({err}). Run `ck --index .` to generate cache.")
        })?;
        let lines: Vec<String> = content
            .lines()
            .map(std::string::ToString::to_string)
            .collect();
        (content, lines)
    } else {
        let content = fs::read_to_string(&resolved_path)
            .map_err(|err| format!("Could not read {}: {}", resolved_path.display(), err))?;
        let lines: Vec<String> = content
            .lines()
            .map(std::string::ToString::to_string)
            .collect();
        (content, lines)
    };

    // Use live chunking instead of cached index data (same approach as --dump-chunks)
    let chunk_spans = if is_pdf {
        // For PDFs, we still need to fall back to cached data since we can't chunk PDF content directly
        if let Some(root) = repo_root {
            load_chunk_spans(&root, &resolved_path).unwrap_or_default()
        } else {
            Vec::new()
        }
    } else {
        // For regular files, use live chunking with fallback to cached data
        match chunk_file_live(&resolved_path) {
            Ok((_, chunks)) => chunks,
            Err(_) => {
                // If live chunking fails, fall back to cached data if available
                if let Some(root) = repo_root {
                    load_chunk_spans(&root, &resolved_path).unwrap_or_default()
                } else {
                    Vec::new()
                }
            }
        }
    };

    Ok((lines, is_pdf, chunk_spans))
}

fn load_chunk_spans(repo_root: &Path, file_path: &Path) -> Result<Vec<IndexedChunkMeta>, String> {
    let standard_path = file_path
        .strip_prefix(repo_root)
        .unwrap_or(file_path)
        .to_path_buf();
    let index_dir = repo_root.join(".ck");
    let sidecar_path = index_dir.join(format!("{}.ck", standard_path.display()));

    if !sidecar_path.exists() {
        return Ok(Vec::new());
    }

    let entry = load_index_entry(&sidecar_path)
        .map_err(|err| format!("Failed to load chunk metadata: {err}"))?;
    let mut metas: Vec<IndexedChunkMeta> = entry
        .chunks
        .iter()
        .map(|chunk| IndexedChunkMeta {
            span: chunk.span.clone(),
            chunk_type: chunk.chunk_type.clone(),
            breadcrumb: chunk.breadcrumb.clone(),
            ancestry: chunk.ancestry.clone().unwrap_or_default(),
            estimated_tokens: chunk.estimated_tokens,
            byte_length: chunk.byte_length,
            leading_trivia: chunk.leading_trivia.clone(),
            trailing_trivia: chunk.trailing_trivia.clone(),
        })
        .collect();

    let has_non_module = metas
        .iter()
        .any(|meta| meta.chunk_type.as_deref() != Some("module"));
    if has_non_module {
        metas.retain(|meta| meta.chunk_type.as_deref() != Some("module"));
    }

    Ok(metas)
}

#[allow(clippy::too_many_arguments)]
pub fn render_heatmap_preview(
    lines: &[String],
    context_start: usize,
    context_end: usize,
    file_path: &Path,
    score: f32,
    match_line: usize,
    query: &str,
) -> Vec<Line<'static>> {
    let mut colored_lines = Vec::new();

    // Header line
    colored_lines.push(Line::from(vec![Span::styled(
        format!("File: {} | Score: {:.3}\n", file_path.display(), score),
        Style::default().fg(COLOR_CYAN),
    )]));

    // Apply heatmap to each line
    for (idx, line) in lines[context_start..context_end].iter().enumerate() {
        let line_num = context_start + idx + 1;
        let is_match_line = line_num == match_line;
        let in_chunk_range = line_num >= match_line.saturating_sub(5) && line_num <= match_line + 5;

        let mut line_spans = vec![Span::styled(
            format!("{line_num:4} | "),
            if is_match_line {
                Style::default()
                    .fg(COLOR_YELLOW)
                    .add_modifier(Modifier::BOLD)
            } else if in_chunk_range {
                Style::default().fg(COLOR_CYAN) // Chunk region in cyan
            } else {
                Style::default().fg(COLOR_DARK_GRAY)
            },
        )];

        // Apply heatmap coloring
        let tokens = split_into_tokens(line);
        for token in tokens {
            let similarity = calculate_token_similarity(&token, query);
            let color = apply_heatmap_color_to_token(&token, similarity);

            let style = if color == Color::Reset {
                Style::default().fg(COLOR_WHITE)
            } else {
                Style::default().fg(color)
            };

            line_spans.push(Span::styled(token.to_string(), style));
        }

        colored_lines.push(Line::from(line_spans));
    }

    colored_lines
}

#[allow(clippy::too_many_arguments)]
pub fn render_syntax_preview(
    lines: &[String],
    context_start: usize,
    context_end: usize,
    file_path: &PathBuf,
    score: f32,
    match_line: usize,
) -> Vec<Line<'static>> {
    let mut colored_lines = Vec::new();

    // Header
    colored_lines.push(Line::from(vec![Span::styled(
        format!("File: {} | Score: {:.3}\n", file_path.display(), score),
        Style::default().fg(COLOR_CYAN),
    )]));

    // Initialize syntect assets once
    let ps = syntax_set();
    let ts = theme_set();
    let theme = ts
        .themes
        .get("base16-ocean.dark")
        .or_else(|| ts.themes.values().next());

    // Detect syntax from file extension
    let syntax = ps
        .find_syntax_for_file(file_path)
        .ok()
        .flatten()
        .unwrap_or_else(|| ps.find_syntax_plain_text());

    let mut highlighter = match theme {
        Some(theme) => HighlightLines::new(syntax, theme),
        None => {
            // Fallback: render without syntax colors
            for (idx, line) in lines[context_start..context_end].iter().enumerate() {
                let line_num = context_start + idx + 1;
                let is_match_line = line_num == match_line;
                let in_chunk_range =
                    line_num >= match_line.saturating_sub(5) && line_num <= match_line + 5;

                let line_spans = vec![
                    Span::styled(
                        format!("{line_num:4} | "),
                        if is_match_line {
                            Style::default()
                                .fg(COLOR_YELLOW)
                                .add_modifier(Modifier::BOLD)
                        } else if in_chunk_range {
                            Style::default().fg(COLOR_CYAN)
                        } else {
                            Style::default().fg(COLOR_DARK_GRAY)
                        },
                    ),
                    Span::styled(line.to_string(), Style::default().fg(COLOR_WHITE)),
                ];

                colored_lines.push(Line::from(line_spans));
            }

            return colored_lines;
        }
    };

    // Apply syntax highlighting
    for (idx, line) in lines[context_start..context_end].iter().enumerate() {
        let line_num = context_start + idx + 1;
        let is_match_line = line_num == match_line;
        let in_chunk_range = line_num >= match_line.saturating_sub(5) && line_num <= match_line + 5;

        let mut line_spans = vec![Span::styled(
            format!("{line_num:4} | "),
            if is_match_line {
                Style::default()
                    .fg(COLOR_YELLOW)
                    .add_modifier(Modifier::BOLD)
            } else if in_chunk_range {
                Style::default().fg(COLOR_CYAN) // Chunk region in cyan
            } else {
                Style::default().fg(COLOR_DARK_GRAY)
            },
        )];

        // Highlight the line
        if let Ok(ranges) = highlighter.highlight_line(line, ps) {
            for (style, text) in ranges {
                let fg = style.foreground;
                let color = Color::Rgb(fg.r, fg.g, fg.b);
                line_spans.push(Span::styled(text.to_string(), Style::default().fg(color)));
            }
        } else {
            line_spans.push(Span::raw(line.to_string()));
        }

        colored_lines.push(Line::from(line_spans));
    }

    colored_lines
}

#[allow(clippy::too_many_arguments)]
pub fn render_chunks_preview(
    lines: &[String],
    context_start: usize,
    context_end: usize,
    file_path: &Path,
    score: f32,
    match_line: usize,
    chunk_meta: Option<&IndexedChunkMeta>,
    is_pdf: bool,
    all_chunks: &[IndexedChunkMeta],
    full_file_mode: bool,
    disable_match_highlighting: bool,
) -> Vec<Line<'static>> {
    let mut colored_lines = Vec::new();

    let header = if let Some(meta) = chunk_meta {
        let span = &meta.span;
        let chunk_kind = meta.chunk_type.as_deref().unwrap_or("chunk");
        let breadcrumb_display = meta
            .breadcrumb
            .as_deref()
            .filter(|crumb| !crumb.is_empty())
            .map(|crumb| format!(" • {crumb}"))
            .unwrap_or_else(|| {
                if !meta.ancestry.is_empty() {
                    format!(" • {}", meta.ancestry.join("::"))
                } else {
                    String::new()
                }
            });
        let token_display = meta
            .estimated_tokens
            .map(|tokens| format!(" • ~{tokens} tokens"))
            .unwrap_or_default();

        format!(
            "File: {} • Score: {:.3}\n{}{}{} • L{}-{}\n",
            file_path.display(),
            score,
            chunk_kind,
            breadcrumb_display,
            token_display,
            span.line_start,
            span.line_end
        )
    } else if is_pdf {
        format!(
            "File: {} • Score: {:.3}
PDF chunk (approximate)
",
            file_path.display(),
            score
        )
    } else {
        format!(
            "File: {} • Score: {:.3}
",
            file_path.display(),
            score
        )
    };

    colored_lines.push(Line::from(vec![Span::styled(
        header,
        Style::default().fg(COLOR_CYAN),
    )]));

    colored_lines.extend(build_chunk_lines(
        lines,
        context_start,
        context_end,
        match_line,
        chunk_meta,
        all_chunks,
        full_file_mode,
        disable_match_highlighting,
    ));

    colored_lines
}

#[allow(clippy::too_many_arguments)]
pub fn build_chunk_lines(
    lines: &[String],
    context_start: usize,
    context_end: usize,
    match_line: usize,
    chunk_meta: Option<&IndexedChunkMeta>,
    all_chunks: &[IndexedChunkMeta],
    full_file_mode: bool,
    disable_match_highlighting: bool,
) -> Vec<Line<'static>> {
    // Calculate the width needed for line numbers
    let max_line_num = lines.len();
    let line_num_width = max_line_num.to_string().len() + 1; // +1 for spacing

    collect_chunk_display_lines(
        lines,
        context_start,
        context_end,
        if disable_match_highlighting {
            0
        } else {
            match_line
        },
        chunk_meta,
        all_chunks,
        full_file_mode,
    )
    .into_iter()
    .map(|row| match row {
        ChunkDisplayLine::Label { prefix, text } => {
            let mut spans = Vec::new();

            // Add indentation
            spans.push(Span::styled(
                " ".repeat(prefix),
                Style::default().fg(COLOR_DARK_GRAY),
            ));

            // Create a bar-like header with borders
            let bar_start = "┌─ ";
            let bar_end = " ─┐";

            // Left border
            spans.push(Span::styled(
                bar_start,
                Style::default()
                    .fg(COLOR_CHUNK_BOUNDARY)
                    .add_modifier(Modifier::BOLD),
            ));

            // Content with background-like effect
            spans.push(Span::styled(
                text,
                Style::default()
                    .fg(COLOR_CHUNK_TEXT)
                    .bg(COLOR_CHUNK_BOUNDARY)
                    .add_modifier(Modifier::BOLD),
            ));

            // Right border
            spans.push(Span::styled(
                bar_end,
                Style::default()
                    .fg(COLOR_CHUNK_BOUNDARY)
                    .add_modifier(Modifier::BOLD),
            ));

            Line::from(spans)
        }
        ChunkDisplayLine::Content {
            columns,
            line_num,
            text,
            is_match_line,
            in_matched_chunk,
            has_any_chunk,
        } => {
            let mut spans = Vec::new();

            // Always render chunk columns with fixed width
            if columns.is_empty() {
                spans.push(Span::styled(" ", Style::default().fg(COLOR_DARK_GRAY)));
            } else {
                for column in columns {
                    let mut style = Style::default().fg(if column.is_match {
                        COLOR_CHUNK_HIGHLIGHT // Orange for highlighted chunk boundaries
                    } else {
                        COLOR_CHUNK_BOUNDARY // Spring green for regular chunk boundaries
                    });
                    if column.is_match {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    spans.push(Span::styled(column.ch.to_string(), style));
                }
            }

            spans.push(Span::styled(" ", Style::default().fg(COLOR_DARK_GRAY)));

            // Use fixed-width line number formatting
            spans.push(Span::styled(
                format!("{line_num:line_num_width$} | "),
                if is_match_line {
                    Style::default()
                        .fg(COLOR_YELLOW)
                        .add_modifier(Modifier::BOLD)
                } else if in_matched_chunk {
                    Style::default()
                        .fg(COLOR_CHUNK_LINE_NUM) // Gold for highlighted chunk line numbers
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(COLOR_GRAY)
                },
            ));

            spans.push(Span::styled(
                text,
                if in_matched_chunk {
                    Style::default()
                        .fg(COLOR_CHUNK_TEXT) // Bright white for highlighted chunk text
                        .add_modifier(Modifier::BOLD)
                } else if has_any_chunk {
                    Style::default().fg(COLOR_WHITE) // Regular white for chunk text
                } else {
                    Style::default().fg(COLOR_DARK_GRAY) // Dim for non-chunk text
                },
            ));

            Line::from(spans)
        }
        ChunkDisplayLine::Message(message) => Line::from(vec![Span::styled(
            message,
            Style::default()
                .fg(COLOR_CHUNK_BOUNDARY)
                .add_modifier(Modifier::ITALIC),
        )]),
    })
    .collect()
}

#[allow(dead_code)]
pub fn build_chunk_strings(
    lines: &[String],
    context_start: usize,
    context_end: usize,
    match_line: usize,
    chunk_meta: Option<&IndexedChunkMeta>,
    all_chunks: &[IndexedChunkMeta],
    full_file_mode: bool,
) -> Vec<String> {
    // Calculate the width needed for line numbers
    let max_line_num = lines.len();
    let line_num_width = max_line_num.to_string().len() + 1; // +1 for spacing

    collect_chunk_display_lines(
        lines,
        context_start,
        context_end,
        match_line,
        chunk_meta,
        all_chunks,
        full_file_mode,
    )
    .into_iter()
    .map(|row| match row {
        ChunkDisplayLine::Label { prefix, text } => {
            format!("{}{}", " ".repeat(prefix), text)
        }
        ChunkDisplayLine::Content {
            columns,
            line_num,
            text,
            is_match_line,
            ..
        } => {
            let mut line_buf = String::new();
            if columns.is_empty() {
                line_buf.push(' ');
            } else {
                for column in columns {
                    line_buf.push(column.ch);
                }
            }
            line_buf.push(' ');
            line_buf.push_str(&format!("{line_num:line_num_width$} | {text}"));
            if is_match_line {
                line_buf.push_str("  <= match");
            }
            line_buf
        }
        ChunkDisplayLine::Message(message) => message,
    })
    .collect()
}

#[allow(dead_code)]
pub fn dump_chunk_view_internal(
    path: &Path,
    match_line: Option<usize>,
    full_file_mode: bool,
) -> Result<Vec<String>, String> {
    let (lines, is_pdf, chunk_spans) = load_preview_lines(path)?;

    if lines.is_empty() {
        return Ok(vec![format!("File: {} (empty)", path.display())]);
    }

    let total_lines = lines.len();
    let mut line_to_focus = match_line
        .or_else(|| chunk_spans.first().map(|meta| meta.span.line_start))
        .unwrap_or(1)
        .clamp(1, total_lines);

    let chunk_meta = chunk_spans
        .iter()
        .filter(|meta| {
            let span = &meta.span;
            line_to_focus >= span.line_start && line_to_focus <= span.line_end
        })
        .min_by_key(|meta| meta.span.line_end.saturating_sub(meta.span.line_start));

    if chunk_meta.is_none() && chunk_spans.is_empty() {
        line_to_focus = line_to_focus.clamp(1, total_lines);
    }

    let mut context_start = if full_file_mode {
        0
    } else {
        line_to_focus
            .saturating_sub(6)
            .min(total_lines.saturating_sub(1))
    };
    let mut context_end = if full_file_mode {
        total_lines
    } else {
        (line_to_focus + 5).min(total_lines)
    };

    if !full_file_mode && let Some(meta) = chunk_meta {
        context_start = meta
            .span
            .line_start
            .saturating_sub(1)
            .min(total_lines.saturating_sub(1));
        context_end = meta.span.line_end.min(total_lines);
    }

    if context_end <= context_start {
        context_end = (context_start + 1).min(total_lines);
    }

    let mut output = Vec::new();

    let header_lines: Vec<String> = if let Some(meta) = chunk_meta {
        let span = &meta.span;
        let chunk_kind = meta.chunk_type.as_deref().unwrap_or("chunk");
        let breadcrumb_display = meta
            .breadcrumb
            .as_deref()
            .filter(|crumb| !crumb.is_empty())
            .map(|crumb| format!(" • {crumb}"))
            .unwrap_or_else(|| {
                if !meta.ancestry.is_empty() {
                    format!(" • {}", meta.ancestry.join("::"))
                } else {
                    String::new()
                }
            });
        let token_display = meta
            .estimated_tokens
            .map(|tokens| format!(" • ~{tokens} tokens"))
            .unwrap_or_default();

        vec![
            format!("File: {}", path.display()),
            format!(
                "{}{}{} • L{}-{}",
                chunk_kind, breadcrumb_display, token_display, span.line_start, span.line_end
            ),
            String::new(),
        ]
    } else if is_pdf {
        vec![
            format!("File: {}", path.display()),
            "PDF chunk (approximate)".to_string(),
            String::new(),
        ]
    } else {
        vec![format!("File: {}", path.display()), String::new()]
    };

    output.extend(header_lines);

    let body = build_chunk_strings(
        &lines,
        context_start,
        context_end,
        line_to_focus,
        chunk_meta,
        &chunk_spans,
        full_file_mode,
    );

    output.extend(body);

    Ok(output)
}
