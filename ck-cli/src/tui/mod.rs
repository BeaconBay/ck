use anyhow::Result;
use ck_core::{
    SearchMode, SearchOptions, SearchResult, get_default_exclude_patterns, pdf,
    read_ckignore_patterns,
};
use ck_index::{IndexStats, get_index_stats, load_index_entry};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use serde::{Deserialize, Serialize};
use shlex::split;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use tokio::sync::mpsc::{
    UnboundedReceiver, UnboundedSender, error::TryRecvError, unbounded_channel,
};
use tokio::task::JoinHandle;

const DEBOUNCE_MS: u64 = 300;

// Color palette - using RGB for consistency across terminals
const COLOR_CYAN: Color = Color::Rgb(80, 200, 200); // Cyan - headers, highlights
const COLOR_YELLOW: Color = Color::Rgb(255, 220, 0); // Yellow - matched lines, commands
const COLOR_WHITE: Color = Color::Rgb(220, 220, 220); // White - primary text
const COLOR_DARK_GRAY: Color = Color::Rgb(100, 100, 100); // Dark gray - subtle text
const COLOR_GRAY: Color = Color::Rgb(150, 150, 150); // Gray - secondary text
const COLOR_GREEN: Color = Color::Rgb(80, 200, 120); // Green - success, chunk boundaries
const COLOR_MAGENTA: Color = Color::Rgb(200, 80, 200); // Magenta - special markers
const COLOR_BLACK: Color = Color::Rgb(0, 0, 0); // Black - backgrounds
const SPINNER_FRAMES: [char; 4] = ['|', '/', '-', '\\'];

#[derive(Debug)]
enum UiEvent {
    Indexing {
        generation: u64,
        message: String,
        progress: Option<f32>,
    },
    IndexingDone {
        generation: u64,
    },
    SearchProgress {
        generation: u64,
        message: String,
    },
    SearchCompleted {
        generation: u64,
        results: Vec<SearchResult>,
        summary: String,
        query: String,
    },
    SearchFailed {
        generation: u64,
        error: String,
    },
}

pub struct TuiApp {
    state: TuiState,
    list_state: ListState,
    last_search_time: Instant,
    search_pending: bool,
    progress_tx: UnboundedSender<UiEvent>,
    progress_rx: UnboundedReceiver<UiEvent>,
    current_generation: u64,
    active_search: Option<JoinHandle<()>>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
enum PreviewMode {
    Heatmap, // Semantic similarity coloring
    Syntax,  // Syntax highlighting
    Chunks,  // Show chunk boundaries
}

#[derive(Serialize, Deserialize)]
struct TuiConfig {
    #[serde(with = "search_mode_serde")]
    search_mode: SearchMode,
    preview_mode: PreviewMode,
    full_file_mode: bool,
}

mod search_mode_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(mode: &SearchMode, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = match mode {
            SearchMode::Semantic => "semantic",
            SearchMode::Regex => "regex",
            SearchMode::Hybrid => "hybrid",
            SearchMode::Lexical => "lexical",
        };
        serializer.serialize_str(s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SearchMode, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "semantic" => SearchMode::Semantic,
            "regex" => SearchMode::Regex,
            "hybrid" => SearchMode::Hybrid,
            "lexical" => SearchMode::Lexical,
            _ => SearchMode::Semantic, // Default fallback
        })
    }
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            search_mode: SearchMode::Semantic,
            preview_mode: PreviewMode::Heatmap,
            full_file_mode: true,
        }
    }
}

