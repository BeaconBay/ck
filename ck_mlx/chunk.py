import os
from dataclasses import dataclass
from pathlib import Path
from typing import List, Optional
from tokenizers import Tokenizer
import tree_sitter_language_pack as tslp

@dataclass
class Chunk:
    content: str
    start_line: int
    end_line: int

EXTENSION_TO_LANGUAGE = {
    ".py": "python",
    ".js": "javascript",
    ".jsx": "javascript",
    ".ts": "typescript",
    ".tsx": "typescript",
    ".go": "go",
    ".rs": "rust",
    ".cpp": "cpp",
    ".hpp": "cpp",
    ".c": "c",
    ".h": "c",
    ".cc": "cpp",
    ".cs": "c_sharp",
    ".java": "java",
    ".rb": "ruby",
    ".php": "php",
    ".html": "html",
    ".css": "css",
    ".sh": "bash",
    ".yml": "yaml",
    ".yaml": "yaml",
    ".json": "json",
    ".md": "markdown",
}

def find_tokenizer_path() -> Optional[str]:
    env_path = os.environ.get("CK_TOKENIZER_PATH") or os.environ.get("OMLX_TOKENIZER_PATH")
    if env_path and os.path.exists(env_path):
        return env_path

    backend = os.environ.get(
        "CK_BACKEND", "api" if os.environ.get("OMLX_API_KEY") else "local"
    )
    if backend == "local":
        model_name = os.environ.get(
            "CK_LOCAL_MODEL", "mlx-community/bge-small-en-v1.5-6bit"
        )
        try:
            from huggingface_hub import try_to_load_from_cache

            cached_path = try_to_load_from_cache(model_name, "tokenizer.json")
        except (ImportError, OSError):
            return None
        return str(cached_path) if cached_path else None

    default_path = "/Users/matthewgold/.omlx/models/lexrivera/zembed-1-embedding-mlx-6Bit/tokenizer.json"
    if os.path.exists(default_path):
        return default_path

    model_name = os.environ.get("OMLX_MODEL", "zembed-1-embedding-mlx-6Bit")
    models_dir = Path("/Users/matthewgold/.omlx/models")
    if models_dir.exists():
        for p in models_dir.glob(f"**/{model_name}/tokenizer.json"):
            return str(p)

    return None

class Chunker:
    def __init__(self, tokenizer_path: Optional[str] = None):
        self.tokenizer = None
        t_path = tokenizer_path or find_tokenizer_path()
        if t_path:
            try:
                self.tokenizer = Tokenizer.from_file(t_path)
            except Exception as e:
                print(f"Warning: Failed to load tokenizer from {t_path}: {e}")
                
    def chunk_file(self, file_path: Path, chunk_size: int = 500, overlap: int = 75) -> List[Chunk]:
        """Chunk a file using tree-sitter or tokenizer/fixed-window fallback."""
        try:
            with open(file_path, "r", encoding="utf-8", errors="replace") as f:
                text = f.read()
        except Exception:
            return []
            
        if not text.strip():
            return []
            
        # Try tree-sitter chunking first
        ext = file_path.suffix.lower()
        lang = EXTENSION_TO_LANGUAGE.get(ext)
        if lang:
            try:
                # Estimate char_max_size as ~4 characters per token
                char_max_size = chunk_size * 4
                config = tslp.ProcessConfig(language=lang, chunk_max_size=char_max_size)
                res = tslp.process(text, config)
                if res.chunks:
                    chunks = []
                    for c in res.chunks:
                        chunks.append(Chunk(
                            content=c.content,
                            start_line=c.start_line + 1,
                            end_line=c.end_line + 1
                        ))
                    return chunks
            except Exception as e:
                # Fallback to standard token-based chunking on tree-sitter failures
                pass

        if self.tokenizer:
            return self._chunk_with_tokenizer(text, chunk_size, overlap)
        else:
            return self._chunk_fallback(text, chunk_size * 4, overlap * 4)
            
    def _chunk_with_tokenizer(self, text: str, chunk_size: int, overlap: int) -> List[Chunk]:
        try:
            encoding = self.tokenizer.encode(text)
        except Exception:
            return self._chunk_fallback(text, chunk_size * 4, overlap * 4)
            
        ids = encoding.ids
        offsets = encoding.offsets
        
        if not ids:
            return []
            
        chunks = []
        step = chunk_size - overlap
        if step <= 0:
            step = chunk_size
            
        i = 0
        total_tokens = len(ids)
        
        while i < total_tokens:
            j = min(i + chunk_size, total_tokens)
            
            chunk_start_char = offsets[i][0]
            chunk_end_char = offsets[j - 1][1]
            
            chunk_content = text[chunk_start_char:chunk_end_char]
            
            start_line = text[:chunk_start_char].count('\n') + 1
            end_line = text[:chunk_end_char].count('\n') + 1
            
            chunks.append(Chunk(
                content=chunk_content,
                start_line=start_line,
                end_line=end_line
            ))
            
            if j == total_tokens:
                break
            i += step
            
        return chunks
        
    def _chunk_fallback(self, text: str, char_chunk_size: int, char_overlap: int) -> List[Chunk]:
        """Simple line-aware character-based fallback chunker."""
        chunks = []
        lines = text.splitlines(keepends=True)
        
        current_chunk_lines = []
        current_char_count = 0
        start_line = 1
        
        for idx, line in enumerate(lines):
            current_chunk_lines.append(line)
            current_char_count += len(line)
            
            if current_char_count >= char_chunk_size:
                chunk_content = "".join(current_chunk_lines)
                end_line = start_line + len(current_chunk_lines) - 1
                chunks.append(Chunk(
                    content=chunk_content,
                    start_line=start_line,
                    end_line=end_line
                ))
                
                # Backtrack to handle overlap
                overlap_lines = []
                overlap_chars = 0
                # Look back to accumulate overlap
                for overlap_line in reversed(current_chunk_lines):
                    if overlap_chars + len(overlap_line) <= char_overlap:
                        overlap_lines.insert(0, overlap_line)
                        overlap_chars += len(overlap_line)
                    else:
                        break
                        
                start_line = end_line - len(overlap_lines) + 1
                current_chunk_lines = overlap_lines
                current_char_count = overlap_chars
                
        if current_chunk_lines:
            chunk_content = "".join(current_chunk_lines)
            end_line = start_line + len(current_chunk_lines) - 1
            chunks.append(Chunk(
                content=chunk_content,
                start_line=start_line,
                end_line=end_line
            ))
            
        return chunks

if __name__ == "__main__":
    import sys
    file_to_chunk = sys.argv[1] if len(sys.argv) > 1 else __file__
    print(f"Chunking file: {file_to_chunk}")
    chunker = Chunker()
    res = chunker.chunk_file(Path(file_to_chunk), chunk_size=20, overlap=5)
    for idx, c in enumerate(res[:5]):
        print(f"Chunk {idx+1} (lines {c.start_line}-{c.end_line}):")
        print(repr(c.content))
        print("-" * 20)
    print(f"Total chunks: {len(res)}")
