#[cfg(feature = "mixedbread")]
use ck_embed::create_embedder;
#[cfg(feature = "mixedbread")]
use ck_embed::reranker::create_reranker;
#[cfg(feature = "mixedbread")]
use ck_models::{ModelRegistry, RerankModelRegistry};

fn main() {
    #[cfg(not(feature = "mixedbread"))]
    {
        println!("This example requires the 'mixedbread' feature to be enabled.");
        println!("Run with: cargo run --example test_mixedbread --features mixedbread");
        return;
    }

    #[cfg(feature = "mixedbread")]
    run_example();
}

#[cfg(feature = "mixedbread")]
fn run_example() {
    println!("=== Testing Mixedbread Models ===\n");

    // Test 1: Model Registry Resolution
    println!("1. Testing Model Registry Resolution");
    println!("   Checking if 'mxbai-xsmall' alias resolves...");

    let registry = ModelRegistry::default();
    match registry.resolve(Some("mxbai-xsmall")) {
        Ok((alias, config)) => {
            println!("   ✅ Resolved alias: '{}'", alias);
            println!("      Model name: {}", config.name);
            println!("      Provider: {}", config.provider);
            println!("      Dimensions: {}", config.dimensions);
            println!("      Max tokens: {}", config.max_tokens);
        }
        Err(e) => {
            println!("   ❌ Failed to resolve alias: {}", e);
            return;
        }
    }

    // Test 2: Embedder Creation
    println!("\n2. Testing Mixedbread Embedder Creation");
    println!("   Attempting to create Mixedbread embedder...");

    let result = create_embedder(Some("mixedbread-ai/mxbai-embed-xsmall-v1"));

    match result {
        Ok(mut embedder) => {
            println!("   ✅ Successfully created embedder: {}", embedder.id());
            println!("      Model name: {}", embedder.model_name());
            println!("      Dimensions: {}", embedder.dim());

            // Test 3: Embedding Generation
            println!("\n3. Testing Embedding Generation");
            let test_texts = vec![
                "Hello world".to_string(),
                "Rust programming language".to_string(),
                "Machine learning and artificial intelligence".to_string(),
            ];
            println!("   Generating embeddings for {} texts...", test_texts.len());

            match embedder.embed(&test_texts) {
                Ok(embeddings) => {
                    println!("   ✅ Successfully generated embeddings");
                    println!(
                        "      Shape: {} embeddings of {} dimensions",
                        embeddings.len(),
                        embeddings[0].len()
                    );

                    // Verify dimensions
                    assert_eq!(
                        embeddings.len(),
                        test_texts.len(),
                        "Should have one embedding per text"
                    );
                    assert_eq!(
                        embeddings[0].len(),
                        384,
                        "Mixedbread xsmall should produce 384-dim vectors"
                    );

                    // Check normalization (L2 norm should be ~1.0)
                    for (i, emb) in embeddings.iter().enumerate() {
                        let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
                        println!(
                            "      Embedding {} L2 norm: {:.6} (should be ~1.0)",
                            i, norm
                        );
                        assert!(
                            (norm - 1.0).abs() < 0.01,
                            "Embeddings should be L2-normalized"
                        );
                    }
                }
                Err(e) => {
                    println!("   ❌ Failed to generate embeddings: {}", e);
                    return;
                }
            }
        }
        Err(e) => {
            println!("   ❌ Failed to create Mixedbread embedder: {}", e);
            println!("      Error details: {:?}", e);
            return;
        }
    }

    // Test 4: Reranker Registry Resolution
    println!("\n4. Testing Reranker Registry Resolution");
    println!("   Checking if 'mxbai' reranker alias resolves...");

    let rerank_registry = RerankModelRegistry::default();
    match rerank_registry.resolve(Some("mxbai")) {
        Ok((alias, config)) => {
            println!("   ✅ Resolved reranker alias: '{}'", alias);
            println!("      Model name: {}", config.name);
            println!("      Provider: {}", config.provider);
        }
        Err(e) => {
            println!("   ❌ Failed to resolve reranker alias: {}", e);
            return;
        }
    }

    // Test 5: Reranker Creation
    println!("\n5. Testing Mixedbread Reranker Creation");
    println!("   Attempting to create Mixedbread reranker...");

    match create_reranker(Some("mixedbread-ai/mxbai-rerank-xsmall-v1")) {
        Ok(mut reranker) => {
            println!("   ✅ Successfully created reranker: {}", reranker.id());

            // Test 6: Reranking
            println!("\n6. Testing Reranking");
            let query = "error handling in Rust";
            let documents = vec![
                "Rust error handling with Result and Option types".to_string(),
                "Python web development frameworks".to_string(),
                "Rust provides excellent error handling mechanisms".to_string(),
                "JavaScript async programming patterns".to_string(),
            ];
            println!("   Query: '{}'", query);
            println!("   Reranking {} documents...", documents.len());

            match reranker.rerank(query, &documents) {
                Ok(results) => {
                    println!("   ✅ Successfully reranked documents");
                    println!("      Results (sorted by score):");
                    for (i, result) in results.iter().enumerate() {
                        println!(
                            "      {}. Score: {:.4} | Doc: {}",
                            i + 1,
                            result.score,
                            if result.document.len() > 60 {
                                &result.document[..60]
                            } else {
                                &result.document
                            }
                        );
                    }

                    // Verify results are sorted by score (descending)
                    let scores: Vec<f32> = results.iter().map(|r| r.score).collect();
                    let sorted_scores: Vec<f32> = {
                        let mut s = scores.clone();
                        s.sort_by(|a, b| b.partial_cmp(a).unwrap());
                        s
                    };
                    assert_eq!(
                        scores, sorted_scores,
                        "Results should be sorted by score descending"
                    );

                    // Verify scores are in valid range [0, 1]
                    for result in &results {
                        assert!(
                            result.score >= 0.0 && result.score <= 1.0,
                            "Rerank scores should be in [0, 1] range"
                        );
                    }
                }
                Err(e) => {
                    println!("   ❌ Failed to rerank: {}", e);
                    return;
                }
            }
        }
        Err(e) => {
            println!("   ❌ Failed to create Mixedbread reranker: {}", e);
            println!("      Error details: {:?}", e);
            return;
        }
    }

    println!("\n=== All Tests Passed! ===");
}
