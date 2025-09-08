use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub trait AnnIndex: Send + Sync {
    fn build(vectors: &[Vec<f32>]) -> Result<Self>
    where
        Self: Sized;
    fn search(&self, query: &[f32], topk: usize) -> Vec<(u32, f32)>;
    fn add(&mut self, id: u32, vector: &[f32]);
    fn save(&self, path: &Path) -> Result<()>;
    fn load(path: &Path) -> Result<Self>
    where
        Self: Sized;
}

pub fn create_index(_backend: Option<&str>) -> Result<Box<dyn AnnIndex>> {
    Ok(Box::new(SimpleIndex::new()?))
}

#[derive(Serialize, Deserialize)]
pub struct SimpleIndex {
    vectors: Vec<Vec<f32>>,
    ids: Vec<u32>,
    dim: usize,
}

impl SimpleIndex {
    pub fn new() -> Result<Self> {
        Ok(Self {
            vectors: Vec::new(),
            ids: Vec::new(),
            dim: 0,
        })
    }

    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot_product / (norm_a * norm_b)
        }
    }
}

impl AnnIndex for SimpleIndex {
    fn build(vectors: &[Vec<f32>]) -> Result<Self>
    where
        Self: Sized,
    {
        if vectors.is_empty() {
            return Self::new();
        }

        let dim = vectors[0].len();
        let ids: Vec<u32> = (0..vectors.len() as u32).collect();

        Ok(Self {
            vectors: vectors.to_vec(),
            ids,
            dim,
        })
    }

    fn search(&self, query: &[f32], topk: usize) -> Vec<(u32, f32)> {
        let mut similarities: Vec<_> = self
            .vectors
            .iter()
            .zip(&self.ids)
            .map(|(vector, &id)| {
                let similarity = self.cosine_similarity(query, vector);
                (id, similarity)
            })
            .collect();

        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        similarities.truncate(topk);
        similarities
    }

    fn add(&mut self, id: u32, vector: &[f32]) {
        if self.dim == 0 {
            self.dim = vector.len();
        }

        self.vectors.push(vector.to_vec());
        self.ids.push(id);
    }

