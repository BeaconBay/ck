use crate::colors::DEBOUNCE_MS;
use crate::commands::{execute_command, show_chunks};
use crate::config::{PreviewMode, TuiConfig};
use crate::events::UiEvent;
use crate::preview::{
    load_preview_lines, render_chunks_preview, render_heatmap_preview, render_syntax_preview,
};
use crate::rendering::{draw_preview, draw_query_input, draw_results_list, draw_status_bar};
use crate::state::{PreviewCache, TuiState};
use anyhow::Result;
use ck_core::{SearchMode, SearchOptions};
use ck_index::get_index_stats;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    widgets::ListState,
};
use shlex::split;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::task::JoinHandle;

pub struct TuiApp {
    pub state: TuiState,
    pub list_state: ListState,
    last_search_time: Instant,
    search_pending: bool,
    progress_tx: UnboundedSender<UiEvent>,
    progress_rx: UnboundedReceiver<UiEvent>,
    current_generation: u64,
    active_search: Option<JoinHandle<()>>,
}

impl TuiApp {
    pub fn new(search_path: PathBuf, initial_query: Option<String>) -> Self {
        let query = initial_query.unwrap_or_default();
        let config = TuiConfig::load();
        let (progress_tx, progress_rx) = unbounded_channel();

        let mut app = Self {
            state: TuiState {
                query: query.clone(),
                mode: config.search_mode.clone(),
                results: Vec::new(),
                selected_idx: 0,
                preview_content: String::new(),
                preview_lines: Vec::new(),
                preview_mode: config.preview_mode.clone(),
                full_file_mode: config.full_file_mode,
                scroll_offset: 0,
                status_message: "Ready. Type to search...".to_string(),
                search_path,
                selected_files: Default::default(),
                search_history: if !query.is_empty() {
                    vec![query]
                } else {
                    Vec::new()
                },
                history_index: 0,
                command_mode: false,
                index_stats: None,
                last_index_stats_refresh: None,
                index_stats_error: None,
                preview_cache: None,
                indexing_message: None,
                indexing_progress: None,
                indexing_active: false,
                indexing_started_at: None,
                last_indexing_update: None,
                search_in_progress: false,
            },
            list_state: ListState::default(),
            last_search_time: Instant::now(),
            search_pending: false,
            progress_tx,
            progress_rx,
            current_generation: 0,
            active_search: None,
        };
        app.list_state.select(Some(0));
        app
    }

