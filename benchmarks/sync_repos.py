#!/usr/bin/env python3
"""
Clone (or update) the 63 benchmark repos into ~/.cache/ck-bench/<name>,
pinned to the revisions in repos.json.

Adapted from semble's sync_repos.py — same shape, BENCH_ROOT relocated
to ~/.cache/ck-bench so the two harnesses don't fight over checkouts.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

BENCH_ROOT = Path.home() / ".cache" / "ck-bench"
REPOS_PATH = Path(__file__).parent / "repos.json"


def load_repos() -> list[dict]:
    return json.loads(REPOS_PATH.read_text())


def _run(cmd: list[str], cwd: Path | None = None, check: bool = True) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, check=check)


def sync_repo(repo: dict, *, check_only: bool = False) -> tuple[str, str]:
    """Returns (name, status) — status in {'ok', 'cloned', 'updated', 'missing', 'mismatched'}."""
    name = repo["name"]
    url = repo["url"]
    revision = repo["revision"]
    target = BENCH_ROOT / name

    if not target.exists():
        if check_only:
            return name, "missing"
        BENCH_ROOT.mkdir(parents=True, exist_ok=True)
        _run(["git", "clone", "--filter=blob:none", "--no-checkout", url, str(target)])
        _run(["git", "checkout", revision], cwd=target)
        return name, "cloned"

    actual = _run(["git", "rev-parse", "HEAD"], cwd=target).stdout.strip()
    if actual == revision:
        return name, "ok"

    if check_only:
        return name, "mismatched"

    _run(["git", "fetch", "origin"], cwd=target, check=False)
    _run(["git", "checkout", revision], cwd=target)
    return name, "updated"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true", help="Verify only; don't clone or update.")
    ap.add_argument("--repo", action="append", default=[], help="Restrict to this repo name (repeatable).")
    ap.add_argument("--language", action="append", default=[], help="Restrict to this language (repeatable).")
    args = ap.parse_args()

    repos = load_repos()
    if args.repo:
        repos = [r for r in repos if r["name"] in args.repo]
    if args.language:
        repos = [r for r in repos if r["language"] in args.language]

    if not repos:
        print("No repos matched filters.", file=sys.stderr)
        return 1

    print(f"BENCH_ROOT: {BENCH_ROOT}")
    print(f"Syncing {len(repos)} repo(s){' (check-only)' if args.check else ''}...")
    for repo in repos:
        try:
            name, status = sync_repo(repo, check_only=args.check)
            print(f"  {status:>11}  {name}")
        except subprocess.CalledProcessError as e:
            print(f"  {'failed':>11}  {repo['name']}: {e.stderr.strip()}", file=sys.stderr)
            return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
