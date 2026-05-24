#!/usr/bin/env python3
"""
Benchmark `ck --sem` against the semble-dataset annotations.

For each repo:
  1. Run `ck --index` (cold-start, timed).
  2. For each query, run `ck --sem --topk 10 --json --threshold 0 <query> <dir>`
     three times; take median latency.
  3. Compute file-level NDCG@10 against the annotated relevant + secondary
     file paths (same metric semble used for ripgrep/probe/grepai baselines).

Outputs benchmarks/results/ck-<sha12>.json with per-repo and per-language
aggregates.

Usage:
  python3 benchmarks/baselines/ck.py --repo curl --repo fastapi  # subset
  python3 benchmarks/baselines/ck.py --language python           # one language
  python3 benchmarks/baselines/ck.py                             # full sweep
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import shutil
import subprocess
import sys
import time
from collections import defaultdict
from dataclasses import asdict, dataclass, field
from pathlib import Path
from statistics import median

BENCH_ROOT = Path.home() / ".cache" / "ck-bench"
BENCHMARKS_DIR = Path(__file__).parent.parent
RESULTS_DIR = BENCHMARKS_DIR / "results"
REPOS_PATH = BENCHMARKS_DIR / "repos.json"
ANNOTATIONS_DIR = BENCHMARKS_DIR / "annotations"

_TOP_K = 10
_LATENCY_RUNS = 3
_QUERY_TIMEOUT_SEC = 60


# --------- data ---------

def _normalize_target(t) -> str:
    """Annotations are either bare path strings or {path, start_line, end_line}
    objects. We do file-level NDCG here so we only need the path."""
    if isinstance(t, str):
        return t
    if isinstance(t, dict) and "path" in t:
        return t["path"]
    raise ValueError(f"unrecognized annotation target: {t!r}")


@dataclass(frozen=True)
class Task:
    repo: str
    language: str
    query: str
    relevant: tuple[str, ...]    # primary relevant file paths (repo-relative)
    secondary: tuple[str, ...]   # secondary relevant file paths
    category: str

    @property
    def all_relevant(self) -> tuple[str, ...]:
        # Dedup while preserving order.
        seen = set()
        out = []
        for p in self.relevant + self.secondary:
            if p not in seen:
                seen.add(p)
                out.append(p)
        return tuple(out)


def load_tasks(repo: dict) -> list[Task]:
    ann_path = ANNOTATIONS_DIR / f"{repo['name']}.json"
    if not ann_path.exists():
        return []
    annotations = json.loads(ann_path.read_text())
    return [
        Task(
            repo=repo["name"],
            language=repo["language"],
            query=a["query"],
            relevant=tuple(_normalize_target(t) for t in (a.get("relevant") or [])),
            secondary=tuple(_normalize_target(t) for t in (a.get("secondary") or [])),
            category=a.get("category", "unknown"),
        )
        for a in annotations
    ]


# --------- metric ---------

def _dcg(rels: list[int]) -> float:
    return sum(r / math.log2(i + 2) for i, r in enumerate(rels))


def ndcg_at_k(relevant_ranks: list[int], n_relevant: int, k: int) -> float:
    if n_relevant == 0:
        return 0.0
    rels = [0] * k
    for r in relevant_ranks:
        if 1 <= r <= k:
            rels[r - 1] = 1
    ideal = _dcg([1] * min(k, n_relevant))
    return _dcg(rels) / ideal if ideal > 0 else 0.0


def file_rank(returned_paths: list[str], target: str) -> int | None:
    """Return 1-based rank of first returned path matching `target`, else None."""
    target_n = target.lstrip("./")
    for i, p in enumerate(returned_paths, 1):
        pn = p.lstrip("./")
        if pn == target_n or pn.endswith("/" + target_n) or target_n.endswith("/" + pn):
            return i
    return None


# --------- ck driver ---------

def _strip_banner(stdout: str) -> list[str]:
    """ck prints stderr-ish banner lines to stdout before JSONL results.
    Filter to lines that start with `{`."""
    return [ln for ln in stdout.splitlines() if ln.startswith("{")]


def ck_version() -> str:
    out = subprocess.run(["ck", "--version"], capture_output=True, text=True, check=True)
    return out.stdout.strip()


def ck_index(repo_dir: Path) -> float:
    """Cold-start index. Returns elapsed seconds.
    NOTE: ck treats positional args as PATTERN; the target dir must be cwd.
    """
    # Remove any existing .ck/ to get a true cold start.
    ck_dir = repo_dir / ".ck"
    if ck_dir.exists():
        shutil.rmtree(ck_dir)
    started = time.perf_counter()
    subprocess.run(
        ["ck", "--index", "."],
        cwd=repo_dir,
        capture_output=True,
        text=True,
        check=True,
        timeout=1800,
    )
    return time.perf_counter() - started


def ck_search_paths(query: str, repo_dir: Path, *, top_k: int = _TOP_K) -> tuple[list[str], float]:
    """Run one `ck --sem`, returning (deduped file paths in rank order, latency ms).
    NOTE: ck treats positional after `--` as PATTERN/PATH; we cwd into repo_dir
    and pass `.` as the path so PATTERN is unambiguously the query.
    """
    started = time.perf_counter()
    proc = subprocess.run(
        [
            "ck", "--sem",
            "--topk", str(top_k * 3),  # over-fetch — ck returns chunk-level, dedup to file
            "--threshold", "0",
            "--json",
            "--no-snippet",
            "--", query, ".",
        ],
        cwd=repo_dir,
        capture_output=True,
        text=True,
        timeout=_QUERY_TIMEOUT_SEC,
    )
    latency_ms = (time.perf_counter() - started) * 1000.0
    if proc.returncode != 0:
        return [], latency_ms

    seen: dict[str, None] = {}
    for line in _strip_banner(proc.stdout):
        try:
            obj = json.loads(line)
        except json.JSONDecodeError:
            continue
        path = obj.get("file") or obj.get("path")
        if not path:
            continue
        # Normalize to repo-relative path.
        try:
            rel = str(Path(path).resolve().relative_to(repo_dir.resolve()))
        except ValueError:
            rel = path
        if rel not in seen:
            seen[rel] = None
        if len(seen) >= top_k:
            break
    return list(seen.keys()), latency_ms


# --------- run ---------

@dataclass
class RepoResult:
    repo: str
    language: str
    chunks: int = 0      # not currently extracted; placeholder for parity
    ndcg10: float = 0.0
    p50_ms: float = 0.0
    index_s: float = 0.0
    queries: int = 0
    by_category: dict[str, float] = field(default_factory=dict)


def evaluate_repo(repo: dict, *, verbose: bool = False) -> RepoResult | None:
    tasks = load_tasks(repo)
    if not tasks:
        print(f"  no annotations for {repo['name']}, skipping", file=sys.stderr)
        return None

    repo_dir = BENCH_ROOT / repo["name"]
    if not repo_dir.exists():
        print(f"  {repo['name']}: not synced — run sync_repos.py first", file=sys.stderr)
        return None
    bench_dir = repo_dir if not repo.get("benchmark_root") else repo_dir / repo["benchmark_root"]

    print(f"[{repo['language']:>10}] {repo['name']}  ({len(tasks)} queries)")

    try:
        idx_s = ck_index(bench_dir)
        print(f"    indexed in {idx_s:.1f}s")
    except subprocess.CalledProcessError as e:
        print(f"    INDEX FAILED: {e.stderr[:200]}", file=sys.stderr)
        return None
    except subprocess.TimeoutExpired:
        print(f"    INDEX TIMED OUT after 1800s", file=sys.stderr)
        return None

    ndcg_sum = 0.0
    latencies: list[float] = []
    cat: dict[str, list[float]] = defaultdict(list)

    for task in tasks:
        run_latencies: list[float] = []
        paths: list[str] = []
        for _ in range(_LATENCY_RUNS):
            paths, lat = ck_search_paths(task.query, bench_dir)
            run_latencies.append(lat)
        latencies.append(median(run_latencies))

        ranks = [r for tgt in task.all_relevant if (r := file_rank(paths, tgt)) is not None]
        q_ndcg = ndcg_at_k(ranks, len(task.all_relevant), _TOP_K)
        ndcg_sum += q_ndcg
        cat[task.category].append(q_ndcg)
        if verbose:
            print(f"      ndcg@10={q_ndcg:.3f}  ranks={ranks}  n_rel={len(task.all_relevant)}  q={task.query!r}")

    mean_ndcg = ndcg_sum / len(tasks) if tasks else 0.0
    p50 = sorted(latencies)[len(latencies) // 2] if latencies else 0.0
    by_cat = {k: sum(v) / len(v) for k, v in cat.items()}

    print(f"    NDCG@10={mean_ndcg:.3f}   p50={p50:.0f}ms")
    return RepoResult(
        repo=repo["name"],
        language=repo["language"],
        ndcg10=mean_ndcg,
        p50_ms=p50,
        index_s=idx_s,
        queries=len(tasks),
        by_category=by_cat,
    )


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--repo", action="append", default=[])
    ap.add_argument("--language", action="append", default=[])
    ap.add_argument("--verbose", "-v", action="store_true")
    ap.add_argument("--output", help="Output JSON path (default: results/ck-<sha12>.json)")
    args = ap.parse_args()

    if not shutil.which("ck"):
        print("Error: `ck` not on $PATH. Install via `cargo install ck-search --locked`.", file=sys.stderr)
        return 1

    ver = ck_version()
    print(f"=== ck benchmark — {ver} ===")
    print(f"BENCH_ROOT: {BENCH_ROOT}")

    repos = json.loads(REPOS_PATH.read_text())
    if args.repo:
        repos = [r for r in repos if r["name"] in args.repo]
    if args.language:
        repos = [r for r in repos if r["language"] in args.language]
    if not repos:
        print("No repos matched filters.", file=sys.stderr)
        return 1
    print(f"Evaluating {len(repos)} repo(s)\n")

    results: list[RepoResult] = []
    for repo in repos:
        r = evaluate_repo(repo, verbose=args.verbose)
        if r:
            results.append(r)
        print()

    if not results:
        print("No successful repo runs.", file=sys.stderr)
        return 1

    # Aggregates
    total_queries = sum(r.queries for r in results)
    mean_ndcg = sum(r.ndcg10 * r.queries for r in results) / max(1, total_queries)
    by_lang: dict[str, list[RepoResult]] = defaultdict(list)
    for r in results:
        by_lang[r.language].append(r)

    print("=== summary ===")
    print(f"{'language':<12}  {'repos':>5}  {'ndcg@10':>8}  {'p50 ms':>8}  {'index s':>8}")
    for lang in sorted(by_lang):
        rs = by_lang[lang]
        qs = sum(r.queries for r in rs)
        ndcg = sum(r.ndcg10 * r.queries for r in rs) / max(1, qs)
        p50 = median([r.p50_ms for r in rs])
        idx = median([r.index_s for r in rs])
        print(f"{lang:<12}  {len(rs):>5}  {ndcg:>8.3f}  {p50:>8.0f}  {idx:>8.1f}")
    print(f"{'OVERALL':<12}  {len(results):>5}  {mean_ndcg:>8.3f}  {median([r.p50_ms for r in results]):>8.0f}  {median([r.index_s for r in results]):>8.1f}")
    print(f"(over {total_queries} queries)")

    # Persist
    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    sha = hashlib.sha1(REPOS_PATH.read_bytes()).hexdigest()[:12]
    out_path = Path(args.output) if args.output else RESULTS_DIR / f"ck-{sha}.json"
    payload = {
        "tool": "ck",
        "version": ver,
        "top_k": _TOP_K,
        "latency_runs": _LATENCY_RUNS,
        "total_queries": total_queries,
        "overall_ndcg10": mean_ndcg,
        "per_repo": [asdict(r) for r in results],
    }
    out_path.write_text(json.dumps(payload, indent=2))
    print(f"\nWrote {out_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