    pub async fn run(mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Run initial search if query provided
        if !self.state.query.is_empty() {
            self.start_search(&mut terminal)?;
            self.pump_progress_events();
        }

        // Main event loop
        let result = self.event_loop(&mut terminal).await;

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    async fn event_loop<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            self.pump_progress_events();
            terminal.draw(|f| self.draw(f))?;
            self.pump_progress_events();

            // Check if we need to trigger a pending search (debouncing)
            if self.search_pending
                && self.last_search_time.elapsed() >= Duration::from_millis(DEBOUNCE_MS)
            {
                self.search_pending = false;
                self.start_search(terminal)?;
                self.pump_progress_events();
            }

            // Poll for events with timeout to support debouncing
            if event::poll(Duration::from_millis(50))?
                && let Event::Key(key) = event::read()?
            {
                // Only process key press events, not release
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => {
                        return Ok(());
                    }
                    KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                        return Ok(());
                    }
                    KeyCode::Char('v') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                        // Ctrl+V: Cycle preview mode
                        self.cycle_preview_mode();
                    }
                    KeyCode::Char('f') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                        // Ctrl+F: Toggle snippet/full file
                        self.toggle_full_file_mode();
                    }
                    KeyCode::Char('d') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                        // Ctrl+D: Show chunk metadata
                        show_chunks(&mut self.state);
                    }
                    KeyCode::Char(' ') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                        // Ctrl+Space: Toggle multi-select
                        self.toggle_select();
                    }
                    KeyCode::Tab => {
                        self.cycle_mode();
                        self.trigger_search();
                    }
                    KeyCode::Up if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                        // Ctrl+Up: Navigate search history
                        self.history_previous();
                    }
                    KeyCode::Down if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                        // Ctrl+Down: Navigate search history
                        self.history_next();
                    }
                    KeyCode::Up => {
                        self.previous_result();
                    }
                    KeyCode::Down => {
                        self.next_result();
                    }
                    KeyCode::PageUp => {
                        self.scroll_up();
                    }
                    KeyCode::PageDown => {
                        self.scroll_down();
                    }
                    KeyCode::Enter => {
                        // In command mode, execute command; otherwise open selected file
                        if self.state.command_mode {
                            execute_command(&mut self.state)?;
                        } else {
                            self.open_selected()?;
                        }
                    }
                    KeyCode::Backspace => {
                        self.state.query.pop();
                        // Exit command mode if we backspace the /
                        if !self.state.query.starts_with('/') {
                            self.state.command_mode = false;
                        }
                        self.trigger_search();
                    }
                    KeyCode::Char(c) => {
                        // All plain characters go to search (including space, s, x, etc.)
                        self.state.query.push(c);

                        // Enter command mode if / is the first character
                        if self.state.query == "/" {
                            self.state.command_mode = true;
                        }

                        self.trigger_search();
                    }
                    _ => {}
                }
                self.pump_progress_events();
            }
        }
    }

    fn draw(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Query input
                Constraint::Min(10),   // Results + Preview
                Constraint::Length(3), // Status bar
            ])
            .split(f.size());

        // Query input box
        draw_query_input(f, chunks[0], &self.state);

        // Split results and preview
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(chunks[1]);

        // Results list
        draw_results_list(f, main_chunks[0], &self.state, &mut self.list_state);

        // Preview pane
        draw_preview(f, main_chunks[1], &self.state);

        // Status bar
        self.refresh_index_stats(false);
        draw_status_bar(f, chunks[2], &self.state);
    }

    fn save_config(&self) {
        let config = TuiConfig {
            search_mode: self.state.mode.clone(),
            preview_mode: self.state.preview_mode.clone(),
            full_file_mode: self.state.full_file_mode,
        };
        let _ = config.save(); // Silently ignore errors
    }

    fn cycle_mode(&mut self) {
        self.state.mode = match self.state.mode {
            SearchMode::Semantic => SearchMode::Regex,
            SearchMode::Regex => SearchMode::Hybrid,
            SearchMode::Hybrid => SearchMode::Semantic,
            SearchMode::Lexical => SearchMode::Semantic, // Skip lexical for now
        };
        self.state.status_message = format!("Switched to {:?} mode", self.state.mode);
        self.save_config();
    }

    fn cycle_preview_mode(&mut self) {
        self.state.preview_mode = match self.state.preview_mode {
            PreviewMode::Heatmap => PreviewMode::Syntax,
            PreviewMode::Syntax => PreviewMode::Chunks,
            PreviewMode::Chunks => PreviewMode::Heatmap,
        };
        self.update_preview();
        self.state.status_message = format!("Preview: {:?}", self.state.preview_mode);
        self.save_config();
    }

    fn toggle_full_file_mode(&mut self) {
        self.state.full_file_mode = !self.state.full_file_mode;
        self.state.scroll_offset = 0; // Reset scroll when toggling
        self.update_preview();
        let mode_text = if self.state.full_file_mode {
            "Full File"
        } else {
            "Snippet"
        };
        self.state.status_message = format!("View: {}", mode_text);
        self.save_config();
    }

    fn scroll_up(&mut self) {
        if self.state.full_file_mode && self.state.scroll_offset > 0 {
            self.state.scroll_offset = self.state.scroll_offset.saturating_sub(10);
            self.update_preview();
        }
    }

    fn scroll_down(&mut self) {
        if self.state.full_file_mode {
            self.state.scroll_offset += 10;
            self.update_preview();
        }
    }

    fn toggle_select(&mut self) {
        if let Some(result) = self.state.results.get(self.state.selected_idx) {
            let file = result.file.clone();
            if self.state.selected_files.contains(&file) {
                self.state.selected_files.remove(&file);
                self.state.status_message = format!("Deselected {}", file.display());
            } else {
                self.state.selected_files.insert(file.clone());
                self.state.status_message = format!(
                    "Selected {} ({} total)",
                    file.display(),
                    self.state.selected_files.len()
                );
            }
        }
    }

    fn history_previous(&mut self) {
        if self.state.search_history.is_empty() {
            return;
        }
        if self.state.history_index > 0 {
            self.state.history_index -= 1;
            self.state.query = self.state.search_history[self.state.history_index].clone();
            self.trigger_search();
        }
    }

    fn history_next(&mut self) {
        if self.state.history_index < self.state.search_history.len().saturating_sub(1) {
            self.state.history_index += 1;
            self.state.query = self.state.search_history[self.state.history_index].clone();
            self.trigger_search();
        }
    }

    fn trigger_search(&mut self) {
        // Don't trigger search in command mode
        if self.state.command_mode {
            return;
        }
        self.search_pending = true;
        self.last_search_time = Instant::now();
    }

    fn pump_progress_events(&mut self) {
        while let Ok(event) = self.progress_rx.try_recv() {
            self.handle_progress_event(event);
        }

        if let Some(handle) = self.active_search.as_ref()
            && handle.is_finished()
        {
            self.active_search = None;
        }
    }

    fn handle_progress_event(&mut self, event: UiEvent) {
        let current_generation = self.current_generation;
        match event {
            UiEvent::Indexing {
                generation,
                message,
                progress,
            } => {
                if generation != current_generation {
                    return;
                }
                self.state.indexing_active = true;
                self.state.indexing_message = Some(message);
                self.state.indexing_progress = progress;
                let now = Instant::now();
                if self.state.indexing_started_at.is_none() {
                    self.state.indexing_started_at = Some(now);
                }
                self.state.last_indexing_update = Some(now);
            }
            UiEvent::IndexingDone { generation } => {
                if generation != current_generation {
                    return;
                }
                self.state.indexing_active = false;
                self.state.indexing_message = None;
                self.state.indexing_progress = None;
                self.state.indexing_started_at = None;
                self.state.last_indexing_update = None;
            }
            UiEvent::SearchProgress {
                generation,
                message,
            } => {
                if generation != current_generation || !self.state.search_in_progress {
                    return;
                }
                self.state.status_message = message;
            }
            UiEvent::SearchCompleted {
                generation,
                results,
                summary,
                query,
            } => {
                if generation != current_generation {
                    return;
                }
                self.search_pending = false;
                self.state.search_in_progress = false;
                self.state.indexing_active = false;
                self.state.indexing_message = None;
                self.state.indexing_progress = None;
                self.state.indexing_started_at = None;
                self.state.last_indexing_update = None;
                self.state.selected_files.clear();
                self.state.results = results;
                self.state.selected_idx = 0;
                self.state.scroll_offset = 0;
                if self.state.results.is_empty() {
                    self.list_state.select(None);
                } else {
                    self.list_state.select(Some(0));
                }
                self.state.preview_cache = None;
                self.update_preview();
                self.state.status_message = summary;

                if self.state.search_history.last() != Some(&query) {
                    self.state.search_history.push(query);
                    if self.state.search_history.len() > 20 {
                        self.state.search_history.remove(0);
                    }
                }
                if !self.state.search_history.is_empty() {
                    self.state.history_index = self.state.search_history.len() - 1;
                }
            }
            UiEvent::SearchFailed { generation, error } => {
                if generation != current_generation {
                    return;
                }
                self.search_pending = false;
                self.state.search_in_progress = false;
                self.state.indexing_active = false;
                self.state.indexing_message = None;
                self.state.indexing_progress = None;
                self.state.indexing_started_at = None;
                self.state.last_indexing_update = None;
                self.state.status_message = format!("Search error: {}", error);
            }
        }
    }

    fn refresh_index_stats(&mut self, force: bool) {
        const REFRESH_INTERVAL: Duration = Duration::from_secs(5);
        let now = Instant::now();
        let should_refresh = force
            || self
                .state
                .last_index_stats_refresh
                .map(|last| now.duration_since(last) >= REFRESH_INTERVAL)
                .unwrap_or(true);

        if !should_refresh {
            return;
        }

        match get_index_stats(&self.state.search_path) {
            Ok(stats) => {
                self.state.index_stats = Some(stats);
                self.state.index_stats_error = None;
            }
            Err(err) => {
                self.state.index_stats = None;
                self.state.index_stats_error = Some(err.to_string());
            }
        }

        self.state.last_index_stats_refresh = Some(now);
    }

    fn start_search<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        if self.state.query.trim().is_empty() {
            self.state.results.clear();
            self.state.preview_content.clear();
            self.state.preview_lines.clear();
            self.state.status_message = "Type to search...".to_string();
            self.state.preview_cache = None;
            self.state.search_in_progress = false;
            self.state.indexing_active = false;
            self.state.indexing_message = None;
            self.state.indexing_progress = None;
            self.state.indexing_started_at = None;
            self.state.last_indexing_update = None;
            self.list_state.select(None);
            return Ok(());
        }

        // Cancel any in-flight search task and advance the generation counter.
        if let Some(handle) = self.active_search.take() {
            handle.abort();
        }
        self.current_generation = self.current_generation.wrapping_add(1);
        let generation = self.current_generation;

        self.state.search_in_progress = true;
        self.state.indexing_active = false;
        self.state.indexing_message = None;
        self.state.indexing_progress = None;
        self.state.indexing_started_at = None;
        self.state.last_indexing_update = None;

        let mut status_message = "Searching...".to_string();
        if !matches!(self.state.mode, SearchMode::Regex)
            && get_index_stats(&self.state.search_path).is_err()
        {
            self.state.indexing_active = true;
            self.state.indexing_message =
                Some("Indexing repository for semantic search...".to_string());
            self.state.indexing_started_at = Some(Instant::now());
            status_message = "Preparing index...".to_string();
        }
        self.state.status_message = status_message;

        terminal.draw(|f| self.draw(f))?;

        let threshold = match self.state.mode {
            SearchMode::Semantic => Some(0.6),
            SearchMode::Hybrid => None,
            SearchMode::Regex => None,
            SearchMode::Lexical => None,
        };

        // Use the centralized pattern builder from ck-core
        // Note: .ckignore handling is now done by WalkBuilder hierarchically
        let exclude_patterns = ck_core::build_exclude_patterns(
            &[],  // No additional excludes in TUI
            true, // Use defaults
        );

        let options = SearchOptions {
            mode: self.state.mode.clone(),
            query: self.state.query.clone(),
            path: self.state.search_path.clone(),
            top_k: Some(50),
            threshold,
            case_insensitive: false,
            whole_word: false,
            fixed_string: false,
            line_numbers: true,
            context_lines: 0,
            before_context_lines: 0,
            after_context_lines: 0,
            recursive: true,
            json_output: false,
            jsonl_output: false,
            no_snippet: false,
            reindex: false,
            show_scores: true,
            show_filenames: true,
            files_with_matches: false,
            files_without_matches: false,
            exclude_patterns,
            include_patterns: Vec::new(),
            respect_gitignore: true,
            use_ckignore: true,
            full_section: false,
            rerank: false,
            rerank_model: None,
            embedding_model: None,
        };

        let progress_tx = self.progress_tx.clone();
        let started_at = Instant::now();

        let handle = tokio::spawn(async move {
            let query_for_history = options.query.clone();
            let search_progress_sender = progress_tx.clone();
            let detailed_sender = progress_tx.clone();
            let completion_sender = progress_tx.clone();

            let search_progress_callback: ck_engine::SearchProgressCallback =
                Box::new(move |message: &str| {
                    let _ = search_progress_sender.send(UiEvent::SearchProgress {
                        generation,
                        message: message.to_string(),
                    });
                });

            let throttle = Arc::new(Mutex::new(Instant::now()));
            let detailed_sender_clone = detailed_sender.clone();
            let detailed_throttle = throttle.clone();
            let detailed_indexing_progress_callback: ck_engine::DetailedIndexingProgressCallback =
                Box::new(move |progress: ck_index::EmbeddingProgress| {
                    let mut last = detailed_throttle.lock().unwrap();
                    if last.elapsed() >= Duration::from_millis(120)
                        || progress.chunk_index + 1 == progress.total_chunks
                    {
                        // Calculate overall progress across all files
                        let total_files = progress.total_files.max(1);
                        let current_file = progress.file_index;
                        let total_chunks_this_file = progress.total_chunks.max(1);
                        let current_chunk = progress.chunk_index + 1;

                        // Overall percentage = (completed files + progress in current file) / total files
                        let file_progress = current_chunk as f32 / total_chunks_this_file as f32;
                        let overall_pct = ((current_file as f32 + file_progress)
                            / total_files as f32)
                            .clamp(0.0, 1.0);

                        // Hierarchical format: filename • files count • chunks count
                        let message = format!(
                            "{} • {}/{} files • {}/{} chunks",
                            progress.file_name,
                            current_file + 1,
                            total_files,
                            current_chunk,
                            total_chunks_this_file,
                        );
                        let _ = detailed_sender_clone.send(UiEvent::Indexing {
                            generation,
                            message,
                            progress: Some(overall_pct),
                        });
                        *last = Instant::now();
                    }
                });

            let result = ck_engine::search_enhanced_with_indexing_progress(
                &options,
                Some(search_progress_callback),
                None, // Skip basic callback - only use detailed callback to avoid flashing
                Some(detailed_indexing_progress_callback),
            )
            .await;

            match result {
                Ok(search_results) => {
                    let elapsed_ms = started_at.elapsed().as_millis();
                    let summary = if search_results.matches.is_empty() {
                        format!("No results ({} ms)", elapsed_ms)
                    } else {
                        format!(
                            "Found {} results ({} ms)",
                            search_results.matches.len(),
                            elapsed_ms
                        )
                    };
                    let _ = completion_sender.send(UiEvent::SearchCompleted {
                        generation,
                        results: search_results.matches,
                        summary,
                        query: query_for_history,
                    });
                }
                Err(err) => {
                    let _ = completion_sender.send(UiEvent::SearchFailed {
                        generation,
                        error: err.to_string(),
                    });
                }
            }

            let _ = detailed_sender.send(UiEvent::IndexingDone { generation });
        });

        self.active_search = Some(handle);

        Ok(())
    }

    fn next_result(&mut self) {
        if self.state.results.is_empty() {
            return;
        }
        self.state.selected_idx = (self.state.selected_idx + 1) % self.state.results.len();
        self.list_state.select(Some(self.state.selected_idx));

        // In full file mode, reset scroll to show the matched chunk
        if self.state.full_file_mode
            && let Some(result) = self.state.results.get(self.state.selected_idx)
        {
            // Position scroll so matched line is near the top (but with some context above)
            self.state.scroll_offset = result.span.line_start.saturating_sub(6);
        }

        self.update_preview();
    }

    fn previous_result(&mut self) {
        if self.state.results.is_empty() {
            return;
        }
        if self.state.selected_idx == 0 {
            self.state.selected_idx = self.state.results.len() - 1;
        } else {
            self.state.selected_idx -= 1;
        }
        self.list_state.select(Some(self.state.selected_idx));

        // In full file mode, reset scroll to show the matched chunk
        if self.state.full_file_mode
            && let Some(result) = self.state.results.get(self.state.selected_idx)
        {
            // Position scroll so matched line is near the top (but with some context above)
            self.state.scroll_offset = result.span.line_start.saturating_sub(6);
        }

        self.update_preview();
    }

    fn update_preview(&mut self) {
        // Guard against empty results or invalid index
        if self.state.results.is_empty() {
            self.state.preview_content.clear();
            self.state.preview_lines.clear();
            return;
        }

        if let Some(result) = self.state.results.get(self.state.selected_idx) {
            // Load and cache file content with lines for the preview
            let cache_miss = self
                .state
                .preview_cache
                .as_ref()
                .map(|cache| cache.file != result.file)
                .unwrap_or(true);

            if cache_miss {
                match load_preview_lines(&result.file) {
                    Ok((lines, is_pdf, chunks)) => {
                        self.state.preview_cache = Some(PreviewCache {
                            file: result.file.clone(),
                            lines,
                            is_pdf,
                            chunks,
                        });
                    }
                    Err(err) => {
                        self.state.preview_content = format!(
                            "File: {}\nScore: {:.3}\n\n{}",
                            result.file.display(),
                            result.score,
                            err
                        );
                        self.state.preview_lines.clear();
                        return;
                    }
                }
            }

            let (lines, is_pdf, chunk_spans) = {
                if let Some(cache) = self.state.preview_cache.as_ref() {
                    (cache.lines.clone(), cache.is_pdf, cache.chunks.clone())
                } else {
                    self.state.preview_content = format!(
                        "File: {}\nScore: {:.3}\n\n(No preview available)",
                        result.file.display(),
                        result.score
                    );
                    self.state.preview_lines.clear();
                    return;
                }
            };
            let lines_ref = &lines;

            // Ensure we don't have an empty file or invalid line range
            if lines_ref.is_empty() {
                self.state.preview_content = format!(
                    "File: {}\nScore: {:.3}\n\n(Empty file)",
                    result.file.display(),
                    result.score
                );
                self.state.preview_lines.clear();
                return;
            }

            // Calculate context range based on mode
            let start_line = result
                .span
                .line_start
                .saturating_sub(1)
                .min(lines_ref.len().saturating_sub(1)); // 0-indexed
            let mut context_start = if self.state.full_file_mode {
                self.state
                    .scroll_offset
                    .min(lines_ref.len().saturating_sub(1))
            } else {
                start_line.saturating_sub(5)
            };
            let mut context_end = if self.state.full_file_mode {
                (context_start + 40).min(lines_ref.len())
            } else {
                (start_line + 10).min(lines_ref.len())
            };

            let chunk_meta = chunk_spans
                .iter()
                .filter(|meta| {
                    let span = &meta.span;
                    let line = result.span.line_start;
                    line >= span.line_start && line <= span.line_end
                })
                .min_by_key(|meta| meta.span.line_end.saturating_sub(meta.span.line_start))
                .cloned();

            // In Chunks mode + snippet mode, show the full chunk instead of ±5 lines
            if self.state.preview_mode == PreviewMode::Chunks
                && !self.state.full_file_mode
                && let Some(meta) = chunk_meta.as_ref()
            {
                context_start = meta
                    .span
                    .line_start
                    .saturating_sub(1)
                    .min(lines_ref.len().saturating_sub(1));
                context_end = meta.span.line_end.min(lines_ref.len());
            }

            if context_end <= context_start {
                context_end = (context_start + 1).min(lines_ref.len());
            }

            // Validate range
            if context_start >= context_end || context_end > lines_ref.len() {
                self.state.preview_content = format!(
                    "File: {}\nScore: {:.3}\n\n(Invalid line range)",
                    result.file.display(),
                    result.score
                );
                self.state.preview_lines.clear();
                return;
            }

            // Render based on preview mode (clone data to avoid borrow issues)
            let file_path = result.file.clone();
            let score = result.score;
            let match_line = result.span.line_start;
            let query = self.state.query.clone();

            self.state.preview_lines = match self.state.preview_mode {
                PreviewMode::Heatmap => render_heatmap_preview(
                    lines_ref,
                    context_start,
                    context_end,
                    &file_path,
                    score,
                    match_line,
                    &query,
                ),
                PreviewMode::Syntax => render_syntax_preview(
                    lines_ref,
                    context_start,
                    context_end,
                    &file_path,
                    score,
                    match_line,
                ),
                PreviewMode::Chunks => render_chunks_preview(
                    lines_ref,
                    context_start,
                    context_end,
                    &file_path,
                    score,
                    match_line,
                    chunk_meta.as_ref(),
                    is_pdf,
                    &chunk_spans,
                    self.state.full_file_mode,
                    self.state.preview_mode == PreviewMode::Chunks,
                ),
            };
            self.state.preview_content.clear();
        } else {
            self.state.preview_content.clear();
            self.state.preview_lines.clear();
        }
    }

    fn open_selected(&self) -> Result<()> {
        // Collect files to open (selected files or current result)
        let files_to_open: Vec<(PathBuf, usize)> = if self.state.selected_files.is_empty() {
            // No files selected, open current result
            if let Some(result) = self.state.results.get(self.state.selected_idx) {
                vec![(result.file.clone(), result.span.line_start)]
            } else {
                return Ok(());
            }
        } else {
            // Open all selected files at their first match line
            self.state
                .selected_files
                .iter()
                .filter_map(|file| {
                    self.state
                        .results
                        .iter()
                        .find(|r| &r.file == file)
                        .map(|r| (file.clone(), r.span.line_start))
                })
                .collect()
        };

        if files_to_open.is_empty() {
            return Ok(());
        }

        let editor = std::env::var("EDITOR")
            .or_else(|_| std::env::var("VISUAL"))
            .unwrap_or_else(|_| "vim".to_string());
        let editor_parts = split(&editor).unwrap_or_else(|| vec![editor.clone()]);
        let (command_name, command_args) = match editor_parts.split_first() {
            Some((command, args)) => (command.to_string(), args.to_vec()),
            None => (editor.clone(), Vec::new()),
        };

        // Need to restore terminal before opening editor
        disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

        let mut command = std::process::Command::new(&command_name);
        command.args(&command_args);

        let editor_basename = Path::new(&command_name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&command_name);

        // Open files based on editor type
        let status = if editor_basename.contains("cursor") || editor_basename.contains("code") {
            // Cursor/VS Code: can open multiple files with -g
            for (file, line) in &files_to_open {
                command
                    .arg("-g")
                    .arg(format!("{}:{}", file.display(), line));
            }
            command.status()?
        } else if editor_basename.contains("subl") {
            // Sublime: can open multiple files
            for (file, line) in &files_to_open {
                command.arg(format!("{}:{}", file.display(), line));
            }
            command.status()?
        } else if editor_basename.contains("emacs") {
            // Emacs: open first file only (multi-file is complex)
            let (file, line) = &files_to_open[0];
            command
                .arg(format!("+{}", line))
                .arg(file.display().to_string())
                .status()?
        } else if editor_basename.contains("nano") {
            // Nano: open first file only
            let (file, line) = &files_to_open[0];
            command
                .arg(format!("+{}", line))
                .arg(file.display().to_string())
                .status()?
        } else {
            // Vim/Neovim: can open multiple files with -p (tabs)
            for (file, line) in &files_to_open {
                command
                    .arg(format!("+{}", line))
                    .arg(file.display().to_string());
            }
            if files_to_open.len() > 1 {
                command.arg("-p"); // Open in tabs
            }
            command.status()?
        };

        if !status.success() {
            eprintln!("Editor exited with error");
        }

        // Don't re-enable raw mode - just exit
        std::process::exit(0);
    }
}
