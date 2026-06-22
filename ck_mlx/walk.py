import os
from pathlib import Path
from typing import Iterator, List
import pathspec

DEFAULT_IGNORES = [
    # VCS / Dependency / Virtualenv
    "node_modules/", "venv/", ".venv/", ".git/", "__pycache__/", "*.pyc", 
    ".pytest_cache/", ".mypy_cache/", ".ruff_cache/", "dist/", "build/", "*.egg-info/",
    ".next/", ".turbo/", "coverage/",
    # Lock files
    "*.lock", "*.lock.*", "package-lock.json", "yarn.lock", "bun.lockb",
    # Secrets
    ".env", ".env.*", "*.pem", "*.key", "secrets/",
    # Build/dist
    "web_dist/", "tui_dist/", "website/", "web/", "*.min.js", "*.min.css",
    ".ck/", ".ck-search/", ".ck-mlx/", ".omg/",
    # Large non-code dirs
    "locales/", ".serena/", ".github/", "docs/",
    # Documents
    "*.pdf", "*.docx", "*.doc",
    # Images
    "*.png", "*.jpg", "*.jpeg", "*.gif", "*.bmp", "*.svg", "*.ico", "*.webp", "*.tiff",
    # Video & Audio
    "*.mp4", "*.avi", "*.mov", "*.mkv", "*.wmv", "*.flv", "*.webm",
    "*.mp3", "*.wav", "*.flac", "*.aac", "*.ogg", "*.m4a",
    # Binary/Compiled
    "*.exe", "*.dll", "*.so", "*.dylib", "*.a", "*.lib", "*.obj", "*.o",
    # Archives
    "*.zip", "*.tar", "*.tar.gz", "*.tgz", "*.rar", "*.7z", "*.bz2", "*.gz",
    # Data files
    "*.db", "*.sqlite", "*.sqlite3", "*.parquet", "*.arrow",
    # Other noise
    "*.schema.json", "openapi.json", "openapi.yaml",
]

def load_ignore_patterns(root_dir: Path) -> List[str]:
    patterns = list(DEFAULT_IGNORES)
    
    # Load root .gitignore
    gitignore_path = root_dir / ".gitignore"
    if gitignore_path.exists():
        try:
            with open(gitignore_path, "r", encoding="utf-8", errors="ignore") as f:
                # Filter out comments and empty lines
                lines = [line.strip() for line in f if line.strip() and not line.strip().startswith("#")]
                patterns.extend(lines)
        except Exception:
            pass
            
    # Load root .ckignore
    ckignore_path = root_dir / ".ckignore"
    if ckignore_path.exists():
        try:
            with open(ckignore_path, "r", encoding="utf-8", errors="ignore") as f:
                lines = [line.strip() for line in f if line.strip() and not line.strip().startswith("#")]
                patterns.extend(lines)
        except Exception:
            pass
            
    return patterns

def walk_files(root_dir: str) -> Iterator[Path]:
    """Walk the root_dir recursively yielding Path objects for non-ignored files."""
    root_path = Path(root_dir).resolve()
    patterns = load_ignore_patterns(root_path)
    spec = pathspec.PathSpec.from_lines("gitwildmatch", patterns)
    
    for dirpath, dirnames, filenames in os.walk(root_path):
        dir_p = Path(dirpath)
        
        # Filter directories in-place to avoid walking into them
        filtered_dirs = []
        for d in dirnames:
            full_d_path = dir_p / d
            try:
                rel_d_path = full_d_path.relative_to(root_path)
                rel_str = str(rel_d_path) + "/"
                if not spec.match_file(rel_str):
                    filtered_dirs.append(d)
            except ValueError:
                # Path mismatch, ignore/skip
                pass
        dirnames[:] = filtered_dirs
        
        for f in filenames:
            full_f_path = dir_p / f
            try:
                rel_f_path = full_f_path.relative_to(root_path)
                rel_str = str(rel_f_path)
                if not spec.match_file(rel_str):
                    yield full_f_path
            except ValueError:
                pass

if __name__ == "__main__":
    import sys
    root = sys.argv[1] if len(sys.argv) > 1 else "."
    print(f"Walking files in: {root}")
    count = 0
    for path in walk_files(root):
        count += 1
        if count <= 10:
            print(f" - {path}")
    print(f"Total files: {count}")
