use ck_core::heatmap::{self, HeatmapBucket};
use ratatui::style::Color;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

pub fn syntax_set() -> &'static SyntaxSet {
    static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

pub fn theme_set() -> &'static ThemeSet {
    static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

pub use heatmap::{calculate_token_similarity, split_into_tokens};

pub fn score_to_color(score: f32) -> Color {
    match HeatmapBucket::from_score(score) {
        HeatmapBucket::Step8 => Color::Rgb(0, 255, 100),
        HeatmapBucket::Step7 => Color::Rgb(0, 200, 80),
        HeatmapBucket::Step6 => Color::Rgb(0, 160, 70),
        HeatmapBucket::Step5 => Color::Rgb(100, 140, 60),
        _ => Color::Rgb(140, 140, 140),
    }
}

pub fn apply_heatmap_color_to_token(token: &str, score: f32) -> Color {
    // Skip coloring whitespace and punctuation
    if token.trim().is_empty() || token.chars().all(|c| !c.is_alphanumeric()) {
        return Color::Reset;
    }

    // 8-step linear gradient: grey to green with bright final step
    let bucket = HeatmapBucket::from_score(score);

    bucket
        .rgb()
        .map(|(r, g, b)| Color::Rgb(r, g, b))
        .unwrap_or(Color::Reset)
}

pub fn find_repo_root(path: &Path) -> Option<PathBuf> {
    let mut current = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };

    loop {
        if ck_core::index_exists(current) {
            return Some(current.to_path_buf());
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => return None,
        }
    }
}
