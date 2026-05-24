use anyhow::Result;
use ck_core::IncludePattern;
use glob::glob;
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::{Component, Path, PathBuf};

/// Expand user-provided glob patterns, mimicking shell behaviour while tolerating
/// unmatched globs by keeping the original pattern. Bare filename globs (e.g.
/// `*.rs`) automatically get a recursive fallback to align with the CLI UX.
pub fn expand_glob_patterns(
    paths: &[PathBuf],
    exclude_patterns: &[String],
) -> Result<Vec<PathBuf>> {
    expand_glob_patterns_internal(paths, exclude_patterns, None)
}

pub fn expand_glob_patterns_with_base(
    base_dir: &Path,
    paths: &[PathBuf],
    exclude_patterns: &[String],
) -> Result<Vec<PathBuf>> {
    expand_glob_patterns_internal(paths, exclude_patterns, Some(base_dir))
}

/// Build IncludePattern structures from filesystem paths, canonicalising where
/// possible and marking whether the path points to a directory.
pub fn build_include_patterns(paths: &[PathBuf]) -> Vec<IncludePattern> {
    let mut includes: Vec<IncludePattern> = Vec::new();

    for path in paths {
        let canonical = canonicalize_lossy(path);
        let is_dir = std::fs::metadata(&canonical)
            .map(|meta| meta.is_dir())
            .unwrap_or(false);

        if let Some(existing) = includes.iter_mut().find(|inc| inc.path == canonical) {
            if is_dir {
                existing.is_dir = true;
            }
        } else {
            includes.push(IncludePattern {
                path: canonical,
                is_dir,
            });
        }
    }

    includes
}

