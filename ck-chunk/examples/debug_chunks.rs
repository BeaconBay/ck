use ck_chunk::{ChunkConfig, chunk_text, chunk_text_with_config};
use ck_core::Language;

fn estimate_tokens(text: &str) -> usize {
    // Match the estimation in ck-chunk
    if text.is_empty() {
        return 0;
    }

    let char_count = text.chars().count();
    (char_count as f32 / 4.5).ceil() as usize
}

#[allow(dead_code)]
fn test_file(path: &str, language: Language) {
    println!("=== Testing {path} ===");
    let code = std::fs::read_to_string(path).unwrap_or_else(|_| panic!("Failed to read {path}"));

    println!("File length: {} characters", code.len());
    println!("Estimated tokens: {}", estimate_tokens(&code));

    let chunks = chunk_text(&code, Some(language)).expect("Failed to chunk code");

    println!("\nGenerated {} chunks:", chunks.len());

    let mut over_limit = 0;
    for (i, chunk) in chunks.iter().enumerate() {
        let token_estimate = estimate_tokens(&chunk.text);
        println!(
            "Chunk {}: {} chars, ~{} tokens, type: {:?}",
            i + 1,
            chunk.text.len(),
            token_estimate,
            chunk.chunk_type
        );

        if token_estimate > 512 {
            over_limit += 1;
            println!("  ❌ WARNING: Chunk exceeds 512 token limit!");
        } else {
            println!("  ✅ Within 512 token limit");
        }

        // Show first few lines of the chunk
        let lines: Vec<&str> = chunk.text.lines().take(3).collect();
        for line in lines {
            println!("  {line}");
        }
        if chunk.text.lines().count() > 3 {
            println!("  ...");
        }
        println!();
    }

    println!(
        "Summary: {}/{} chunks exceed 512 token limit\n",
        over_limit,
        chunks.len()
    );
}

#[allow(dead_code)]
fn test_striding() {
    println!("=== Testing Striding Functionality ===");

    // Configure striding with a small limit to test the mechanism
    let config = ChunkConfig {
        max_tokens: 200,    // Very small limit to trigger striding
        stride_overlap: 50, // 25% overlap
        enable_striding: true,
    };

    let code = std::fs::read_to_string("examples/code/large_function.py")
        .expect("Failed to read large Python file");

    println!(
        "Original file: {} chars, ~{} tokens",
        code.len(),
        estimate_tokens(&code)
    );

    let chunks = chunk_text_with_config(&code, Some(Language::Python), &config)
        .expect("Failed to chunk with striding");

    println!("\nWith striding (limit: {} tokens):", config.max_tokens);
    println!("Generated {} chunks:", chunks.len());

    let mut strided_count = 0;
    for (i, chunk) in chunks.iter().enumerate() {
        let token_estimate = estimate_tokens(&chunk.text);
        let stride_info = if let Some(ref info) = chunk.stride_info {
            strided_count += 1;
            format!(
                " [STRIDE {}/{} from {}]",
                info.stride_index + 1,
                info.total_strides,
                &info.original_chunk_id[..20]
            )
        } else {
            " [ORIGINAL]".to_string()
        };

        println!(
            "Chunk {}: {} chars, ~{} tokens, type: {:?}{}",
            i + 1,
            chunk.text.len(),
            token_estimate,
            chunk.chunk_type,
            stride_info
        );

        if token_estimate > config.max_tokens {
            println!("  ⚠️  Still exceeds limit! ({token_estimate})");
        } else {
            println!("  ✅ Within limit");
        }
    }

    println!(
        "\nSummary: {}/{} chunks are strided",
        strided_count,
        chunks.len()
    );
}

fn main() {
    // Skip file tests that require specific paths, focus on striding
    println!("=== Striding Test ===");

    // Create a large synthetic function to test striding
    let large_code = r#"
def very_large_function():
    """
    This is a very large function that will definitely exceed token limits.
    It contains a lot of logic and comments to make it realistically large.
    """
    # Initialize variables
    result = []
    config = {"timeout": 30, "retries": 3}
    
    # First major section - input validation
    if not data:
        print("No data provided")
        return None
    
    for i in range(100):
        if i % 2 == 0:
            result.append(f"Even number: {i}")
        else:
            result.append(f"Odd number: {i}")
    
    # Second major section - processing
    processed_data = []
    for item in result:
        processed_item = item.upper()
        if len(processed_item) > 10:
            processed_item = processed_item[:10] + "..."
        processed_data.append(processed_item)
    
    # Third major section - more complex logic
    final_result = {}
    for idx, item in enumerate(processed_data):
        key = f"item_{idx}"
        final_result[key] = {
            "value": item,
            "index": idx,
            "is_even": idx % 2 == 0,
            "length": len(item)
        }
    
    # Fourth major section - validation and cleanup
    cleaned_result = {}
    for key, value in final_result.items():
        if value["length"] > 5:
            cleaned_result[key] = value
    
    # Fifth major section - return processing
    if len(cleaned_result) == 0:
        return {"status": "empty", "count": 0}
    
    return {
        "status": "success", 
        "count": len(cleaned_result),
        "data": cleaned_result,
        "metadata": {
            "processed_at": "2024-01-01",
            "version": "1.0",
            "algorithm": "basic"
        }
    }
"#;

    println!(
        "Large synthetic function: {} chars, ~{} tokens",
        large_code.len(),
        estimate_tokens(large_code)
    );

    // Test with normal chunking (no striding)
    println!("\n=== Without Striding ===");
    let normal_chunks =
        chunk_text(large_code, Some(Language::Python)).expect("Failed to chunk without striding");

    println!("Generated {} chunks:", normal_chunks.len());
    for (i, chunk) in normal_chunks.iter().enumerate() {
        let tokens = estimate_tokens(&chunk.text);
        println!(
            "Chunk {}: {} chars, ~{} tokens",
            i + 1,
            chunk.text.len(),
            tokens
        );
    }

    // Test with realistic Nomic model limit (8192 tokens)
    println!("\n=== With Nomic Model Limits (8192 token limit) ===");
    let config = ChunkConfig {
        max_tokens: 8192,     // Nomic model's actual limit
        stride_overlap: 1024, // 12.5% overlap
        enable_striding: true,
    };

    let strided_chunks = chunk_text_with_config(large_code, Some(Language::Python), &config)
        .expect("Failed to chunk with striding");

    println!("Generated {} chunks:", strided_chunks.len());
    let mut strided_count = 0;
    for (i, chunk) in strided_chunks.iter().enumerate() {
        let tokens = estimate_tokens(&chunk.text);
        let stride_info = if chunk.stride_info.is_some() {
            strided_count += 1;
            " [STRIDED]"
        } else {
            " [ORIGINAL]"
        };

        println!(
            "Chunk {}: {} chars, ~{} tokens{}",
            i + 1,
            chunk.text.len(),
            tokens,
            stride_info
        );

        if tokens > config.max_tokens {
            println!("  ❌ Still exceeds 8192 token limit!");
        } else {
            println!("  ✅ Fits in Nomic model context window");
        }
    }

    println!(
        "\nResult: {}/{} chunks are strided",
        strided_count,
        strided_chunks.len()
    );
}
