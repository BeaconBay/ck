# SWE-bench Retrieval Benchmark for CK

This benchmark evaluates CK's code search and retrieval capabilities using the [SWE-bench](https://www.swebench.com/) dataset.

## Overview

**Goal**: Given a GitHub issue description, retrieve the relevant files that need to be modified to fix the issue.

**Dataset**: SWE-bench Lite (300 instances) - a curated subset of real-world GitHub issues from popular Python repositories.

**Evaluation Metrics**:
- **Recall@K**: Percentage of instances where all gold files appear in top K results
- **MRR (Mean Reciprocal Rank)**: Average of 1/rank for the first relevant file
- **Time**: Average search time per query

**Baseline**: BM25 lexical search (as used in SWE-bench paper)

**CK Methods Tested**:
- Pure semantic search (`--sem`)
- Pure regex search (`--regex`)
- Hybrid search (default)

## Setup

1. Install Python dependencies:
```bash
cd benchmarks/swe-bench
pip install -r requirements.txt
```

2. Build CK (from repository root):
```bash
cargo build --release
```

3. Download SWE-bench Lite dataset (happens automatically on first run)

## Usage

### Run Full Benchmark

```bash
python run.py
```

This will:
1. Download SWE-bench Lite dataset
2. Clone necessary repositories to `data/repos/`
3. Run CK search for each instance
4. Compare results against gold file lists
5. Generate summary in `results/summary.md`

### Run Subset

```bash
# Test on first 10 instances
python run.py --limit 10

# Test specific repository
python run.py --repo django/django
```

### Configuration

Edit `run.py` to configure:
- `TOP_K`: Number of results to retrieve (default: 10)
- `CK_BINARY`: Path to CK binary
- Dataset variant (Lite, Verified, Full)

## Dataset Structure

Each SWE-bench instance contains:
- `instance_id`: Unique identifier (e.g., "django__django-12345")
- `repo`: Repository name (e.g., "django/django")
- `problem_statement`: Issue description (used as search query)
- `patch`: Gold solution showing which files were modified

## Expected Results

Based on SWE-bench paper baseline:
- BM25 Recall@10: ~40-50%
- CK Hybrid should achieve: TBD (to be benchmarked)

## Output

Results are saved to:
- `results/detailed_results.json`: Per-instance results
- `results/summary.md`: Human-readable summary with metrics
- `results/timing.csv`: Performance timing data

## Limitations

- Dataset focused on Python repositories
- File retrieval only (not full patch generation)
- Requires repositories to be cloned locally

## References

- [SWE-bench Paper](https://arxiv.org/abs/2310.06770)
- [SWE-bench GitHub](https://github.com/SWE-bench/SWE-bench)
- [Dataset on HuggingFace](https://huggingface.co/datasets/princeton-nlp/SWE-bench_Lite)
