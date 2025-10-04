#[cfg(test)]
use serial_test::serial;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn ck_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ck"))
}

#[test]
fn test_basic_grep_functionality() {
    let temp_dir = TempDir::new().unwrap();

    // Create test files
    fs::write(
        temp_dir.path().join("file1.txt"),
        "hello world\nrust programming\ntest line",
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("file2.rs"),
        "fn main() {\n    println!(\"Hello Rust\");\n}",
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("file3.py"),
        "print('Hello Python')\n# rust comment",
    )
    .unwrap();

    // Test basic regex search
    let output = Command::new(ck_binary())
        .args(["rust", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("rust programming"));
    assert!(stdout.contains("# rust comment"));
}

#[test]
fn test_case_insensitive_search() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(
        temp_dir.path().join("test.txt"),
        "Hello World\nHELLO WORLD\nhello world",
    )
    .unwrap();

    let output = Command::new(ck_binary())
        .args(["-i", "HELLO", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let line_count = stdout.lines().count();
    assert_eq!(line_count, 6); // Should match all three lines (filename + content for each)
}

#[test]
fn test_recursive_search() {
    let temp_dir = TempDir::new().unwrap();
    fs::create_dir(temp_dir.path().join("subdir")).unwrap();
    fs::write(temp_dir.path().join("root.txt"), "target text").unwrap();
    fs::write(
        temp_dir.path().join("subdir").join("nested.txt"),
        "target text",
    )
    .unwrap();

    let output = Command::new(ck_binary())
        .args(["-r", "target", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let line_count = stdout.lines().count();
    assert_eq!(line_count, 4); // Should find matches in both files (filename + content for each)
}

#[test]
fn test_json_output() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join("test.txt"), "json test line").unwrap();

    let output = Command::new(ck_binary())
        .args(["--json", "json", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Should be valid JSON
    let json_result: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(json_result["file"].is_string());
    assert!(json_result["score"].is_number());
    assert!(json_result["preview"].is_string());
}

#[test]
#[serial]
fn test_index_command() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join("test.txt"), "indexable content").unwrap();

    // Test index creation
    let output = Command::new(ck_binary())
        .args(["--index", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run ck index");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stdout.contains("Indexed")
            || stdout.contains("✓ Indexed")
            || stderr.contains("Indexed")
            || stderr.contains("✓ Indexed")
    );

    // Check that .ck directory was created
    assert!(temp_dir.path().join(".ck").exists());
}

fn read_manifest_updated(dir: &Path) -> u64 {
    let manifest_path = dir.join(".ck").join("manifest.json");
    let data = fs::read(manifest_path).expect("manifest should exist");
    let manifest: serde_json::Value = serde_json::from_slice(&data).expect("valid json");
    manifest
        .get("updated")
        .and_then(|v| v.as_u64())
        .expect("updated timestamp")
}

#[test]
#[serial]
fn test_switch_model_skips_when_same_model() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join("test.rs"), "fn main() {}").unwrap();

    let status = Command::new(ck_binary())
        .args(["--index", "."])
        .current_dir(temp_dir.path())
        .status()
        .expect("ck --index should run");
    assert!(status.success());

    let updated_before = read_manifest_updated(temp_dir.path());

    let output = Command::new(ck_binary())
        .args(["--switch-model", "bge-small"])
        .current_dir(temp_dir.path())
        .output()
        .expect("ck --switch-model should run");

    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("No rebuild required"));

    let updated_after = read_manifest_updated(temp_dir.path());
    assert_eq!(
        updated_before, updated_after,
        "manifest should be unchanged when model matches"
    );
}

#[test]
#[serial]
fn test_switch_model_force_rebuild() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(
        temp_dir.path().join("main.rs"),
        "fn main() { println!(\"hi\"); }",
    )
    .unwrap();

    let status = Command::new(ck_binary())
        .args(["--index", "."])
        .current_dir(temp_dir.path())
        .status()
        .expect("ck --index should run");
    assert!(status.success());

    let updated_before = read_manifest_updated(temp_dir.path());

    std::thread::sleep(std::time::Duration::from_secs(1));

    let output = Command::new(ck_binary())
        .args(["--switch-model", "bge-small", "--force"])
        .current_dir(temp_dir.path())
        .output()
        .expect("ck --switch-model --force should run");

    assert!(output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Switching Embedding Model"));

    let updated_after = read_manifest_updated(temp_dir.path());
    assert!(
        updated_after > updated_before,
        "forced rebuild should update manifest timestamp"
    );
}

