use std::path::PathBuf;
use tempfile::TempDir;
use tokio::fs;

// Import from the main.rs module
use ck_search::{CkMcpServer, HybridSearchRequest, RegexSearchRequest, SemanticSearchRequest};

#[tokio::test]
async fn test_mcp_semantic_search_basic_functionality() {
    let temp_dir = create_test_files().await;
    let server = CkMcpServer::new(temp_dir.path().to_path_buf()).unwrap();

    // Test first page request
    let request = SemanticSearchRequest {
        query: "function".to_string(),
        path: temp_dir.path().to_string_lossy().to_string(),
        top_k: Some(10),
        threshold: Some(0.1),
        cursor: None,
        page_size: Some(5),
        include_snippet: Some(true),
        snippet_length: Some(100),
        context_lines: Some(0),
        ..Default::default()
    };

    let result = server.handle_semantic_search(request, None, None).await;

    // Verify the result contains pagination information
    if let Ok((summary, response)) = result {
        assert!(summary.contains("Page 1"));

        // Verify response structure
        assert!(response["search"].is_object());
        assert!(response["results"].is_object());
        assert!(response["pagination"].is_object());

        // Check pagination fields
        assert!(response["pagination"]["current_page"].is_number());
        assert!(response["results"]["count"].is_number());
        assert!(response["results"]["has_more"].is_boolean());
        assert_eq!(response["search"]["mode"], "semantic");
    }
}

#[tokio::test]
async fn test_mcp_regex_search_basic_functionality() {
    let temp_dir = create_test_files().await;
    let server = CkMcpServer::new(temp_dir.path().to_path_buf()).unwrap();

    let request = RegexSearchRequest {
        pattern: "function".to_string(),
        path: temp_dir.path().to_string_lossy().to_string(),
        ignore_case: Some(true),
        context: Some(2),
        cursor: None,
        page_size: Some(3),
        include_snippet: Some(true),
        snippet_length: Some(50),
        ..Default::default()
    };

    let result = server.handle_regex_search(request).await;

    if let Ok((summary, response)) = result {
        assert!(summary.contains("Page 1"));

        // Verify response structure matches expected format
        assert_eq!(response["search"]["mode"], "regex");
        assert!(response["results"]["matches"].is_array());
        assert!(response["pagination"]["page_size"].is_number());

        // Check match structure for regex search
        if let Some(matches) = response["results"]["matches"].as_array()
            && !matches.is_empty()
        {
            let first_match = &matches[0];
            assert_eq!(first_match["type"], "regex_match");
            assert!(first_match["match"]["line_number"].is_number());
        }
    }
}

#[tokio::test]
async fn test_mcp_hybrid_search_basic_functionality() {
    let temp_dir = create_test_files().await;
    let server = CkMcpServer::new(temp_dir.path().to_path_buf()).unwrap();

    let request = HybridSearchRequest {
        query: "function error".to_string(),
        path: temp_dir.path().to_string_lossy().to_string(),
        top_k: Some(5),
        threshold: Some(0.01),
        cursor: None,
        page_size: Some(2),
        include_snippet: Some(true),
        snippet_length: Some(200),
        context_lines: Some(1),
        ..Default::default()
    };

    let result = server.handle_hybrid_search(request).await;

    if let Ok((summary, response)) = result {
        assert!(summary.contains("Page 1"));

        // Verify hybrid-specific fields
        assert_eq!(response["search"]["mode"], "hybrid");

        // Check match structure for hybrid search
        if let Some(matches) = response["results"]["matches"].as_array()
            && !matches.is_empty()
        {
            let first_match = &matches[0];
            assert_eq!(first_match["type"], "hybrid_match");
            // Hybrid matches should have both score and rrf_score
            assert!(first_match["match"]["score"].is_number());
            assert!(first_match["match"]["rrf_score"].is_number());
        }
    }
}