pub(crate) fn split_path_patterns(path: &Path) -> Vec<String> {
    let path_str = path.to_string_lossy();
    if !path_str.contains(';') {
        return vec![path_str.to_string()];
    }

    path_str
        .split(';')
        .filter_map(|segment| {
            let trimmed = segment.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect()
}

fn expand_glob_patterns_internal(
    paths: &[PathBuf],
    exclude_patterns: &[String],
    base_dir: Option<&Path>,
) -> Result<Vec<PathBuf>> {
    let globset = build_globset(exclude_patterns);
    let mut expanded = Vec::new();

    for path in paths {
        for pattern in split_path_patterns(path) {
            let is_simple = !pattern.contains('/') && !pattern.contains('\\');

            let glob_path = if let Some(base) = base_dir {
                let candidate = Path::new(&pattern);
                if candidate.is_absolute() {
                    candidate.to_path_buf()
                } else {
                    base.join(&pattern)
                }
            } else {
                PathBuf::from(&pattern)
            };

            let glob_str = glob_path.to_string_lossy().to_string();
            let mut matched = run_glob(&glob_str, &globset, base_dir, &mut expanded)?;

            if is_simple {
                let fallback_path = if let Some(base) = base_dir {
                    base.join(format!("**/{pattern}"))
                } else {
                    PathBuf::from(format!("**/{pattern}"))
                };
                let fallback_str = fallback_path.to_string_lossy().to_string();
                matched |= run_glob(&fallback_str, &globset, base_dir, &mut expanded)?;
            }

            if !matched {
                push_if_new(&mut expanded, glob_path);
            }
        }
    }

    Ok(expanded)
}

fn run_glob(
    pattern: &str,
    globset: &GlobSet,
    base_dir: Option<&Path>,
    expanded: &mut Vec<PathBuf>,
) -> Result<bool> {
    let mut matched = false;
    match glob(pattern) {
        Ok(glob_paths) => {
            for glob_result in glob_paths {
                match glob_result {
                    Ok(matched_path) => {
                        if should_exclude_path(&matched_path, globset, base_dir) {
                            continue;
                        }
                        matched = true;
                        push_if_new(expanded, matched_path);
                    }
                    Err(e) => {
                        eprintln!("Warning: glob error for pattern '{pattern}': {e}");
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Warning: invalid glob pattern '{pattern}': {e}");
        }
    }
    Ok(matched)
}

fn push_if_new(acc: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !acc.iter().any(|existing| existing == &candidate) {
        acc.push(candidate);
    }
}

fn canonicalize_lossy(path: &Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }

    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

fn should_exclude_path(path: &Path, globset: &GlobSet, base_dir: Option<&Path>) -> bool {
    if globset.is_match(path) {
        return true;
    }

    if let Some(base) = base_dir
        && let Ok(relative) = path.strip_prefix(base)
    {
        if !relative.as_os_str().is_empty() && globset.is_match(relative) {
            return true;
        }

        for component in relative.components() {
            if let Component::Normal(name) = component
                && globset.is_match(name)
            {
                return true;
            }
        }
    }

    for component in path.components() {
        if let Component::Normal(name) = component
            && globset.is_match(name)
        {
            return true;
        }
    }

    false
}

fn build_globset(patterns: &[String]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();

    for pattern in patterns {
        if let Ok(glob) = Glob::new(pattern) {
            builder.add(glob);
        }

        if let Some(stripped) = pattern.strip_suffix("/**") {
            if !stripped.is_empty()
                && let Ok(glob) = Glob::new(stripped)
            {
                builder.add(glob);
            }
        } else if let Some(stripped) = pattern.strip_suffix("\\**") {
            // Support Windows-style globstar suffixes as well.
            if !stripped.is_empty()
                && let Ok(glob) = Glob::new(stripped)
            {
                builder.add(glob);
            }
        }
    }

    builder.build().unwrap_or_else(|_| GlobSet::empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn expands_basic_glob_patterns() {
        let temp_dir = tempdir().unwrap();
        let base = temp_dir.path();

        write_file(&base.join("alpha.rs"), "fn main() {}");
        write_file(&base.join("nested/beta.rs"), "fn helper() {}");
        write_file(&base.join("gamma.ts"), "export const X = 1;");

        let expanded = expand_glob_patterns_with_base(base, &[PathBuf::from("*.rs")], &[])
            .expect("expand *.rs");

        let has_alpha = expanded.iter().any(|p| p.ends_with("alpha.rs"));
        let has_beta = expanded.iter().any(|p| p.ends_with("beta.rs"));

        assert!(has_alpha, "alpha.rs should be expanded");
        assert!(
            has_beta,
            "nested/beta.rs should be expanded via recursive fallback"
        );
        assert_eq!(expanded.len(), 2);
    }

    #[test]
    fn expands_literal_files_and_directories() {
        let temp_dir = tempdir().unwrap();
        let base = temp_dir.path();

        write_file(&base.join("docs/guide.md"), "# Guide");
        write_file(&base.join("src/file.ts"), "export {}");

        let expanded = expand_glob_patterns_with_base(
            base,
            &[PathBuf::from("docs/"), PathBuf::from("src/file.ts")],
            &[],
        )
        .expect("expand literals");

        let has_docs = expanded.iter().any(|p| p.ends_with("docs"));
        let has_ts = expanded.iter().any(|p| p.ends_with("file.ts"));

        assert!(has_docs, "docs directory should be included");
        assert!(has_ts, "file.ts should be included");
    }

    #[test]
    fn splits_semicolon_separated_patterns() {
        let temp_dir = tempdir().unwrap();
        let base = temp_dir.path();

        write_file(&base.join("docs/readme.md"), "# docs");
        write_file(&base.join("lib/lib.rs"), "pub fn lib() {}");
        write_file(&base.join("file.ts"), "export {}");

        let expanded =
            expand_glob_patterns_with_base(base, &[PathBuf::from("docs/;*.rs;file.ts")], &[])
                .expect("expand semicolon list");

        let has_docs = expanded.iter().any(|p| p.ends_with("docs"));
        let has_rs = expanded.iter().any(|p| p.ends_with("lib.rs"));
        let has_ts = expanded.iter().any(|p| p.ends_with("file.ts"));

        assert!(has_docs, "docs directory should be present");
        assert!(has_rs, "lib.rs should be matched from glob");
        assert!(has_ts, "file.ts should be included explicitly");
    }

    #[test]
    fn respects_wildcard_exclude_patterns() {
        let temp_dir = tempdir().unwrap();
        let base = temp_dir.path();

        write_file(&base.join("keep.rs"), "fn main() {}");
        write_file(&base.join("ignore.json"), "{}");
        write_file(&base.join("nested/keep.rs"), "fn helper() {}");
        write_file(&base.join("nested/ignore.json"), "{}");

        let expanded =
            expand_glob_patterns_with_base(base, &[PathBuf::from("**/*")], &["*.json".to_string()])
                .expect("expand with excludes");

        let includes_json = expanded.iter().any(|p| p.ends_with("ignore.json"));
        let includes_rs = expanded.iter().filter(|p| p.ends_with("keep.rs")).count();

        assert!(!includes_json, "*.json files should be excluded");
        assert_eq!(includes_rs, 2, "both keep.rs files should remain");
    }

    #[test]
    fn respects_directory_globstar_excludes() {
        let temp_dir = tempdir().unwrap();
        let base = temp_dir.path();

        write_file(&base.join("foo/bar/data.txt"), "foo");
        write_file(&base.join("foo/other.txt"), "foo");
        write_file(&base.join("root.txt"), "root");

        let expanded =
            expand_glob_patterns_with_base(base, &[PathBuf::from("**/*")], &["foo/**".to_string()])
                .expect("expand with directory glob");

        let includes_foo = expanded.iter().any(|p| p.to_string_lossy().contains("foo"));
        let includes_root = expanded.iter().any(|p| p.ends_with("root.txt"));

        assert!(!includes_foo, "foo/** should exclude everything under foo");
        assert!(includes_root, "root.txt should still be present");
    }
}
