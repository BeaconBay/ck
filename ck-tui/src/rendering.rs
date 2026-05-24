use crate::colors::*;
use crate::state::TuiState;
use crate::utils::score_to_color;
use ck_core::SearchMode;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

pub fn draw_query_input(f: &mut Frame, area: Rect, state: &TuiState) {
    let (title, style) = if state.command_mode {
        // In command mode
        (
            "Command (Enter to execute, /help for help)".to_string(),
            Style::default().fg(COLOR_CYAN).add_modifier(Modifier::BOLD),
        )
    } else {
        // In search mode
        let mode_indicator = match state.mode {
            SearchMode::Semantic => "[SEM]",
            SearchMode::Regex => "[REG]",
            SearchMode::Hybrid => "[HYB]",
            SearchMode::Lexical => "[LEX]",
        };
        (
            format!("Search {mode_indicator} (Tab to cycle, /help for commands)"),
            Style::default().fg(COLOR_YELLOW),
        )
    };

    let input = Paragraph::new(state.query.as_str())
        .style(style)
        .block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(input, area);
}

pub fn draw_results_list(f: &mut Frame, area: Rect, state: &TuiState, list_state: &mut ListState) {
    let items: Vec<ListItem> = state
        .results
        .iter()
        .enumerate()
        .map(|(idx, result)| {
            let score_color = score_to_color(result.score);
            let is_selected = state.selected_files.contains(&result.file);
            let prefix = if is_selected { "✓ " } else { "  " };
            let content = format!(
                "{}[{:.3}] {}:{}",
                prefix,
                result.score,
                result.file.display(),
                result.span.line_start
            );
            let style = if idx == state.selected_idx {
                Style::default()
                    .fg(COLOR_BLACK)
                    .bg(score_color)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default()
                    .fg(score_color)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(score_color)
            };
            ListItem::new(content).style(style)
        })
        .collect();

    let title = format!("Results ({}/{})", state.results.len(), state.results.len());
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    f.render_stateful_widget(list, area, list_state);
}

pub fn draw_preview(f: &mut Frame, area: Rect, state: &TuiState) {
    // Determine title based on preview mode and context mode
    let view_mode = if state.full_file_mode {
        "Full File"
    } else {
        "Snippet"
    };
    let title = format!(
        "{}: {:?} (^V: view | ^F: toggle | PgUp/Dn: scroll)",
        view_mode, state.preview_mode
    );

    let preview = if !state.preview_lines.is_empty() {
        Paragraph::new(state.preview_lines.clone())
            .block(Block::default().borders(Borders::ALL).title(title.clone()))
    } else {
        // Fallback to plain text
        let preview_text = if state.preview_content.is_empty() {
            "No preview available"
        } else {
            &state.preview_content
        };
        Paragraph::new(preview_text)
            .style(Style::default().fg(COLOR_WHITE))
            .block(Block::default().borders(Borders::ALL).title(title))
    };

    f.render_widget(preview, area);
}

pub fn draw_status_bar(f: &mut Frame, area: Rect, state: &TuiState) {
    let help_text = " ↑↓: Nav | Tab: Mode | ^V: View | ^Space: Select | Enter: Open | ^↑↓: History | Esc/q: Quit ";

    let mut status_spans = vec![Span::styled(
        state.status_message.clone(),
        Style::default().fg(COLOR_CYAN),
    )];

    if state.indexing_active {
        let spinner_idx = state
            .indexing_started_at
            .map(|start| ((start.elapsed().as_millis() / 120) as usize) % SPINNER_FRAMES.len())
            .unwrap_or(0);
        let spinner = SPINNER_FRAMES[spinner_idx];

        status_spans.push(Span::raw(" | "));
        status_spans.push(Span::styled(
            format!("{spinner} "),
            Style::default().fg(COLOR_YELLOW),
        ));

        // Overall percentage in fixed width, appears before the detailed message
        if let Some(progress) = state.indexing_progress {
            let pct = (progress * 100.0).clamp(0.0, 100.0).round() as i32;
            status_spans.push(Span::styled(
                format!("[{pct:>3}%] "),
                Style::default()
                    .fg(COLOR_GREEN)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        // Parse the detailed message to colorize parts
        if let Some(message) = state.indexing_message.as_ref() {
            // Split on bullet points to colorize differently
            let parts: Vec<&str> = message.split(" • ").collect();
            for (i, part) in parts.iter().enumerate() {
                if i > 0 {
                    status_spans.push(Span::styled(" • ", Style::default().fg(COLOR_DARK_GRAY)));
                }
                let color = if i == 0 {
                    COLOR_CYAN // Filename in cyan
                } else {
                    COLOR_GRAY // Counts in gray
                };
                status_spans.push(Span::styled(*part, Style::default().fg(color)));
            }
        } else {
            status_spans.push(Span::styled("Indexing...", Style::default().fg(COLOR_CYAN)));
        }
    } else if let Some(message) = state.indexing_message.as_ref() {
        status_spans.push(Span::raw(" | "));
        status_spans.push(Span::styled(
            message.clone(),
            Style::default().fg(COLOR_GRAY),
        ));
    }

    if !state.selected_files.is_empty() {
        status_spans.push(Span::raw(" | "));
        status_spans.push(Span::styled(
            format!("{} selected", state.selected_files.len()),
            Style::default().fg(COLOR_MAGENTA),
        ));
    }

    let index_info = if let Some(stats) = state.index_stats.as_ref() {
        format!(
            "Index: {} files, {} chunks",
            stats.total_files, stats.total_chunks
        )
    } else if let Some(err) = state.index_stats_error.as_ref() {
        format!("Index error: {err}")
    } else {
        "Index: --".to_string()
    };
    status_spans.push(Span::raw(" | "));
    status_spans.push(Span::styled(index_info, Style::default().fg(COLOR_GRAY)));

    status_spans.push(Span::raw(" | "));
    status_spans.push(Span::styled(
        help_text,
        Style::default().fg(COLOR_DARK_GRAY),
    ));

    let status =
        Paragraph::new(Line::from(status_spans)).block(Block::default().borders(Borders::ALL));
    f.render_widget(status, area);
}