#[tokio::test]
async fn test_mcp_invalid_cursor_handling() {
    let temp_dir = create_test_files().await;
    let server = CkMcpServer::new(temp_dir.path().to_path_buf()).unwrap();

    // Test with invalid cursor
    let request = SemanticSearchRequest {
        query: "test".to_string(),
        path: temp_dir.path().to_string_lossy().to_string(),
        top_k: Some(10),
        threshold: Some(0.1),
        cursor: Some("invalid_cursor".to_string()),
        page_size: Some(5),
        include_snippet: Some(true),
        snippet_length: Some(100),
        context_lines: Some(0),
        ..Default::default()
    };

    let result = server.handle_semantic_search(request, None, None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_mcp_search_parameters_validation() {
    let temp_dir = create_test_files().await;
    let server = CkMcpServer::new(temp_dir.path().to_path_buf()).unwrap();

    // Test with extreme page size (should be clamped)
    let request = SemanticSearchRequest {
        query: "test".to_string(),
        path: temp_dir.path().to_string_lossy().to_string(),
        top_k: Some(10),
        threshold: Some(0.1),
        cursor: None,
        page_size: Some(1000), // Should be clamped to 200
        include_snippet: Some(true),
        snippet_length: Some(10000), // Should be clamped to 2000
        context_lines: Some(50),     // Should be clamped to 10
        ..Default::default()
    };

    let result = server.handle_semantic_search(request, None, None).await;

    if let Ok((_, response)) = result {
        // The actual page size in the response should be clamped
        let page_size = response["pagination"]["page_size"].as_u64().unwrap_or(0);
        assert!(page_size <= 200);
    }
}

#[tokio::test]
async fn test_mcp_nonexistent_path() {
    let server = CkMcpServer::new(PathBuf::from("/nonexistent")).unwrap();

    let request = SemanticSearchRequest {
        query: "test".to_string(),
        path: "/definitely/does/not/exist".to_string(),
        top_k: Some(10),
        threshold: Some(0.1),
        cursor: None,
        page_size: Some(5),
        include_snippet: Some(true),
        snippet_length: Some(100),
        context_lines: Some(0),
        ..Default::default()
    };

    let result = server.handle_semantic_search(request, None, None).await;
    assert!(result.is_err());

    if let Err(error) = result {
        let msg = error.to_string();
        assert!(
            msg.contains("does not exist"),
            "expected 'does not exist' in error, got: {msg}"
        );
    }
}

async fn create_test_files() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create some test files with different content
    let files = vec![
        (
            "test1.rs",
            "function main() {\n    println!(\"Hello world\");\n}\n\nfunction helper() {\n    // Some helper code\n}",
        ),
        (
            "test2.js",
            "function calculate(x, y) {\n    return x + y;\n}\n\nfunction error_handler() {\n    console.error(\"An error occurred\");\n}",
        ),
        (
            "test3.py",
            "def process_data(data):\n    try:\n        return data.process()\n    except Exception as e:\n        handle_error(e)\n\ndef handle_error(error):\n    print(f\"Error: {error}\")",
        ),
        (
            "test4.txt",
            "This is a text file with some content.\nIt contains various words and phrases.\nSome lines mention functions and errors.",
        ),
    ];

    for (filename, content) in files {
        let file_path = temp_dir.path().join(filename);
        fs::write(file_path, content)
            .await
            .expect("Failed to write test file");
    }

    temp_dir
}

#[tokio::test]
async fn test_mcp_top_k_page_size_interaction() {
    let temp_dir = create_test_files().await;
    let server = CkMcpServer::new(temp_dir.path().to_path_buf()).unwrap();

    // Test case 1: top_k=5, page_size=3 should give us one page with 3 results, then one with 2
    let request = SemanticSearchRequest {
        query: "test".to_string(), // Should match content in test files
        path: temp_dir.path().to_string_lossy().to_string(),
        top_k: Some(5),
        threshold: Some(0.1),
        cursor: None,
        page_size: Some(3),
        include_snippet: Some(true),
        snippet_length: Some(100),
        context_lines: Some(0),
        ..Default::default()
    };

    let result = server.handle_semantic_search(request, None, None).await;
    assert!(result.is_ok());

    if let Ok((summary, response)) = result {
        // Verify we get at most 3 results in first page
        if let Some(matches) = response["results"]["matches"].as_array() {
            let match_count = matches.len();
            assert!(
                match_count <= 3,
                "First page should have at most 3 matches, got {}",
                match_count
            );
        }

        // Check that we respect the top_k=5 setting
        if let Some(total_count) = response["results"]["total_count"].as_u64() {
            assert!(
                total_count <= 5,
                "Total count should respect top_k=5, got {}",
                total_count
            );
        }

        // Check that summary reflects correct top_k
        assert!(
            summary.contains("top_k: 5"),
            "Summary should show top_k: 5, got: {}",
            summary
        );
    }

    // Test case 2: top_k=2, page_size=10 should give us one page with 2 results max
    let request2 = SemanticSearchRequest {
        query: "function".to_string(),
        path: temp_dir.path().to_string_lossy().to_string(),
        top_k: Some(2),
        threshold: Some(0.1),
        cursor: None,
        page_size: Some(10),
        include_snippet: Some(true),
        snippet_length: Some(100),
        context_lines: Some(0),
        ..Default::default()
    };

    let result2 = server.handle_semantic_search(request2, None, None).await;
    if let Ok((summary2, response2)) = result2 {
        // Check that we respect the top_k=2 setting
        if let Some(total_count) = response2["results"]["total_count"].as_u64() {
            assert!(
                total_count <= 2,
                "Should respect top_k=2 limit, got {} total",
                total_count
            );
        }

        if let Some(matches) = response2["results"]["matches"].as_array() {
            let match_count = matches.len();
            assert!(
                match_count <= 2,
                "Should respect top_k=2 limit, got {} matches",
                match_count
            );
        }

        // Check that summary reflects correct top_k
        assert!(
            summary2.contains("top_k: 2"),
            "Summary should show top_k: 2, got: {}",
            summary2
        );
    }
}

#[tokio::test]
async fn test_mcp_semantic_search_with_missing_files() {
    use std::fs;

    let temp_dir = create_test_files().await;
    let server = CkMcpServer::new(temp_dir.path().to_path_buf()).unwrap();

    // First, do an initial search to ensure the index is created
    let initial_request = SemanticSearchRequest {
        query: "function".to_string(),
        path: temp_dir.path().to_string_lossy().to_string(),
        top_k: Some(10),
        threshold: Some(0.1),
        cursor: None,
        page_size: Some(10),
        include_snippet: Some(true),
        snippet_length: Some(100),
        context_lines: Some(0),
        ..Default::default()
    };

    let _ = server
        .handle_semantic_search(initial_request, None, None)
        .await;

    // Now remove one of the test files to simulate a stale index
    let file_to_remove = temp_dir.path().join("test1.rs");
    if file_to_remove.exists() {
        fs::remove_file(&file_to_remove).expect("Failed to remove test file");
    }

    // Search again - this should handle the missing file gracefully
    let request = SemanticSearchRequest {
        query: "function".to_string(),
        path: temp_dir.path().to_string_lossy().to_string(),
        top_k: Some(20), // Request more results to increase chance of hitting missing file
        threshold: Some(0.1),
        cursor: None,
        page_size: Some(10),
        include_snippet: Some(true),
        snippet_length: Some(100),
        context_lines: Some(0),
        ..Default::default()
    };

    let result = server.handle_semantic_search(request, None, None).await;

    // The search should succeed (not panic or error) even with missing file
    assert!(
        result.is_ok(),
        "Search should succeed even with missing files"
    );

    if let Ok((summary, response)) = result {
        // Should still get results from remaining files
        assert!(response["results"]["matches"].is_array());
        // The summary should not say "unlimited" since we set top_k
        assert!(summary.contains("top_k: 20"));
    }
}
