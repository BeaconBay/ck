#[cfg(feature = "fastembed")]
use fastembed::EmbeddingModel;

fn main() {
    #[cfg(not(feature = "fastembed"))]
    {
        println!("This example requires the 'fastembed' feature to be enabled.");
        println!("Run with: cargo run --example list_models --features fastembed");
        return;
    }

    #[cfg(feature = "fastembed")]
    run_example();
}

#[cfg(feature = "fastembed")]
fn run_example() {
    println!("Available Embedding Models in fastembed-rs:");
    println!("============================================");

    // List all available models by trying to match against known variants
    let models = [
        // Nomic models (8192 context)
        ("NomicEmbedTextV1", EmbeddingModel::NomicEmbedTextV1),
        ("NomicEmbedTextV15", EmbeddingModel::NomicEmbedTextV15),
        // Code-specific models
        (
            "JinaEmbeddingsV2BaseCode",
            EmbeddingModel::JinaEmbeddingsV2BaseCode,
        ),
        // Current default
        ("BGESmallENV15", EmbeddingModel::BGESmallENV15),
        // Other popular ones
        ("AllMiniLML6V2", EmbeddingModel::AllMiniLML6V2),
        ("BGEBaseENV15", EmbeddingModel::BGEBaseENV15),
        ("BGELargeENV15", EmbeddingModel::BGELargeENV15),
    ];

    for (name, model) in models {
        println!("✓ {name} - Available");

        // Try to get model info without downloading
        println!("  Model enum variant: {model:?}");
    }

    println!("\nNote: This just shows the models defined in the enum.");
    println!("Each model will be downloaded on first use.");
}
