# CK Benchmarks

This directory contains benchmarks for evaluating CK's search performance against industry standards.

## Available Benchmarks

### SWE-bench

Evaluates CK's code search and retrieval capabilities using real-world GitHub issues from the [SWE-bench](https://www.swebench.com/) dataset.

- **Dataset**: 2,294 real GitHub issues from popular Python repositories
- **Task**: Given an issue description, retrieve relevant files that need to be modified
- **Baseline**: BM25 retrieval (as used in SWE-bench evaluations)
- **CK Advantage**: Tests hybrid semantic + lexical search vs pure lexical search

See [`swe-bench/README.md`](./swe-bench/README.md) for detailed setup and usage instructions.

## Running Benchmarks

Each benchmark has its own directory with:
- `README.md` - Detailed documentation
- `run.py` - Main benchmark runner script
- `requirements.txt` - Python dependencies
- `data/` - Downloaded benchmark data (gitignored)
- `results/` - Benchmark results

## Results

Benchmark results and performance comparisons are documented in each benchmark's directory.

## Contributing

To add a new benchmark:

1. Create a new directory: `benchmarks/<benchmark-name>/`
2. Add README, run script, and requirements
3. Update this main README with a description
4. Ensure large data files are gitignored