impl TuiConfig {
    fn load() -> Self {
        let config_path = Self::config_path();
        if let Ok(contents) = std::fs::read_to_string(&config_path) {
            serde_json::from_str(&contents).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    fn save(&self) -> Result<()> {
        let config_path = Self::config_path();
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(&config_path, contents)?;
        Ok(())
    }

    fn config_path() -> PathBuf {
        if let Some(config_dir) = dirs::config_dir() {
            config_dir.join("ck").join("tui.json")
        } else {
            PathBuf::from(".ck_tui.json")
        }
    }
}

struct TuiState {
    query: String,
    mode: SearchMode,
    results: Vec<SearchResult>,
    selected_idx: usize,
    preview_content: String,
    preview_lines: Vec<Line<'static>>, // Colored preview
    preview_mode: PreviewMode,
    full_file_mode: bool, // false = snippet (±5 lines), true = full file
    scroll_offset: usize, // For scrolling in full file mode
    status_message: String,
    search_path: PathBuf,
    selected_files: HashSet<PathBuf>, // For multi-select
    search_history: Vec<String>,      // Search history
    history_index: usize,             // Current position in history
    command_mode: bool,               // true when query starts with /
    index_stats: Option<IndexStats>,
    last_index_stats_refresh: Option<Instant>,
    index_stats_error: Option<String>,
    preview_cache: Option<PreviewCache>,
    indexing_message: Option<String>,
    indexing_progress: Option<f32>,
    indexing_active: bool,
    indexing_started_at: Option<Instant>,
    last_indexing_update: Option<Instant>,
    search_in_progress: bool,
}

struct PreviewCache {
    file: PathBuf,
    lines: Vec<String>,
    is_pdf: bool,
    chunks: Vec<IndexedChunkMeta>,
}

#[derive(Clone)]
struct IndexedChunkMeta {
    span: ck_core::Span,
    chunk_type: Option<String>,
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
                selected_files: HashSet::new(),
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
            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    // Only process key press events, not release
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            return Ok(());
                        }
                        KeyCode::Char('c')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            return Ok(());
                        }
                        KeyCode::Char('v')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            // Ctrl+V: Cycle preview mode
                            self.cycle_preview_mode();
                        }
                        KeyCode::Char('f')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            // Ctrl+F: Toggle snippet/full file
                            self.toggle_full_file_mode();
                        }
                        KeyCode::Char('d')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            // Ctrl+D: Show chunk metadata
                            self.show_chunks();
                        }
                        KeyCode::Char(' ')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
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
                                self.execute_command()?;
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
        self.draw_query_input(f, chunks[0]);

        // Split results and preview
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(chunks[1]);

        // Results list
        self.draw_results_list(f, main_chunks[0]);

        // Preview pane
        self.draw_preview(f, main_chunks[1]);

        // Status bar
        self.draw_status_bar(f, chunks[2]);
    }

    fn draw_query_input(&self, f: &mut Frame, area: Rect) {
        let (title, style) = if self.state.command_mode {
            // In command mode
            (
                "Command (Enter to execute, /help for help)".to_string(),
                Style::default().fg(COLOR_CYAN).add_modifier(Modifier::BOLD),
            )
        } else {
            // In search mode
            let mode_indicator = match self.state.mode {
                SearchMode::Semantic => "[SEM]",
                SearchMode::Regex => "[REG]",
                SearchMode::Hybrid => "[HYB]",
                SearchMode::Lexical => "[LEX]",
            };
            (
                format!(
                    "Search {} (Tab to cycle, /help for commands)",
                    mode_indicator
                ),
                Style::default().fg(COLOR_YELLOW),
            )
        };

        let input = Paragraph::new(self.state.query.as_str())
            .style(style)
            .block(Block::default().borders(Borders::ALL).title(title));
        f.render_widget(input, area);
    }

    fn draw_results_list(&mut self, f: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .state
            .results
            .iter()
            .enumerate()
            .map(|(idx, result)| {
                let score_color = score_to_color(result.score);
                let is_selected = self.state.selected_files.contains(&result.file);
                let prefix = if is_selected { "✓ " } else { "  " };
                let content = format!(
                    "{}[{:.3}] {}:{}",
                    prefix,
                    result.score,
                    result.file.display(),
                    result.span.line_start
                );
                let style = if idx == self.state.selected_idx {
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

        let title = format!(
            "Results ({}/{})",
            self.state.results.len(),
            self.state.results.len()
        );
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        f.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn draw_preview(&self, f: &mut Frame, area: Rect) {
        // Determine title based on preview mode and context mode
        let view_mode = if self.state.full_file_mode {
            "Full File"
        } else {
            "Snippet"
        };
        let title = match self.state.preview_mode {
            PreviewMode::Heatmap => format!(
                "{}: Heatmap (^V: view | ^F: toggle | PgUp/Dn: scroll)",
                view_mode
            ),
            PreviewMode::Syntax => format!(
                "{}: Syntax (^V: view | ^F: toggle | PgUp/Dn: scroll)",
                view_mode
            ),
            PreviewMode::Chunks => format!(
                "{}: Chunks (^V: view | ^F: toggle | PgUp/Dn: scroll)",
                view_mode
            ),
        };

        let preview = if !self.state.preview_lines.is_empty() {
            Paragraph::new(self.state.preview_lines.clone())
                .block(Block::default().borders(Borders::ALL).title(title.clone()))
        } else {
            // Fallback to plain text
            let preview_text = if self.state.preview_content.is_empty() {
                "No preview available"
            } else {
                &self.state.preview_content
            };
            Paragraph::new(preview_text)
                .style(Style::default().fg(COLOR_WHITE))
                .block(Block::default().borders(Borders::ALL).title(title))
        };

        f.render_widget(preview, area);
    }

    fn draw_status_bar(&mut self, f: &mut Frame, area: Rect) {
        let help_text = " ↑↓: Nav | Tab: Mode | ^V: View | ^Space: Select | Enter: Open | ^↑↓: History | Esc/q: Quit ";

        // Refresh cached index stats on a slow cadence
        self.refresh_index_stats(false);

        let mut status_spans = vec![Span::styled(
            self.state.status_message.clone(),
            Style::default().fg(COLOR_CYAN),
        )];

        if self.state.indexing_active {
            let spinner_idx = self
                .state
                .indexing_started_at
                .map(|start| ((start.elapsed().as_millis() / 120) as usize) % SPINNER_FRAMES.len())
                .unwrap_or(0);
            let spinner = SPINNER_FRAMES[spinner_idx];

            status_spans.push(Span::raw(" | "));
            status_spans.push(Span::styled(
                format!("{} ", spinner),
                Style::default().fg(COLOR_YELLOW),
            ));

            // Overall percentage in fixed width, appears before the detailed message
            if let Some(progress) = self.state.indexing_progress {
                let pct = (progress * 100.0).clamp(0.0, 100.0).round() as i32;
                status_spans.push(Span::styled(
                    format!("[{:>3}%] ", pct),
                    Style::default()
                        .fg(COLOR_GREEN)
                        .add_modifier(Modifier::BOLD),
                ));
            }

            // Parse the detailed message to colorize parts
            if let Some(message) = self.state.indexing_message.as_ref() {
                // Split on bullet points to colorize differently
                let parts: Vec<&str> = message.split(" • ").collect();
                for (i, part) in parts.iter().enumerate() {
                    if i > 0 {
                        status_spans
                            .push(Span::styled(" • ", Style::default().fg(COLOR_DARK_GRAY)));
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
        } else if let Some(message) = self.state.indexing_message.as_ref() {
            status_spans.push(Span::raw(" | "));
            status_spans.push(Span::styled(
                message.clone(),
                Style::default().fg(COLOR_GRAY),
            ));
        }

        if !self.state.selected_files.is_empty() {
            status_spans.push(Span::raw(" | "));
            status_spans.push(Span::styled(
                format!("{} selected", self.state.selected_files.len()),
                Style::default().fg(COLOR_MAGENTA),
            ));
        }

        let index_info = if let Some(stats) = self.state.index_stats.as_ref() {
            format!(
                "Index: {} files, {} chunks",
                stats.total_files, stats.total_chunks
            )
        } else if let Some(err) = self.state.index_stats_error.as_ref() {
            format!("Index error: {}", err)
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
        loop {
            match self.progress_rx.try_recv() {
                Ok(event) => self.handle_progress_event(event),
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }

        if let Some(handle) = self.active_search.as_ref() {
            if handle.is_finished() {
                self.active_search = None;
            }
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
        if !matches!(self.state.mode, SearchMode::Regex) {
            if get_index_stats(&self.state.search_path).is_err() {
                self.state.indexing_active = true;
                self.state.indexing_message =
                    Some("Indexing repository for semantic search...".to_string());
                self.state.indexing_started_at = Some(Instant::now());
                status_message = "Preparing index...".to_string();
            }
        }
        self.state.status_message = status_message;

        terminal.draw(|f| self.draw(f))?;

        let threshold = match self.state.mode {
            SearchMode::Semantic => Some(0.6),
            SearchMode::Hybrid => None,
            SearchMode::Regex => None,
            SearchMode::Lexical => None,
        };

        let mut exclude_patterns = get_default_exclude_patterns();
        if let Ok(extra) = read_ckignore_patterns(&self.state.search_path) {
            exclude_patterns.extend(extra);
        }

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
            respect_gitignore: true,
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
        if self.state.full_file_mode {
            if let Some(result) = self.state.results.get(self.state.selected_idx) {
                // Position scroll so matched line is near the top (but with some context above)
                self.state.scroll_offset = result.span.line_start.saturating_sub(6);
            }
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
        if self.state.full_file_mode {
            if let Some(result) = self.state.results.get(self.state.selected_idx) {
                // Position scroll so matched line is near the top (but with some context above)
                self.state.scroll_offset = result.span.line_start.saturating_sub(6);
            }
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
            if self.state.preview_mode == PreviewMode::Chunks && !self.state.full_file_mode {
                if let Some(meta) = chunk_meta.as_ref() {
                    context_start = meta
                        .span
                        .line_start
                        .saturating_sub(1)
                        .min(lines_ref.len().saturating_sub(1));
                    context_end = meta.span.line_end.min(lines_ref.len());
                }
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

            match self.state.preview_mode {
                PreviewMode::Heatmap => self.render_heatmap_preview(
                    lines_ref,
                    context_start,
                    context_end,
                    &file_path,
                    score,
                    match_line,
                    &query,
                ),
                PreviewMode::Syntax => self.render_syntax_preview(
                    lines_ref,
                    context_start,
                    context_end,
                    &file_path,
                    score,
                    match_line,
                ),
                PreviewMode::Chunks => self.render_chunks_preview(
                    lines_ref,
                    context_start,
                    context_end,
                    &file_path,
                    score,
                    match_line,
                    chunk_meta.as_ref(),
                    is_pdf,
                    &chunk_spans,
                ),
            }
        } else {
            self.state.preview_content.clear();
            self.state.preview_lines.clear();
        }
    }

    fn render_heatmap_preview(
        &mut self,
        lines: &[String],
        context_start: usize,
        context_end: usize,
        file_path: &PathBuf,
        score: f32,
        match_line: usize,
        query: &str,
    ) {
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
            let in_chunk_range =
                line_num >= match_line.saturating_sub(5) && line_num <= match_line + 5;

            let mut line_spans = vec![Span::styled(
                format!("{:4} | ", line_num),
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

        self.state.preview_lines = colored_lines;
        self.state.preview_content.clear();
    }

    fn render_syntax_preview(
        &mut self,
        lines: &[String],
        context_start: usize,
        context_end: usize,
        file_path: &PathBuf,
        score: f32,
        match_line: usize,
    ) {
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
                            format!("{:4} | ", line_num),
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

                self.state.preview_lines = colored_lines;
                self.state.preview_content.clear();
                return;
            }
        };

        // Apply syntax highlighting
        for (idx, line) in lines[context_start..context_end].iter().enumerate() {
            let line_num = context_start + idx + 1;
            let is_match_line = line_num == match_line;
            let in_chunk_range =
                line_num >= match_line.saturating_sub(5) && line_num <= match_line + 5;

            let mut line_spans = vec![Span::styled(
                format!("{:4} | ", line_num),
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

        self.state.preview_lines = colored_lines;
        self.state.preview_content.clear();
    }

    fn render_chunks_preview(
        &mut self,
        lines: &[String],
        context_start: usize,
        context_end: usize,
        file_path: &PathBuf,
        score: f32,
        match_line: usize,
        chunk_meta: Option<&IndexedChunkMeta>,
        is_pdf: bool,
        all_chunks: &[IndexedChunkMeta],
    ) {
        let mut colored_lines = Vec::new();

        let header = if let Some(meta) = chunk_meta {
            let span = &meta.span;
            let total_lines = span
                .line_end
                .saturating_sub(span.line_start)
                .saturating_add(1);
            format!(
                "File: {} | Score: {:.3} | {} L{}-{} ({} lines)\n",
                file_path.display(),
                score,
                meta.chunk_type.as_deref().unwrap_or("chunk"),
                span.line_start,
                span.line_end,
                total_lines
            )
        } else if is_pdf {
            format!(
                "File: {} | Score: {:.3} | Approximate chunk (PDF)\n",
                file_path.display(),
                score
            )
        } else {
            format!(
                "File: {} | Score: {:.3} | Chunk boundaries unavailable\n",
                file_path.display(),
                score
            )
        };

        colored_lines.push(Line::from(vec![Span::styled(
            header,
            Style::default().fg(COLOR_CYAN),
        )]));

        let marker_width = compute_max_chunk_overlap(all_chunks).max(1);

        for (idx, line) in lines[context_start..context_end].iter().enumerate() {
            let line_num = context_start + idx + 1;
            let is_match_line = line_num == match_line;

            // Check if this line is in the matched chunk
            let in_matched_chunk = chunk_meta
                .map(|meta| {
                    let span = &meta.span;
                    line_num >= span.line_start && line_num <= span.line_end
                })
                .unwrap_or(false);

            // Find ALL chunks that contain this line (handles overlapping chunks)
            // In snippet mode, only show the matched chunk to reduce noise
            let mut chunks_at_line: Vec<&IndexedChunkMeta> = if self.state.full_file_mode {
                // Full file mode: show all overlapping chunks
                all_chunks
                    .iter()
                    .filter(|meta| {
                        let span = &meta.span;
                        line_num >= span.line_start && line_num <= span.line_end
                    })
                    .collect()
            } else {
                // Snippet mode: only show matched chunk to reduce visual noise
                chunk_meta
                    .iter()
                    .filter(|meta| {
                        let span = &meta.span;
                        line_num >= span.line_start && line_num <= span.line_end
                    })
                    .cloned()
                    .collect()
            };

            // Sort by span size (largest first) for nesting visualization
            chunks_at_line.sort_by_key(|span| {
                std::cmp::Reverse(span.span.line_end.saturating_sub(span.span.line_start))
            });

            // Build nested boundary markers for overlapping chunks
            let boundary_marker = if chunks_at_line.is_empty() {
                " ".repeat(marker_width)
            } else {
                let mut markers = String::new();
                for (depth, meta) in chunks_at_line.iter().enumerate() {
                    if depth >= marker_width {
                        break;
                    } // Respect calculated overlap width

                    // Choose marker based on position and depth
                    let span = &meta.span;
                    let marker_char = if span.line_start == span.line_end {
                        if depth == 0 { '━' } else { '─' } // Single line
                    } else if line_num == span.line_start {
                        if depth == 0 { '┏' } else { '╔' } // Top
                    } else if line_num == span.line_end {
                        if depth == 0 { '┗' } else { '╚' } // Bottom
                    } else {
                        if depth == 0 { '┃' } else { '║' } // Middle
                    };

                    markers.push(marker_char);
                }
                // Pad to full marker width so columns stay aligned
                while markers.len() < marker_width {
                    markers.push(' ');
                }
                markers
            };

            let marker_color = if in_matched_chunk {
                COLOR_MAGENTA
            } else if !chunks_at_line.is_empty() {
                COLOR_DARK_GRAY
            } else {
                COLOR_DARK_GRAY
            };

            let mut line_spans = vec![
                Span::styled(
                    boundary_marker,
                    if in_matched_chunk {
                        Style::default()
                            .fg(marker_color)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(marker_color)
                    },
                ),
                Span::styled(
                    format!("{:4} | ", line_num),
                    if is_match_line {
                        Style::default()
                            .fg(COLOR_YELLOW)
                            .add_modifier(Modifier::BOLD)
                    } else if in_matched_chunk {
                        Style::default().fg(COLOR_CYAN)
                    } else {
                        Style::default().fg(COLOR_GRAY)
                    },
                ),
            ];

            if !all_chunks.is_empty() {
                if let Some(meta) = chunk_meta {
                    if let Some(label) = meta.chunk_type.as_ref()
                        && line_num == meta.span.line_start
                    {
                        line_spans.push(Span::styled(
                            format!("[{label}] "),
                            Style::default().fg(COLOR_MAGENTA),
                        ));
                    }
                }

                // Show all lines, but dim non-matched chunks
                line_spans.push(Span::styled(
                    line.to_string(),
                    if in_matched_chunk {
                        Style::default().fg(COLOR_WHITE)
                    } else if !chunks_at_line.is_empty() {
                        Style::default().fg(COLOR_GRAY) // Other chunks in gray
                    } else {
                        Style::default().fg(COLOR_DARK_GRAY) // Non-chunk lines dimmer
                    },
                ));
            } else {
                // Fallback heuristics for files without chunk metadata
                let is_boundary = line.trim_start().starts_with("fn ")
                    || line.trim_start().starts_with("func ")
                    || line.trim_start().starts_with("def ")
                    || line.trim_start().starts_with("class ")
                    || line.trim_start().starts_with("impl ")
                    || line.trim_start().starts_with("struct ")
                    || line.trim_start().starts_with("enum ");

                if is_boundary {
                    line_spans[0] = Span::styled(
                        "┣━ ",
                        Style::default()
                            .fg(COLOR_MAGENTA)
                            .add_modifier(Modifier::BOLD),
                    );
                }

                line_spans.push(Span::styled(
                    line.to_string(),
                    if is_boundary {
                        Style::default()
                            .fg(COLOR_GREEN)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(COLOR_WHITE)
                    },
                ));
            }

            colored_lines.push(Line::from(line_spans));
        }

        if chunk_meta.is_none() && !all_chunks.is_empty() {
            colored_lines.push(Line::from(vec![Span::styled(
                "Chunk metadata available but no matching chunk found for this line.",
                Style::default().fg(COLOR_MAGENTA),
            )]));
        }

        self.state.preview_lines = colored_lines;
        self.state.preview_content.clear();
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

    fn execute_command(&mut self) -> Result<()> {
        let cmd = self.state.query.trim();

        match cmd {
            "/help" | "/h" | "/?" => {
                self.show_help();
            }
            "/clear" | "/c" => {
                self.state.results.clear();
                self.state.preview_content.clear();
                self.state.preview_lines.clear();
                self.state.query.clear();
                self.state.command_mode = false;
                self.state.status_message = "Cleared results".to_string();
            }
            "/history" => {
                self.show_history();
            }
            "/stats" => {
                self.show_stats();
            }
            _ => {
                self.state.status_message = format!(
                    "Unknown command: {}. Type /help for available commands",
                    cmd
                );
            }
        }

        Ok(())
    }

    fn show_help(&mut self) {
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
        self.state.preview_lines = help_text
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

        self.state.query.clear();
        self.state.command_mode = false;
        self.state.status_message = "Help - Press Esc to return to search".to_string();
    }

    fn show_chunks(&mut self) {
        // Get currently selected file
        if self.state.results.is_empty() {
            self.state.status_message = "No search results - run a search first".to_string();
            self.state.query.clear();
            self.state.command_mode = false;
            return;
        }

        let selected_file = self.state.results[self.state.selected_idx].file.clone();

        // Find repo root and load chunks
        let repo_root = find_repo_root(&selected_file);
        let all_chunks = if let Some(root) = repo_root {
            load_chunk_spans(&root, &selected_file).unwrap_or_default()
        } else {
            Vec::new()
        };

        if all_chunks.is_empty() {
            self.state.status_message = format!("No chunks found for {}", selected_file.display());
            self.state.query.clear();
            self.state.command_mode = false;
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
            let chunk_type = chunk
                .chunk_type
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or("unknown");

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
                        .map(|n| format!("#{}", n))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }

            chunks_text.push("".to_string());
        }

        chunks_text.push("Press Esc to close".to_string());

        // Convert to colored lines
        self.state.preview_lines = chunks_text
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

        self.state.query.clear();
        self.state.command_mode = false;
        self.state.scroll_offset = 0;
        self.state.status_message = format!(
            "Chunk metadata for {} - Press Esc to return",
            selected_file.display()
        );
    }

    fn show_history(&mut self) {
        if self.state.search_history.is_empty() {
            self.state.status_message = "No search history".to_string();
            self.state.query.clear();
            self.state.command_mode = false;
            return;
        }

        let history_text: Vec<String> = std::iter::once("━━━ SEARCH HISTORY ━━━".to_string())
            .chain(std::iter::once("".to_string()))
            .chain(
                self.state
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

        self.state.preview_lines = history_text
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

        self.state.query.clear();
        self.state.command_mode = false;
        self.state.status_message = "Search History".to_string();
    }

    fn show_stats(&mut self) {
        self.refresh_index_stats(true);

        let stats_text = if let Some(stats) = self.state.index_stats.as_ref() {
            vec![
                "━━━ INDEX STATISTICS ━━━".to_string(),
                "".to_string(),
                format!("  Path: {}", self.state.search_path.display()),
                format!("  Files: {}", stats.total_files),
                format!(
                    "  Chunks: {} ({} embedded)",
                    stats.total_chunks, stats.embedded_chunks
                ),
                format!("  Total size: {} bytes", stats.total_size_bytes),
                format!("  Index size: {} bytes", stats.index_size_bytes),
                "".to_string(),
            ]
        } else if let Some(err) = self.state.index_stats_error.as_ref() {
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

        self.state.preview_lines = stats_text
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

        self.state.query.clear();
        self.state.command_mode = false;
        self.state.status_message = "Index Statistics".to_string();
    }
}

fn load_preview_lines(path: &Path) -> Result<(Vec<String>, bool, Vec<IndexedChunkMeta>), String> {
    let resolved_path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let repo_root = find_repo_root(&resolved_path);
    let is_pdf = pdf::is_pdf_file(&resolved_path);

    let lines = if is_pdf {
        let root = repo_root.clone().ok_or_else(|| {
            "PDF preview unavailable (missing .ck index). Run `ck --index .` first.".to_string()
        })?;

        let cache_path = pdf::get_content_cache_path(&root, &resolved_path);
        let content = fs::read_to_string(&cache_path).map_err(|err| {
            format!(
                "PDF preview unavailable ({}). Run `ck --index .` to generate cache.",
                err
            )
        })?;

        content.lines().map(|line| line.to_string()).collect()
    } else {
        let content = fs::read_to_string(&resolved_path)
            .map_err(|err| format!("Could not read {}: {}", resolved_path.display(), err))?;
        content.lines().map(|line| line.to_string()).collect()
    };

    let chunk_spans = if let Some(root) = repo_root {
        load_chunk_spans(&root, &resolved_path).unwrap_or_default()
    } else {
        Vec::new()
    };

    Ok((lines, is_pdf, chunk_spans))
}

fn find_repo_root(path: &Path) -> Option<PathBuf> {
    let mut current = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };

    loop {
        if current.join(".ck").exists() {
            return Some(current.to_path_buf());
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => return None,
        }
    }
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
        .map_err(|err| format!("Failed to load chunk metadata: {}", err))?;
    Ok(entry
        .chunks
        .iter()
        .map(|chunk| IndexedChunkMeta {
            span: chunk.span.clone(),
            chunk_type: chunk.chunk_type.clone(),
        })
        .collect())
}

fn compute_max_chunk_overlap(chunks: &[IndexedChunkMeta]) -> usize {
    if chunks.is_empty() {
        return 0;
    }

    let mut events: Vec<(usize, i32)> = Vec::with_capacity(chunks.len() * 2);
    for chunk in chunks {
        events.push((chunk.span.line_start, 1));
        events.push((chunk.span.line_end.saturating_add(1), -1));
    }

    events.sort_by_key(|(line, _)| *line);

    let mut current = 0;
    let mut max_overlap = 0;
    for (_, delta) in events {
        current += delta;
        if current > max_overlap {
            max_overlap = current;
        }
    }

    max_overlap.max(0) as usize
}

fn syntax_set() -> &'static SyntaxSet {
    static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAX_SET.get_or_init(|| SyntaxSet::load_defaults_newlines())
}

fn theme_set() -> &'static ThemeSet {
    static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();
    THEME_SET.get_or_init(|| ThemeSet::load_defaults())
}

fn score_to_color(score: f32) -> Color {
    if score >= 0.875 {
        Color::Rgb(0, 255, 100) // Bright green
    } else if score >= 0.75 {
        Color::Rgb(0, 200, 80)
    } else if score >= 0.625 {
        Color::Rgb(0, 160, 70)
    } else if score >= 0.5 {
        Color::Rgb(100, 140, 60)
    } else {
        Color::Rgb(140, 140, 140) // Gray
    }
}

// Token-level similarity calculation for heatmap highlighting
fn split_into_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current_token = String::new();

    for ch in text.chars() {
        match ch {
            ' ' | '\t' | '\n' => {
                if !current_token.is_empty() {
                    tokens.push(current_token.clone());
                    current_token.clear();
                }
                tokens.push(ch.to_string());
            }
            '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';' | ':' | '.' | '!' | '?' => {
                if !current_token.is_empty() {
                    tokens.push(current_token.clone());
                    current_token.clear();
                }
                tokens.push(ch.to_string());
            }
            _ => {
                current_token.push(ch);
            }
        }
    }

    if !current_token.is_empty() {
        tokens.push(current_token);
    }

    tokens
}

fn calculate_token_similarity(token: &str, pattern: &str) -> f32 {
    // Skip whitespace and punctuation
    if token.trim().is_empty() || token.chars().all(|c| !c.is_alphanumeric()) {
        return 0.0;
    }

    let token_lower = token.to_lowercase();
    let pattern_lower = pattern.to_lowercase();

    // Exact match gets highest score
    if token_lower == pattern_lower {
        return 1.0;
    }

    // Check if token contains any pattern words or vice versa
    let pattern_words: Vec<&str> = pattern_lower.split_whitespace().collect();
    let mut max_score: f32 = 0.0;

    for pattern_word in &pattern_words {
        if pattern_word.len() < 3 {
            continue; // Skip short words
        }

        // Exact word match
        if token_lower == *pattern_word {
            max_score = max_score.max(0.9);
        }
        // Substring match
        else if token_lower.contains(pattern_word) {
            let ratio = pattern_word.len() as f32 / token_lower.len() as f32;
            max_score = max_score.max(0.6 * ratio);
        }
        // Pattern word contains token
        else if pattern_word.contains(&token_lower) && token_lower.len() >= 3 {
            let ratio = token_lower.len() as f32 / pattern_word.len() as f32;
            max_score = max_score.max(0.5 * ratio);
        }
        // Fuzzy similarity for related terms
        else {
            let similarity = calculate_fuzzy_similarity(&token_lower, pattern_word);
            max_score = max_score.max(similarity * 0.4);
        }
    }

    max_score
}

fn calculate_fuzzy_similarity(s1: &str, s2: &str) -> f32 {
    // Simple edit distance-based similarity
    if s1.is_empty() || s2.is_empty() || s1.len() < 3 || s2.len() < 3 {
        return 0.0;
    }

    let len1 = s1.len();
    let len2 = s2.len();
    let max_len = len1.max(len2);

    // Count common characters
    let s1_chars: HashSet<char> = s1.chars().collect();
    let s2_chars: HashSet<char> = s2.chars().collect();
    let common_chars = s1_chars.intersection(&s2_chars).count();

    // Similarity based on common characters
    common_chars as f32 / max_len as f32
}

fn apply_heatmap_color_to_token(token: &str, score: f32) -> Color {
    // Skip coloring whitespace and punctuation
    if token.trim().is_empty() || token.chars().all(|c| !c.is_alphanumeric()) {
        return Color::Reset;
    }

    // 8-step linear gradient: grey to green with bright final step
    match score {
        s if s >= 0.875 => Color::Rgb(0, 255, 100), // Extra bright green
        s if s >= 0.75 => Color::Rgb(0, 180, 80),
        s if s >= 0.625 => Color::Rgb(0, 160, 70),
        s if s >= 0.5 => Color::Rgb(0, 140, 60),
        s if s >= 0.375 => Color::Rgb(50, 120, 80),
        s if s >= 0.25 => Color::Rgb(100, 130, 100),
        s if s >= 0.125 => Color::Rgb(140, 140, 140),
        s if s > 0.0 => Color::Rgb(180, 180, 180),
        _ => Color::Reset,
    }
}

pub async fn run_tui(search_path: PathBuf, initial_query: Option<String>) -> Result<()> {
    let app = TuiApp::new(search_path, initial_query);
    app.run().await
}
