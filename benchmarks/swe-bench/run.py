#!/usr/bin/env python3
"""
SWE-bench Retrieval Benchmark for CK

Evaluates CK's code search performance using SWE-bench Lite dataset.
"""

import argparse
import json
import os
import subprocess
import time
from collections import defaultdict
from pathlib import Path
from typing import Dict, List, Set

from datasets import load_dataset
from git import Repo
from tqdm import tqdm


# Configuration
SCRIPT_DIR = Path(__file__).parent
DATA_DIR = SCRIPT_DIR / "data"
REPOS_DIR = DATA_DIR / "repos"
RESULTS_DIR = SCRIPT_DIR / "results"
CK_BINARY = SCRIPT_DIR.parent.parent / "target" / "release" / "ck"

# Ensure directories exist
DATA_DIR.mkdir(exist_ok=True)
REPOS_DIR.mkdir(exist_ok=True)
RESULTS_DIR.mkdir(exist_ok=True)


def load_swebench_lite():
    """Load SWE-bench Lite dataset from HuggingFace."""
    print("Loading SWE-bench Lite dataset...")
    dataset = load_dataset("princeton-nlp/SWE-bench_Lite", split="test")
    return dataset


def extract_gold_files(patch: str) -> Set[str]:
    """Extract list of modified files from a patch."""
    files = set()
    for line in patch.split('\n'):
        if line.startswith('diff --git'):
            # Format: diff --git a/path/file.py b/path/file.py
            parts = line.split()
            if len(parts) >= 3:
                file_path = parts[2][2:]  # Remove 'a/' prefix
                files.add(file_path)
    return files


def clone_or_update_repo(repo_name: str, commit_hash: str) -> Path:
    """Clone repository if needed and checkout specific commit."""
    repo_path = REPOS_DIR / repo_name.replace('/', '_')

    if not repo_path.exists():
        print(f"Cloning {repo_name}...")
        clone_url = f"https://github.com/{repo_name}.git"
        Repo.clone_from(clone_url, repo_path)

    repo = Repo(repo_path)

    # Fetch latest if needed
    if commit_hash not in [c.hexsha for c in repo.iter_commits()]:
        print(f"Fetching updates for {repo_name}...")
        repo.remotes.origin.fetch()

    # Checkout specific commit
    repo.git.checkout(commit_hash, force=True)

    return repo_path


def run_ck_search(query: str, repo_path: Path, method: str = "hybrid", top_k: int = 10) -> tuple[List[str], float]:
    """
    Run CK search and return list of file paths and search time.

    Args:
        query: Search query (issue description)
        repo_path: Path to repository
        method: Search method (semantic, regex, hybrid)
        top_k: Number of results to retrieve

    Returns:
        (list of file paths, search time in seconds)
    """
    if not CK_BINARY.exists():
        raise FileNotFoundError(
            f"CK binary not found at {CK_BINARY}. "
            "Please build CK first: cargo build --release"
        )

    # For semantic/hybrid search, ensure repository is indexed first
    if method in ["semantic", "hybrid"]:
        index_cmd = [str(CK_BINARY), "--index", str(repo_path), "-q"]
        try:
            subprocess.run(index_cmd, capture_output=True, timeout=300, check=True)
        except Exception as e:
            print(f"Warning: Failed to index {repo_path}: {e}")

    # Build CK command based on method
    cmd = [str(CK_BINARY)]

    if method == "semantic":
        cmd.extend(["--sem", query])
    elif method == "regex":
        cmd.extend(query)  # Default regex mode
    else:  # hybrid
        cmd.extend(["--hybrid", query])

    cmd.extend([
        str(repo_path),
        "--topk", str(top_k),
        "--jsonl",  # Use JSONL output for easier parsing
        "--no-snippet",  # Don't need code snippets
        "-q",  # Quiet mode
    ])

    start_time = time.time()

    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=60,
            cwd=repo_path
        )
        elapsed = time.time() - start_time

        # Parse JSONL output to extract file paths
        files = []
        for line in result.stdout.strip().split('\n'):
            if line:
                try:
                    data = json.loads(line)
                    file_path = data.get('file_path', '')
                    if file_path:
                        # Make path relative to repo
                        if file_path.startswith(str(repo_path)):
                            file_path = file_path[len(str(repo_path))+1:]
                        # Avoid duplicates
                        if file_path not in files:
                            files.append(file_path)
                except json.JSONDecodeError:
                    continue

        return files, elapsed

    except subprocess.TimeoutExpired:
        elapsed = time.time() - start_time
        print(f"Warning: CK search timed out after {elapsed:.2f}s")
        return [], elapsed
    except Exception as e:
        elapsed = time.time() - start_time
        print(f"Error running CK: {e}")
        return [], elapsed


def calculate_recall_at_k(predicted: List[str], gold: Set[str], k: int) -> float:
    """Calculate recall@k - are all gold files in top k predictions?"""
    if not gold:
        return 0.0

    predicted_set = set(predicted[:k])
    hits = len(gold.intersection(predicted_set))
    return hits / len(gold)


def calculate_mrr(predicted: List[str], gold: Set[str]) -> float:
    """Calculate Mean Reciprocal Rank - rank of first relevant file."""
    for i, pred_file in enumerate(predicted):
        if pred_file in gold:
            return 1.0 / (i + 1)
    return 0.0


