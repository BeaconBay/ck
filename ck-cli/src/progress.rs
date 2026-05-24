use console::{Term, style};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::time::{Duration, Instant};

pub struct StatusReporter {
    term: Term,
    multi_progress: MultiProgress,
    pub quiet: bool,
}

impl StatusReporter {
    pub fn new(quiet: bool) -> Self {
        Self {
            term: Term::stderr(),
            multi_progress: MultiProgress::new(),
            quiet,
        }
    }

    pub fn info(&self, msg: &str) {
        if !self.quiet {
            let _ = self.term.write_line(&format!(
                "{} {}",
                style("ℹ").cyan().bold(),
                style(msg).dim()
            ));
        }
    }

    pub fn success(&self, msg: &str) {
        if !self.quiet {
            let _ = self.term.write_line(&format!(
                "{} {}",
                style("✓").green().bold(),
                style(msg).green()
            ));
        }
    }

    pub fn warn(&self, msg: &str) {
        if !self.quiet {
            let _ = self.term.write_line(&format!(
                "{} {}",
                style("⚠").yellow().bold(),
                style(msg).yellow()
            ));
        }
    }

    #[allow(dead_code)]
    pub fn error(&self, msg: &str) {
        let _ = self
            .term
            .write_line(&format!("{} {}", style("✗").red().bold(), style(msg).red()));
    }

    #[allow(dead_code)]
    pub fn create_file_progress(&self, total: u64, operation: &str) -> Option<ProgressBar> {
        if self.quiet {
            return None;
        }

        let pb = self.multi_progress.add(ProgressBar::new(total));
        pb.set_style(
            ProgressStyle::default_bar()
                .template(&format!(
                    "{{spinner:.green}} {} {{wide_bar:.cyan/blue}} {{pos}}/{{len}} {{msg}}",
                    style(operation).bold()
                ))
                .unwrap_or_else(|_| ProgressStyle::default_bar())
                .progress_chars("█▉▊▋▌▍▎▏  ")
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        pb.enable_steady_tick(Duration::from_millis(120));
        Some(pb)
    }

    pub fn create_spinner(&self, msg: &str) -> Option<ProgressBar> {
        if self.quiet {
            return None;
        }

        let pb = self.multi_progress.add(ProgressBar::new_spinner());
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner())
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        pb.set_message(msg.to_string());
        pb.enable_steady_tick(Duration::from_millis(120));
        Some(pb)
    }

    #[allow(dead_code)]
    pub fn update_file_progress(&self, pb: &Option<ProgressBar>, file_name: &str) {
        if let Some(pb) = pb {
            pb.inc(1);
            pb.set_message(format!("{}", style(file_name).dim()));
        }
    }

    pub fn finish_progress(&self, pb: Option<ProgressBar>, success_msg: &str) {
        if let Some(pb) = pb {
            pb.finish_with_message(format!(
                "{} {}",
                style("✓").green().bold(),
                style(success_msg).green()
            ));
        }
    }

    #[allow(dead_code)]
    pub fn streaming_files(&self, files: &[std::path::PathBuf]) {
        if !self.quiet && !files.is_empty() {
            let max_display = 5;
            let display_files = if files.len() > max_display {
                &files[..max_display]
            } else {
                files
            };

            for (i, file) in display_files.iter().enumerate() {
                let file_name = file
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| file.to_string_lossy().to_string());

                let _ = self.term.write_line(&format!(
                    "  {} {}",
                    style(format!("{}.", i + 1)).dim(),
                    style(&file_name).cyan()
                ));

                // Small delay to create streaming effect
                std::thread::sleep(Duration::from_millis(50));
            }

            if files.len() > max_display {
                let remaining = files.len() - max_display;
                let _ = self.term.write_line(&format!(
                    "  {} {}",
                    style("...").dim(),
                    style(format!("and {remaining} more files")).dim()
                ));
            }
        }
    }

    pub fn section_header(&self, title: &str) {
        if !self.quiet {
            let _ = self.term.write_line("");
            let _ = self.term.write_line(&format!(
                "{} {}",
                style("▸").blue().bold(),
                style(title).bold()
            ));
        }
    }
}

// Helper macro for conditional progress reporting
#[macro_export]
macro_rules! status_info {
    ($reporter:expr, $($arg:tt)*) => {
        $reporter.info(&format!($($arg)*))
    };
}

#[macro_export]
macro_rules! status_success {
    ($reporter:expr, $($arg:tt)*) => {
        $reporter.success(&format!($($arg)*))
    };
}

