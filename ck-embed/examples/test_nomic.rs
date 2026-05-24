#[cfg(feature = "fastembed")]
use ck_embed::create_embedder;

fn main() {
    #[cfg(not(feature = "fastembed"))]
    {
        println!("This example requires the 'fastembed' feature to be enabled.");
        println!("Run with: cargo run --example test_nomic --features fastembed");
        return;
    }

    #[cfg(feature = "fastembed")]
    run_example();
}

#[cfg(feature = "fastembed")]
fn run_example() {
    println!("=== Testing Nomic Model ===");

    println!("Attempting to create Nomic Embed Text V1.5 embedder...");

    let result = create_embedder(Some("nomic-embed-text-v1.5"));

    match result {
        Ok(mut embedder) => {
            println!("✅ Successfully created embedder: {}", embedder.id());
            println!("   Dimensions: {}", embedder.dim());

            let test_texts = vec!["hello world".to_string()];
            println!("Testing embedding generation...");

            match embedder.embed(&test_texts) {
                Ok(embeddings) => {
                    println!("✅ Successfully generated embeddings");
                    println!(
                        "   Shape: {} embeddings of {} dimensions",
                        embeddings.len(),
                        embeddings[0].len()
                    );
                }
                Err(e) => {
                    println!("❌ Failed to generate embeddings: {e}");
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to create Nomic embedder: {e}");
            println!("   Error details: {e:?}");

            println!("\n--- Trying alternative model names ---");

            // Try different variations
            let alternatives = [
                "nomic-embed-text-v1",
                "NomicEmbedTextV15",
                "NomicEmbedTextV1",
            ];

            for alt in alternatives {
                println!("Trying: {alt}");
                match create_embedder(Some(alt)) {
                    Ok(_) => println!("  ✅ {alt} works!"),
                    Err(e) => println!("  ❌ {alt} failed: {e}"),
                }
            }
        }
    }

    println!("\n--- Testing working BGE model for comparison ---");
    match create_embedder(Some("BAAI/bge-small-en-v1.5")) {
        Ok(embedder) => println!("✅ BGE model works: {} dims", embedder.dim()),
        Err(e) => println!("❌ Even BGE failed: {e}"),
    }
}
