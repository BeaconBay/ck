use crate::chunks::IndexedChunkMeta;
use crate::colors::*;
use crate::state::TuiState;
use crate::utils::find_repo_root;
use anyhow::Result;
use ck_index::load_index_entry;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use std::path::Path;

pub fn execute_command(state: &mut TuiState) -> Result<()> {
    let cmd = state.query.trim();

    match cmd {
        "/help" | "/h" | "/?" => {
            show_help(state);
        }
        "/clear" | "/c" => {
            state.results.clear();
            state.preview_content.clear();
            state.preview_lines.clear();
            state.query.clear();
            state.command_mode = false;
            state.status_message = "Cleared results".to_string();
        }
        "/history" => {
            show_history(state);
        }
        "/stats" => {
            show_stats(state);
        }
        _ => {
            state.status_message =
                format!("Unknown command: {cmd}. Type /help for available commands");
        }
    }

    Ok(())
}

fn show_help(state: &mut TuiState) {
    let help_text = vec![
        "━━━ COMMAND MENU ━━━".to_string(),
        "".to_string(),
        "Available commands:".to_string(),
        "  /help, /h, /?    - Show this help".to_string(),
        "  /clear, /c       - Clear results and search".to_string(),
        "  /history         - Show search history".to_string(),
        "  /stats           - Show index statistics".to_string(),
        "".to_string(),
        "━━━ KEYBINDINGS ━━━".to_string(),
        "".to_string(),
        "  Tab              - Cycle search modes (SEM/REG/HYB)".to_string(),
        "  Ctrl+V           - Cycle preview modes (Heatmap/Syntax/Chunks)".to_string(),
        "  Ctrl+F           - Toggle snippet/full file view".to_string(),
        "  Ctrl+D           - Show chunk metadata (debug)".to_string(),
        "  Ctrl+Space       - Multi-select files".to_string(),
        "  Ctrl+Up/Down     - Navigate search history".to_string(),
        "  Up/Down          - Navigate results".to_string(),
        "  PgUp/PgDn        - Scroll preview".to_string(),
        "  Enter            - Open in $EDITOR".to_string(),
        "  Esc, q, Ctrl+C   - Quit".to_string(),
        "".to_string(),
        "━━━ SEARCH MODES ━━━".to_string(),
        "".to_string(),
        "  SEM - Semantic: Find code by meaning".to_string(),
        "  REG - Regex: Pattern matching".to_string(),
        "  HYB - Hybrid: Combined semantic + regex".to_string(),
        "".to_string(),
        "━━━ PREVIEW MODES ━━━".to_string(),
        "".to_string(),
        "  Heatmap - Semantic similarity coloring".to_string(),
        "  Syntax  - Syntax highlighting".to_string(),
        "  Chunks  - Function/class boundaries".to_string(),
        "".to_string(),
        "Press Esc to close help".to_string(),
    ];

    // Convert help text to colored lines
    state.preview_lines = help_text
        .iter()
        .map(|line| {
            if line.starts_with("━━━") {
                Line::from(Span::styled(
                    line.clone(),
                    Style::default().fg(COLOR_CYAN).add_modifier(Modifier::BOLD),
                ))
            } else if line.starts_with("  /")
                || line.starts_with("  Ctrl")
                || line.starts_with("  Tab")
                || line.starts_with("  Up")
                || line.starts_with("  PgUp")
                || line.starts_with("  Enter")
                || line.starts_with("  Esc")
                || line.starts_with("  SEM")
                || line.starts_with("  REG")
                || line.starts_with("  HYB")
                || line.starts_with("  Heatmap")
                || line.starts_with("  Syntax")
                || line.starts_with("  Chunks")
            {
                // Command/key on left, description on right
                if let Some(dash_pos) = line.find(" - ") {
                    let (key, desc) = line.split_at(dash_pos);
                    Line::from(vec![
                        Span::styled(
                            key.to_string(),
                            Style::default()
                                .fg(COLOR_YELLOW)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(desc.to_string(), Style::default().fg(COLOR_WHITE)),
                    ])
                } else {
                    Line::from(Span::styled(line.clone(), Style::default().fg(COLOR_WHITE)))
                }
            } else if line.starts_with("Press") {
                Line::from(Span::styled(
                    line.clone(),
                    Style::default()
                        .fg(COLOR_DARK_GRAY)
                        .add_modifier(Modifier::ITALIC),
                ))
            } else {
                Line::from(Span::styled(line.clone(), Style::default().fg(COLOR_WHITE)))
            }
        })
        .collect();

    state.query.clear();
    state.command_mode = false;
    state.status_message = "Help - Press Esc to return to search".to_string();
}

pub fn show_chunks(state: &mut TuiState) {
    // Get currently selected file
    if state.results.is_empty() {
        state.status_message = "No search results - run a search first".to_string();
        state.query.clear();
        state.command_mode = false;
        return;
    }

    let selected_file = state.results[state.selected_idx].file.clone();

    // Find repo root and load chunks
    let repo_root = find_repo_root(&selected_file);
    let all_chunks = if let Some(root) = repo_root {
        load_chunk_spans(&root, &selected_file).unwrap_or_default()
    } else {
        Vec::new()
    };

    if all_chunks.is_empty() {
        state.status_message = format!("No chunks found for {}", selected_file.display());
        state.query.clear();
        state.command_mode = false;
        return;
    }

    // Build chunk metadata display
    let mut chunks_text: Vec<String> = vec![
        format!("━━━ CHUNK METADATA: {} ━━━", selected_file.display()),
        "".to_string(),
        format!("Total chunks: {}", all_chunks.len()),
        "".to_string(),
    ];

    // Sort chunks by line_start for display
    let mut sorted_chunks = all_chunks.clone();
    sorted_chunks.sort_by_key(|c| c.span.line_start);

    // Detect overlaps
    for (i, chunk) in sorted_chunks.iter().enumerate() {
        let chunk_type = chunk.chunk_type.as_deref().unwrap_or("unknown");

        chunks_text.push(format!(
            "Chunk #{}: {} [lines {}-{}]",
            i + 1,
            chunk_type,
            chunk.span.line_start,
            chunk.span.line_end
        ));

        // Check for overlaps with other chunks
        let mut overlaps_with = Vec::new();
        for (j, other) in sorted_chunks.iter().enumerate() {
            if i == j {
                continue;
            }
            // Check if chunks overlap
            if chunk.span.line_start <= other.span.line_end
                && chunk.span.line_end >= other.span.line_start
            {
                overlaps_with.push(j + 1);
            }
        }

        if !overlaps_with.is_empty() {
            chunks_text.push(format!(
                "  Overlaps with: {}",
                overlaps_with
                    .iter()
                    .map(|n| format!("#{n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        chunks_text.push("".to_string());
    }

    chunks_text.push("Press Esc to close".to_string());

    // Convert to colored lines
    state.preview_lines = chunks_text
        .iter()
        .map(|line| {
            if line.starts_with("━━━") {
                Line::from(Span::styled(
                    line.clone(),
                    Style::default().fg(COLOR_CYAN).add_modifier(Modifier::BOLD),
                ))
            } else if line.starts_with("Chunk #") {
                Line::from(Span::styled(
                    line.clone(),
                    Style::default()
                        .fg(COLOR_YELLOW)
                        .add_modifier(Modifier::BOLD),
                ))
            } else if line.starts_with("  Overlaps") {
                Line::from(Span::styled(
                    line.clone(),
                    Style::default().fg(COLOR_MAGENTA),
                ))
            } else if line.starts_with("Total chunks") {
                Line::from(Span::styled(
                    line.clone(),
                    Style::default()
                        .fg(COLOR_GREEN)
                        .add_modifier(Modifier::BOLD),
                ))
            } else if line.starts_with("Press") {
                Line::from(Span::styled(
                    line.clone(),
                    Style::default()
                        .fg(COLOR_DARK_GRAY)
                        .add_modifier(Modifier::ITALIC),
                ))
            } else {
                Line::from(Span::styled(line.clone(), Style::default().fg(COLOR_WHITE)))
            }
        })
        .collect();

    state.query.clear();
    state.command_mode = false;
    state.scroll_offset = 0;
    state.status_message = format!(
        "Chunk metadata for {} - Press Esc to return",
        selected_file.display()
    );
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

fn show_history(state: &mut TuiState) {
    if state.search_history.is_empty() {
        state.status_message = "No search history".to_string();
        state.query.clear();
        state.command_mode = false;
        return;
    }

    let history_text: Vec<String> = std::iter::once("━━━ SEARCH HISTORY ━━━".to_string())
        .chain(std::iter::once("".to_string()))
        .chain(
            state
                .search_history
                .iter()
                .rev()
                .enumerate()
                .map(|(i, query)| format!("  {}: {}", i + 1, query)),
        )
        .chain(std::iter::once("".to_string()))
        .chain(std::iter::once(
            "Use Ctrl+Up/Down to navigate history".to_string(),
        ))
        .collect();

    state.preview_lines = history_text
        .iter()
        .map(|line| {
            if line.starts_with("━━━") {
                Line::from(Span::styled(
                    line.clone(),
                    Style::default().fg(COLOR_CYAN).add_modifier(Modifier::BOLD),
                ))
            } else if line.starts_with("  ") && line.contains(": ") {
                Line::from(Span::styled(
                    line.clone(),
                    Style::default().fg(COLOR_YELLOW),
                ))
            } else {
                Line::from(Span::styled(line.clone(), Style::default().fg(COLOR_WHITE)))
            }
        })
        .collect();

    state.query.clear();
    state.command_mode = false;
    state.status_message = "Search History".to_string();
}

fn show_stats(state: &mut TuiState) {
    let stats_text = if let Some(stats) = state.index_stats.as_ref() {
        vec![
            "━━━ INDEX STATISTICS ━━━".to_string(),
            "".to_string(),
            format!("  Path: {}", state.search_path.display()),
            format!("  Files: {}", stats.total_files),
            format!(
                "  Chunks: {} ({} embedded)",
                stats.total_chunks, stats.embedded_chunks
            ),
            format!("  Total size: {} bytes", stats.total_size_bytes),
            format!("  Index size: {} bytes", stats.index_size_bytes),
            "".to_string(),
        ]
    } else if let Some(err) = state.index_stats_error.as_ref() {
        vec![
            "━━━ INDEX STATISTICS ━━━".to_string(),
            "".to_string(),
            format!("  Error: {}", err),
            "".to_string(),
        ]
    } else {
        vec![
            "━━━ INDEX STATISTICS ━━━".to_string(),
            "".to_string(),
            "  Index data unavailable".to_string(),
            "".to_string(),
        ]
    };

    state.preview_lines = stats_text
        .iter()
        .map(|line| {
            if line.starts_with("━━━") {
                Line::from(Span::styled(
                    line.clone(),
                    Style::default().fg(COLOR_CYAN).add_modifier(Modifier::BOLD),
                ))
            } else if line.starts_with("  ") {
                Line::from(Span::styled(
                    line.clone(),
                    Style::default().fg(COLOR_YELLOW),
                ))
            } else {
                Line::from(Span::styled(line.clone(), Style::default().fg(COLOR_WHITE)))
            }
        })
        .collect();

    state.query.clear();
    state.command_mode = false;
    state.status_message = "Index Statistics".to_string();
}