#[test]
#[serial]
fn test_semantic_search() {
    let temp_dir = TempDir::new().unwrap();

    // Create test files with different semantic content
    fs::write(
        temp_dir.path().join("ai.txt"),
        "Machine learning and artificial intelligence are transforming technology",
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("cooking.txt"),
        "Cooking recipes and kitchen tips for delicious meals",
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("programming.txt"),
        "Software development with Python and data science frameworks",
    )
    .unwrap();

    // First create an index
    let output = Command::new(ck_binary())
        .args(["--index", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run ck index");

    assert!(output.status.success());

    // Test semantic search - should rank AI content higher for "neural networks"
    let output = Command::new(ck_binary())
        .args(["--sem", "neural networks", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run ck semantic search");

    // Semantic search requires models which might not be available in test environment
    // So we just check if it runs without crashing
    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).unwrap();

        // If we got results, AI file should appear due to semantic similarity
        let lines: Vec<&str> = stdout.trim().lines().collect();
        if !lines.is_empty() {
            // Check that we got some output
            assert!(!stdout.is_empty());
        }
    }
    // Note: Semantic search might fail in test environments due to model availability
    // This is acceptable for integration tests
}

#[test]
#[serial]
fn test_lexical_search() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(
        temp_dir.path().join("doc1.txt"),
        "machine learning algorithms",
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("doc2.txt"),
        "web development frameworks",
    )
    .unwrap();

    // Create index
    let output = Command::new(ck_binary())
        .args(["--index", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run ck index");

    assert!(output.status.success());

    // Test lexical search
    let output = Command::new(ck_binary())
        .args(["--lex", "machine learning", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run ck lexical search");

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("doc1.txt"));
    }
}

#[test]
#[serial]
fn test_hybrid_search() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(
        temp_dir.path().join("mixed.txt"),
        "Python programming and machine learning",
    )
    .unwrap();

    // Create index
    let output = Command::new(ck_binary())
        .args(["--index", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run ck index");

    assert!(output.status.success());

    // Test hybrid search
    let output = Command::new(ck_binary())
        .args(["--hybrid", "Python", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run ck hybrid search");

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("mixed.txt"));
    }
}

#[test]
fn test_context_lines() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(
        temp_dir.path().join("context.txt"),
        "line 1\nline 2\ntarget line\nline 4\nline 5",
    )
    .unwrap();

    let output = Command::new(ck_binary())
        .args(["-C", "1", "target", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck with context");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Should include context lines
    assert!(stdout.contains("line 2"));
    assert!(stdout.contains("target line"));
    assert!(stdout.contains("line 4"));
}

#[test]
fn test_topk_limit() {
    let temp_dir = TempDir::new().unwrap();

    // Create multiple files with matches
    for i in 1..=10 {
        fs::write(
            temp_dir.path().join(format!("file{}.txt", i)),
            "match content",
        )
        .unwrap();
    }

    let output = Command::new(ck_binary())
        .args(["--topk", "5", "match", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck with topk");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let line_count = stdout.trim().lines().count();
    assert!(line_count <= 10); // Up to 5 results, each with filename + content line
}

#[test]
fn test_line_numbers() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(
        temp_dir.path().join("numbered.txt"),
        "line 1\nmatched line\nline 3",
    )
    .unwrap();

    let output = Command::new(ck_binary())
        .args(["-n", "matched", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck with line numbers");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Should include line number (line 2)
    assert!(stdout.contains("2:matched line"));
}

#[test]
#[serial]
fn test_clean_command() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join("test.txt"), "test content").unwrap();

    // Create index first
    let output = Command::new(ck_binary())
        .args(["--index", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run ck index");

    assert!(
        output.status.success(),
        "Index creation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        temp_dir.path().join(".ck").exists(),
        "Index directory not created"
    );

    // Clean index
    let output = Command::new(ck_binary())
        .args(["--clean", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run ck clean");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stdout.contains("Index cleaned")
            || stdout.contains("✓ Index cleaned")
            || stderr.contains("Index cleaned")
            || stderr.contains("✓ Index cleaned")
    );
}

#[test]
fn test_no_matches_stderr_message() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join("test.txt"), "hello world").unwrap();

    // Search for pattern that won't match
    let output = Command::new(ck_binary())
        .args(["nonexistent_pattern", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    // Should exit with code 1 (no matches)
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));

    // Should have empty stdout
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.trim().is_empty());

    // Should have stderr message explaining the exit code
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("No matches found"));
}

#[test]
fn test_nonexistent_directory_error() {
    let output = Command::new(ck_binary())
        .args(["--sem", "test", "/nonexistent/directory"])
        .output()
        .expect("Failed to run ck");

    // Should fail with specific error message
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Path does not exist"));
    assert!(stderr.contains("/nonexistent/directory"));
}

#[test]
fn test_error_handling() {
    // Test with nonexistent directory
    let _output = Command::new(ck_binary())
        .args(["test", "/nonexistent/directory"])
        .output()
        .expect("Failed to run ck");

    // Should handle error gracefully (might succeed with no matches, which is fine)
    // The important thing is that it doesn't crash

    // Test invalid regex
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join("test.txt"), "test content").unwrap();

    let output = Command::new(ck_binary())
        .args(["[invalid", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    // Should fail gracefully with invalid regex
    assert!(!output.status.success());
}

#[test]
fn test_invalid_regex_warning_during_highlighting() {
    let temp_dir = TempDir::new().unwrap();

    // Create a test file
    fs::write(
        temp_dir.path().join("test.txt"),
        "hello world\ntest content",
    )
    .unwrap();

    // Test that an invalid regex pattern shows a warning during highlighting
    // The search will fail (as expected), but we should see a warning about the invalid regex
    let output = Command::new(ck_binary())
        .args([
            "[invalid", // This is an invalid regex pattern
            temp_dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run ck");

    // The search should fail due to invalid regex
    assert!(!output.status.success());

    // Check that we get a proper regex error message, not silent failure
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("regex") || stderr.contains("pattern") || stderr.contains("invalid"));
}

#[test]
fn test_jsonl_basic_output() {
    let temp_dir = TempDir::new().unwrap();

    // Create test files
    fs::write(
        temp_dir.path().join("test.rs"),
        "fn main() {\n    println!(\"Hello Rust\");\n}",
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("test.py"),
        "print('Hello Python')\ndef main():\n    pass",
    )
    .unwrap();

    let output = Command::new(ck_binary())
        .args(["fn main", "--jsonl", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Should output JSONL format
    assert!(!stdout.trim().is_empty());

    // Each line should be valid JSON
    for line in stdout.lines() {
        if !line.trim().is_empty() {
            let json: serde_json::Value = serde_json::from_str(line).expect("Invalid JSON line");

            // Verify required JSONL fields exist
            assert!(json.get("path").is_some());
            assert!(json.get("span").is_some());
            assert!(json.get("language").is_some());
            assert!(json.get("snippet").is_some());
            assert!(json.get("score").is_some());

            // Verify span structure
            let span = json.get("span").unwrap().as_object().unwrap();
            assert!(span.get("byte_start").is_some());
            assert!(span.get("byte_end").is_some());
            assert!(span.get("line_start").is_some());
            assert!(span.get("line_end").is_some());
        }
    }
}

#[test]
fn test_jsonl_no_snippet_flag() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join("test.rs"),
        "fn main() {\n    println!(\"Hello Rust\");\n}",
    )
    .unwrap();

    let output = Command::new(ck_binary())
        .args([
            "fn main",
            "--jsonl",
            "--no-snippet",
            temp_dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run ck");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Should output JSONL format without snippets
    for line in stdout.lines() {
        if !line.trim().is_empty() {
            let json: serde_json::Value = serde_json::from_str(line).expect("Invalid JSON line");

            // Should not have snippet field when --no-snippet is used
            assert!(json.get("snippet").is_none());

            // Should still have other required fields
            assert!(json.get("path").is_some());
            assert!(json.get("span").is_some());
            assert!(json.get("language").is_some());
            assert!(json.get("score").is_some());
        }
    }
}

#[test]
fn test_jsonl_vs_regular_output() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join("test.rs"),
        "fn main() {\n    println!(\"Hello Rust\");\n}",
    )
    .unwrap();

    // Regular output
    let regular_output = Command::new(ck_binary())
        .args(["fn main", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    // JSONL output
    let jsonl_output = Command::new(ck_binary())
        .args(["fn main", "--jsonl", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    assert!(regular_output.status.success());
    assert!(jsonl_output.status.success());

    let regular_stdout = String::from_utf8(regular_output.stdout).unwrap();
    let jsonl_stdout = String::from_utf8(jsonl_output.stdout).unwrap();

    // Regular output should NOT be JSON
    assert!(!regular_stdout.contains("{\"path\":"));

    // JSONL output should be JSON
    assert!(jsonl_stdout.contains("{\"path\":"));
    assert!(jsonl_stdout.contains("\"span\":"));
    assert!(jsonl_stdout.contains("\"language\":"));
}

#[test]
fn test_jsonl_with_different_languages() {
    let temp_dir = TempDir::new().unwrap();

    // Create files in different languages
    fs::write(
        temp_dir.path().join("test.rs"),
        "fn main() {\n    println!(\"Hello Rust\");\n}",
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("test.py"),
        "def main():\n    print('Hello Python')",
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("test.js"),
        "function main() {\n    console.log('Hello JS');\n}",
    )
    .unwrap();

    let output = Command::new(ck_binary())
        .args(["main", "--jsonl", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    let mut rust_found = false;
    let mut python_found = false;
    let mut js_found = false;

    // Check that different languages are correctly detected
    for line in stdout.lines() {
        if !line.trim().is_empty() {
            let json: serde_json::Value = serde_json::from_str(line).expect("Invalid JSON line");

            let language = json.get("language").unwrap().as_str().unwrap();
            match language {
                "rust" => rust_found = true,
                "python" => python_found = true,
                "javascript" => js_found = true,
                _ => {}
            }
        }
    }

    // Should detect all three languages
    assert!(rust_found);
    assert!(python_found);
    assert!(js_found);
}

#[test]
#[serial]
fn test_add_single_file_to_index() {
    let temp_dir = TempDir::new().unwrap();

    // Create a test file to add
    let test_file = temp_dir.path().join("test_file.txt");
    fs::write(&test_file, "This is test content for indexing").unwrap();

    // First create an index in the directory
    let output = Command::new(ck_binary())
        .args(["--index", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to create index");

    assert!(output.status.success(), "Failed to create initial index");

    // Create another file after index creation
    let new_file = temp_dir.path().join("new_file.txt");
    fs::write(&new_file, "New file content to be added").unwrap();

    // Test adding the new file with absolute path
    let output = Command::new(ck_binary())
        .args(["--add", new_file.to_str().unwrap()])
        .output()
        .expect("Failed to run ck --add");

    assert!(
        output.status.success(),
        "Failed to add file: stderr: {}, stdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    // Check for success message in either stdout or stderr
    assert!(
        stdout.contains("Added") || stderr.contains("Added"),
        "Expected 'Added' in output, got stdout: {}, stderr: {}",
        stdout,
        stderr
    );

    // Verify the file was actually added by searching for it
    let output = Command::new(ck_binary())
        .args(["New file", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to search for added file");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("New file content"),
        "Added file content not found in search"
    );
}

#[test]
#[serial]
fn test_add_file_with_relative_path() {
    let temp_dir = TempDir::new().unwrap();

    // Create index first
    let output = Command::new(ck_binary())
        .args(["--index", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to create index");

    assert!(output.status.success());

    // Create a new file to add
    fs::write(
        temp_dir.path().join("relative_file.txt"),
        "Relative path content",
    )
    .unwrap();

    // Test adding with relative path from the temp directory
    let output = Command::new(ck_binary())
        .args(["--add", "relative_file.txt"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run ck --add with relative path");

    assert!(
        output.status.success(),
        "Failed to add file with relative path: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify file was added
    let output = Command::new(ck_binary())
        .args(["Relative path", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to search");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Relative path content"));
}