#[macro_export]
macro_rules! status_warn {
    ($reporter:expr, $($arg:tt)*) => {
        $reporter.warn(&format!($($arg)*))
    };
}

#[allow(unused_attributes)]
#[macro_export]
/// Enhanced indexing progress tracker with multiple progress bars
#[allow(dead_code)]
pub struct EnhancedIndexingProgress {
    overall_progress: ProgressBar,
    current_file_progress: Option<ProgressBar>,
    start_time: Instant,
    files_completed: usize,
    total_files: usize,
    total_chunks: usize,
}

#[allow(dead_code)]
impl EnhancedIndexingProgress {
    pub fn new(status: &StatusReporter, total_files: usize) -> Self {
        let overall_pb = status
            .multi_progress
            .add(ProgressBar::new(total_files as u64));
        overall_pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} files {msg}")
                .unwrap()
                .progress_chars("█▉▊▋▌▍▎▏  "),
        );
        overall_pb.set_message("Starting indexing...");

        Self {
            overall_progress: overall_pb,
            current_file_progress: None,
            start_time: Instant::now(),
            files_completed: 0,
            total_files,
            total_chunks: 0,
        }
    }

    pub fn start_file(
        &mut self,
        status: &StatusReporter,
        file_name: &str,
        estimated_chunks: usize,
    ) {
        // Finish previous file progress bar if exists
        if let Some(old_pb) = self.current_file_progress.take() {
            old_pb.finish_and_clear();
        }

        // Create new progress bar for this file
        let file_pb = status.multi_progress.insert_after(
            &self.overall_progress,
            ProgressBar::new(estimated_chunks as u64),
        );
        file_pb.set_style(
            ProgressStyle::default_bar()
                .template("  {spinner:.yellow} [{bar:30.yellow/dim}] {pos}/{len} chunks {msg}")
                .unwrap()
                .progress_chars("●◐○  "),
        );
        file_pb.set_message(format!("📄 {file_name}"));

        self.current_file_progress = Some(file_pb);
    }

    pub fn update_chunk_progress(&mut self, chunk_number: usize, chunk_size: usize) {
        if let Some(ref pb) = self.current_file_progress {
            pb.set_position(chunk_number as u64);
            pb.set_message(format!("🔢 chunk {chunk_number} ({chunk_size} chars)"));
        }
    }

    pub fn complete_file(&mut self, file_name: &str, chunks_processed: usize) {
        self.files_completed += 1;
        self.total_chunks += chunks_processed;

        // Update overall progress
        self.overall_progress
            .set_position(self.files_completed as u64);
        let elapsed = self.start_time.elapsed();
        let files_per_sec = if elapsed.as_secs() > 0 {
            self.files_completed as f64 / elapsed.as_secs() as f64
        } else {
            0.0
        };

        self.overall_progress.set_message(format!(
            "{} ({:.1}/s, {} chunks total)",
            file_name.split('/').next_back().unwrap_or(file_name),
            files_per_sec,
            self.total_chunks
        ));

        // Complete current file progress bar
        if let Some(pb) = self.current_file_progress.take() {
            pb.finish_with_message(format!("✓ {chunks_processed} chunks"));
        }
    }

    pub fn finish(self, status: &StatusReporter) {
        let elapsed = self.start_time.elapsed();

        self.overall_progress.finish_with_message(format!(
            "✅ {} files, {} chunks in {:.1}s",
            self.files_completed,
            self.total_chunks,
            elapsed.as_secs_f64()
        ));

        // Show exciting completion stats
        let files_per_sec = if elapsed.as_secs() > 0 {
            self.files_completed as f64 / elapsed.as_secs() as f64
        } else {
            self.files_completed as f64
        };
        let chunks_per_sec = if elapsed.as_secs() > 0 {
            self.total_chunks as f64 / elapsed.as_secs() as f64
        } else {
            self.total_chunks as f64
        };

        status.success(&format!(
            "🚀 Indexed {} files ({} chunks) in {:.2}s - {:.1} files/sec, {:.1} chunks/sec",
            self.files_completed,
            self.total_chunks,
            elapsed.as_secs_f64(),
            files_per_sec,
            chunks_per_sec
        ));
    }
}

#[allow(unused_macros)]
macro_rules! status_error {
    ($reporter:expr, $($arg:tt)*) => {
        $reporter.error(&format!($($arg)*))
    };
}
