use console::{Term, style};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::time::Duration;

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
                    style(format!("and {} more files", remaining)).dim()
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

#[macro_export]
macro_rules! status_error {
    ($reporter:expr, $($arg:tt)*) => {
        $reporter.error(&format!($($arg)*))
    };
}