def run_benchmark(
    limit: int = None,
    repo_filter: str = None,
    method: str = "hybrid",
    top_k: int = 10
):
    """Run full benchmark."""
    dataset = load_swebench_lite()

    # Convert to list for easier filtering/slicing
    dataset_list = list(dataset)

    # Filter dataset if requested
    if repo_filter:
        dataset_list = [item for item in dataset_list if item['repo'] == repo_filter]

    if limit:
        dataset_list = dataset_list[:limit]

    print(f"\nRunning benchmark on {len(dataset_list)} instances...")
    print(f"Method: {method}, Top-K: {top_k}\n")

    results = []
    metrics = {
        'recall@5': [],
        'recall@10': [],
        'mrr': [],
        'search_time': []
    }

    for instance in tqdm(dataset_list, desc="Processing instances"):
        instance_id = instance['instance_id']
        repo_name = instance['repo']
        problem_statement = instance['problem_statement']
        patch = instance['patch']
        base_commit = instance['base_commit']

        # Extract gold files from patch
        gold_files = extract_gold_files(patch)

        if not gold_files:
            print(f"Warning: No gold files found for {instance_id}")
            continue

        try:
            # Clone/update repository
            repo_path = clone_or_update_repo(repo_name, base_commit)

            # Run CK search
            predicted_files, search_time = run_ck_search(
                problem_statement,
                repo_path,
                method=method,
                top_k=top_k
            )

            # Calculate metrics
            recall_5 = calculate_recall_at_k(predicted_files, gold_files, 5)
            recall_10 = calculate_recall_at_k(predicted_files, gold_files, 10)
            mrr = calculate_mrr(predicted_files, gold_files)

            # Store results
            result = {
                'instance_id': instance_id,
                'repo': repo_name,
                'gold_files': list(gold_files),
                'predicted_files': predicted_files,
                'recall@5': recall_5,
                'recall@10': recall_10,
                'mrr': mrr,
                'search_time': search_time
            }
            results.append(result)

            # Update metrics
            metrics['recall@5'].append(recall_5)
            metrics['recall@10'].append(recall_10)
            metrics['mrr'].append(mrr)
            metrics['search_time'].append(search_time)

        except Exception as e:
            print(f"\nError processing {instance_id}: {e}")
            continue

    # Calculate summary statistics
    summary = {
        'method': method,
        'top_k': top_k,
        'total_instances': len(results),
        'avg_recall@5': sum(metrics['recall@5']) / len(metrics['recall@5']) if metrics['recall@5'] else 0,
        'avg_recall@10': sum(metrics['recall@10']) / len(metrics['recall@10']) if metrics['recall@10'] else 0,
        'avg_mrr': sum(metrics['mrr']) / len(metrics['mrr']) if metrics['mrr'] else 0,
        'avg_search_time': sum(metrics['search_time']) / len(metrics['search_time']) if metrics['search_time'] else 0,
    }

    # Save detailed results
    results_file = RESULTS_DIR / f"detailed_results_{method}.json"
    with open(results_file, 'w') as f:
        json.dump({
            'summary': summary,
            'results': results
        }, f, indent=2)

    print(f"\n{'='*60}")
    print(f"BENCHMARK RESULTS ({method})")
    print(f"{'='*60}")
    print(f"Total instances: {summary['total_instances']}")
    print(f"Average Recall@5: {summary['avg_recall@5']:.2%}")
    print(f"Average Recall@10: {summary['avg_recall@10']:.2%}")
    print(f"Average MRR: {summary['avg_mrr']:.3f}")
    print(f"Average search time: {summary['avg_search_time']:.2f}s")
    print(f"\nDetailed results saved to: {results_file}")

    # Generate markdown summary
    generate_summary_report(summary, results)

    return summary, results


def generate_summary_report(summary: Dict, results: List[Dict]):
    """Generate human-readable markdown summary."""
    summary_file = RESULTS_DIR / "summary.md"

    with open(summary_file, 'w') as f:
        f.write(f"# SWE-bench Retrieval Benchmark Results\n\n")
        f.write(f"**Date**: {time.strftime('%Y-%m-%d %H:%M:%S')}\n\n")
        f.write(f"**Method**: {summary['method']}\n\n")
        f.write(f"**Top-K**: {summary['top_k']}\n\n")

        f.write(f"## Summary Metrics\n\n")
        f.write(f"| Metric | Value |\n")
        f.write(f"|--------|-------|\n")
        f.write(f"| Total Instances | {summary['total_instances']} |\n")
        f.write(f"| Recall@5 | {summary['avg_recall@5']:.2%} |\n")
        f.write(f"| Recall@10 | {summary['avg_recall@10']:.2%} |\n")
        f.write(f"| MRR | {summary['avg_mrr']:.3f} |\n")
        f.write(f"| Avg Search Time | {summary['avg_search_time']:.2f}s |\n\n")

        f.write(f"## Top Performing Instances\n\n")
        # Sort by MRR
        top_results = sorted(results, key=lambda x: x['mrr'], reverse=True)[:10]
        for i, result in enumerate(top_results, 1):
            f.write(f"{i}. **{result['instance_id']}** - MRR: {result['mrr']:.3f}\n")

        f.write(f"\n## Worst Performing Instances\n\n")
        worst_results = sorted(results, key=lambda x: x['mrr'])[:10]
        for i, result in enumerate(worst_results, 1):
            f.write(f"{i}. **{result['instance_id']}** - MRR: {result['mrr']:.3f}\n")

    print(f"Summary report saved to: {summary_file}")


def main():
    parser = argparse.ArgumentParser(description="Run SWE-bench retrieval benchmark for CK")
    parser.add_argument("--limit", type=int, help="Limit number of instances to test")
    parser.add_argument("--repo", type=str, help="Filter by specific repository")
    parser.add_argument("--method", choices=["semantic", "regex", "hybrid"],
                       default="hybrid", help="Search method to use")
    parser.add_argument("--top-k", type=int, default=10, help="Number of results to retrieve")

    args = parser.parse_args()

    run_benchmark(
        limit=args.limit,
        repo_filter=args.repo,
        method=args.method,
        top_k=args.top_k
    )


if __name__ == "__main__":
    main()