    fn save(&self, path: &Path) -> Result<()> {
        let data = bincode::serialize(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    fn load(path: &Path) -> Result<Self>
    where
        Self: Sized,
    {
        let data = std::fs::read(path)?;
        let index: Self = bincode::deserialize(&data)?;
        Ok(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_simple_index_new() {
        let index = SimpleIndex::new().unwrap();
        assert_eq!(index.vectors.len(), 0);
        assert_eq!(index.ids.len(), 0);
        assert_eq!(index.dim, 0);
    }

    #[test]
    fn test_simple_index_build_empty() {
        let vectors: Vec<Vec<f32>> = vec![];
        let index = SimpleIndex::build(&vectors).unwrap();
        assert_eq!(index.vectors.len(), 0);
        assert_eq!(index.dim, 0);
    }

    #[test]
    fn test_simple_index_build() {
        let vectors = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];

        let index = SimpleIndex::build(&vectors).unwrap();
        assert_eq!(index.vectors.len(), 3);
        assert_eq!(index.ids.len(), 3);
        assert_eq!(index.dim, 3);
        assert_eq!(index.ids, vec![0, 1, 2]);
    }

    #[test]
    fn test_cosine_similarity() {
        let index = SimpleIndex::new().unwrap();

        // Identical vectors should have similarity 1.0
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = index.cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);

        // Orthogonal vectors should have similarity 0.0
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = index.cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < 1e-6);

        // Opposite vectors should have similarity -1.0
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = index.cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_zero_vectors() {
        let index = SimpleIndex::new().unwrap();

        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = index.cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);

        let a = vec![1.0, 2.0, 3.0];
        let b = vec![0.0, 0.0, 0.0];
        let sim = index.cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_search() {
        let vectors = vec![
            vec![1.0, 0.0, 0.0], // id=0
            vec![0.0, 1.0, 0.0], // id=1
            vec![0.5, 0.5, 0.0], // id=2
        ];

        let index = SimpleIndex::build(&vectors).unwrap();

        // Query closest to first vector
        let query = vec![0.9, 0.1, 0.0];
        let results = index.search(&query, 2);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 0); // First result should be vector 0
        assert!(results[0].1 > results[1].1); // First should have higher similarity
    }

    #[test]
    fn test_search_empty_index() {
        let vectors: Vec<Vec<f32>> = vec![];
        let index = SimpleIndex::build(&vectors).unwrap();

        let query = vec![1.0, 0.0];
        let results = index.search(&query, 5);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_search_topk_limit() {
        let vectors = vec![
            vec![1.0, 0.0],
            vec![0.9, 0.1],
            vec![0.8, 0.2],
            vec![0.7, 0.3],
            vec![0.6, 0.4],
        ];

        let index = SimpleIndex::build(&vectors).unwrap();

        let query = vec![1.0, 0.0];
        let results = index.search(&query, 3);

        assert_eq!(results.len(), 3);
        // Results should be sorted by similarity (descending)
        for i in 1..results.len() {
            assert!(results[i - 1].1 >= results[i].1);
        }
    }

    #[test]
    fn test_add() {
        let mut index = SimpleIndex::new().unwrap();

        index.add(100, &[1.0, 2.0, 3.0]);
        assert_eq!(index.vectors.len(), 1);
        assert_eq!(index.ids.len(), 1);
        assert_eq!(index.ids[0], 100);
        assert_eq!(index.dim, 3);

        index.add(200, &[4.0, 5.0, 6.0]);
        assert_eq!(index.vectors.len(), 2);
        assert_eq!(index.ids.len(), 2);
        assert_eq!(index.ids[1], 200);
    }

    #[test]
    fn test_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("test_index.bin");

        // Create and save index
        let vectors = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let index = SimpleIndex::build(&vectors).unwrap();
        index.save(&index_path).unwrap();

        // Load index
        let loaded_index = SimpleIndex::load(&index_path).unwrap();

        assert_eq!(loaded_index.vectors.len(), index.vectors.len());
        assert_eq!(loaded_index.ids.len(), index.ids.len());
        assert_eq!(loaded_index.dim, index.dim);

        // Test that loaded index works the same
        let query = vec![1.0, 2.0, 3.0];
        let original_results = index.search(&query, 2);
        let loaded_results = loaded_index.search(&query, 2);

        assert_eq!(original_results.len(), loaded_results.len());
        for (orig, loaded) in original_results.iter().zip(&loaded_results) {
            assert_eq!(orig.0, loaded.0);
            assert!((orig.1 - loaded.1).abs() < 1e-6);
        }
    }

    #[test]
    fn test_create_index() {
        let _index = create_index(None).unwrap();

        // Should create a SimpleIndex
        // We can't directly test the type, but we can test the interface
        let vectors = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let index = SimpleIndex::build(&vectors).unwrap();

        let query = vec![1.0, 0.0];
        let results = index.search(&query, 1);
        assert!(!results.is_empty());
    }

    #[test]
    fn test_load_nonexistent_file() {
        let result = SimpleIndex::load(&std::path::PathBuf::from("nonexistent.bin"));
        assert!(result.is_err());
    }

    #[test]
    fn test_ann_index_trait() {
        let vectors = vec![vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0]];

        let mut index: Box<dyn AnnIndex> = Box::new(SimpleIndex::build(&vectors).unwrap());

        // Test search through trait
        let query = vec![1.0, 0.0, 0.0];
        let results = index.search(&query, 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 0);

        // Test add through trait
        index.add(99, &[0.0, 0.0, 1.0]);
        let results = index.search(&[0.0, 0.0, 1.0], 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 99);
    }
}
