#[cfg(feature = "fastembed")]
use ck_embed::create_reranker;

fn main() {
    #[cfg(not(feature = "fastembed"))]
    {
        println!("This example requires the 'fastembed' feature to be enabled.");
        println!("Run with: cargo run --example test_reranking --features fastembed");
        return;
    }

    #[cfg(feature = "fastembed")]
    run_example();
}

#[cfg(feature = "fastembed")]
fn run_example() {
    println!("=== Reranking Test ===");

    let mut reranker = create_reranker(None).expect("Failed to create reranker");

    println!("Created reranker: {}", reranker.id());

    let query = "error handling in programming";
    let documents = vec![
        "try catch exception handling in Java".to_string(),
        "user interface design patterns".to_string(),
        "error handling with Result types in Rust".to_string(),
        "database connection management".to_string(),
        "exception handling best practices".to_string(),
    ];

    println!("\nQuery: '{query}'");
    println!("Documents to rerank:");
    for (i, doc) in documents.iter().enumerate() {
        println!("  {}: {}", i + 1, doc);
    }

    let results = reranker
        .rerank(query, &documents)
        .expect("Failed to rerank documents");

    println!("\nReranked results:");
    // Sort by score (descending)
    let mut sorted_results = results;
    sorted_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

    for (i, result) in sorted_results.iter().enumerate() {
        println!(
            "  {}: [Score: {:.3}] {}",
            i + 1,
            result.score,
            result.document
        );
    }

    println!("\n✅ Reranking test completed successfully!");
}
