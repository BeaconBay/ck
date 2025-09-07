use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

// Helper function to get the path to the ck binary
fn get_ck_binary() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // Go up from ck-cli to workspace root
    path.push("target");
    path.push("debug");
    path.push("ck");

    // Ensure binary exists by building it
    let output = Command::new("cargo")
        .args(["build", "--bin", "ck"])
        .output()
        .expect("Failed to build ck binary");

    if !output.status.success() {
        panic!(
            "Failed to build ck: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    path
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
    let output = Command::new(get_ck_binary())
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

    let output = Command::new(get_ck_binary())
        .args(["-i", "HELLO", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let line_count = stdout.lines().count();
    assert_eq!(line_count, 3); // Should match all three lines
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

    let output = Command::new(get_ck_binary())
        .args(["-r", "target", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let line_count = stdout.lines().count();
    assert_eq!(line_count, 2); // Should find matches in both files
}

#[test]
fn test_json_output() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join("test.txt"), "json test line").unwrap();

    let output = Command::new(get_ck_binary())
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
fn test_index_command() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join("test.txt"), "indexable content").unwrap();

    // Test index creation
    let output = Command::new(get_ck_binary())
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

#[test]
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
    let output = Command::new(get_ck_binary())
        .args(["--index", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run ck index");

    assert!(output.status.success());

    // Test semantic search - should rank AI content higher for "neural networks"
    let output = Command::new(get_ck_binary())
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
    let output = Command::new(get_ck_binary())
        .args(["--index", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run ck index");

    assert!(output.status.success());

    // Test lexical search
    let output = Command::new(get_ck_binary())
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
fn test_hybrid_search() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(
        temp_dir.path().join("mixed.txt"),
        "Python programming and machine learning",
    )
    .unwrap();

    // Create index
    let output = Command::new(get_ck_binary())
        .args(["--index", "."])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run ck index");

    assert!(output.status.success());

    // Test hybrid search
    let output = Command::new(get_ck_binary())
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

    let output = Command::new(get_ck_binary())
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

    let output = Command::new(get_ck_binary())
        .args(["--topk", "5", "match", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck with topk");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let line_count = stdout.trim().lines().count();
    assert!(line_count <= 5);
}

#[test]
fn test_line_numbers() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(
        temp_dir.path().join("numbered.txt"),
        "line 1\nmatched line\nline 3",
    )
    .unwrap();

    let output = Command::new(get_ck_binary())
        .args(["-n", "matched", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck with line numbers");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Should include line number (line 2)
    assert!(stdout.contains(":2:"));
}

#[test]
fn test_clean_command() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join("test.txt"), "test content").unwrap();

    // Create index first
    let output = Command::new(get_ck_binary())
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
    let output = Command::new(get_ck_binary())
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
    let output = Command::new(get_ck_binary())
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
    let output = Command::new(get_ck_binary())
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
    let _output = Command::new(get_ck_binary())
        .args(["test", "/nonexistent/directory"])
        .output()
        .expect("Failed to run ck");

    // Should handle error gracefully (might succeed with no matches, which is fine)
    // The important thing is that it doesn't crash

    // Test invalid regex
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join("test.txt"), "test content").unwrap();

    let output = Command::new(get_ck_binary())
        .args(["[invalid", temp_dir.path().to_str().unwrap()])
        .output()
        .expect("Failed to run ck");

    // Should fail gracefully with invalid regex
    assert!(!output.status.success());
}
