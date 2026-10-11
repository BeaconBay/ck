#!/usr/bin/env bash
set -euo pipefail

binary="${1:?missing release binary path}"
"$binary" --version

fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
mkdir "$fixture/source"
source_file="$fixture/source/input.rs"
printf 'fn main() { println!("release regression needle"); }\n' > "$source_file"

"$binary" --regex needle "$source_file" --json | tee "$fixture/regex.jsonl"
"$binary" --index "$fixture/source"
"$binary" --sem 'release regression' "$source_file" --threshold 0 --json \
  | tee "$fixture/semantic.jsonl"

# JSON avoids terminal highlighting and lets us reject dummy zero-score
# embeddings as well as missing or incorrectly scoped results.
python3 - "$source_file" "$fixture/regex.jsonl" "$fixture/semantic.jsonl" <<'PY'
import json
import math
import sys
from pathlib import Path

expected_file = Path(sys.argv[1]).resolve()
for mode, output in zip(("regex", "semantic"), sys.argv[2:]):
    results = [json.loads(line) for line in Path(output).read_text().splitlines()]
    assert results, f"{mode}: no results"
    for result in results:
        assert Path(result["file"]).resolve() == expected_file, result
        assert "release regression needle" in result["preview"], result
        if mode == "semantic":
            score = result["score"]
            assert math.isfinite(score) and 0 < score <= 1, result
print("Release binary passed regex, indexing, and semantic regressions")
PY
