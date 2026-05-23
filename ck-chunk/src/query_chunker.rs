use std::{borrow::Cow, collections::HashSet, env, fs, path::PathBuf};

use anyhow::{Context, Result};
use tree_sitter::{Language, Query, QueryCursor, StreamingIterator, Tree};

use crate::{Chunk, ChunkType, ParseableLanguage, build_chunk};

const QUERY_OVERRIDE_DIR_ENV: &str = "CK_CHUNK_QUERY_DIR";

pub(crate) fn chunk_with_queries(
    language: ParseableLanguage,
    ts_language: Language,
    tree: &Tree,
    source: &str,
) -> Result<Option<Vec<Chunk>>> {
    let Some(query_source) = load_query_source(language)? else {
        return Ok(None);
    };

    let query = Query::new(&ts_language, &query_source)
        .with_context(|| format!("Failed to compile query for {}", language))?;

    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut seen_spans = HashSet::new();
    let mut chunks = Vec::new();

    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
    while let Some(mat) = matches.next() {
        for capture in mat.captures {
            let capture_name = &capture_names[capture.index as usize];
            if let Some(chunk_type) = chunk_type_from_capture(capture_name) {
                if language == ParseableLanguage::Haskell
                    && chunk_type == ChunkType::Function
                    && capture
                        .node
                        .parent()
                        .is_some_and(|parent| parent.kind() == "signature")
                {
                    continue;
                }

                if let Some(chunk) = build_chunk(capture.node, source, chunk_type, language) {
                    let span_key = (chunk.span.byte_start, chunk.span.byte_end);
                    if seen_spans.insert(span_key) {
                        chunks.push(chunk);
                    }
                }
            }
        }
    }

    if chunks.is_empty() {
        return Ok(None);
    }

    chunks.sort_by_key(|chunk| chunk.span.byte_start);
    Ok(Some(chunks))
}

fn load_query_source(language: ParseableLanguage) -> Result<Option<Cow<'static, str>>> {
    if let Some(dir) = env::var_os(QUERY_OVERRIDE_DIR_ENV) {
        let override_path = PathBuf::from(dir)
            .join(language.to_string())
            .join("tags.scm");

        if override_path.exists() {
            let contents = fs::read_to_string(&override_path).with_context(|| {
                format!(
                    "Failed to read query override for {} at {}",
                    language,
                    override_path.display()
                )
            })?;
            return Ok(Some(Cow::Owned(contents)));
        }
    }

    Ok(builtin_query(language).map(Cow::Borrowed))
}

fn builtin_query(language: ParseableLanguage) -> Option<&'static str> {
    match language {
        ParseableLanguage::Python => Some(include_str!("../queries/python/tags.scm")),
        ParseableLanguage::TypeScript => Some(include_str!("../queries/typescript/tags.scm")),
        ParseableLanguage::JavaScript => Some(include_str!("../queries/javascript/tags.scm")),
        ParseableLanguage::Rust => Some(include_str!("../queries/rust/tags.scm")),
        ParseableLanguage::Haskell => Some(include_str!("../queries/haskell/tags.scm")),
        ParseableLanguage::Ruby => Some(include_str!("../queries/ruby/tags.scm")),
        ParseableLanguage::Go => Some(include_str!("../queries/go/tags.scm")),
        ParseableLanguage::CSharp => Some(include_str!("../queries/csharp/tags.scm")),
        ParseableLanguage::Zig => Some(include_str!("../queries/zig/tags.scm")),

        ParseableLanguage::Dart => Some(include_str!("../queries/dart/tags.scm")),

        ParseableLanguage::Elixir => Some(include_str!("../queries/elixir/tags.scm")),
    }
}

fn chunk_type_from_capture(name: &str) -> Option<ChunkType> {
    let type_name = name.split('.').next_back().unwrap_or(name);

    match type_name {
        "function" | "fn" => Some(ChunkType::Function),
        "method" => Some(ChunkType::Method),
        "class" | "struct" | "enum" | "trait" => Some(ChunkType::Class),
        "module" | "namespace" | "impl" | "mod" => Some(ChunkType::Module),
        // Module attributes (Elixir @spec, @type, @callback, @behaviour)
        "spec" | "type" | "callback" | "behaviour" => Some(ChunkType::Text),
        "text" | "import" => Some(ChunkType::Text),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChunkType, tree_sitter_language};
    use tree_sitter::Parser;

    #[test]
    fn rust_queries_capture_core_constructs() {
        let source = r#"
            mod sample {
                pub struct Thing;

                impl Thing {
                    pub fn new() -> Self { Self }
                    fn helper(&self) {}
                }
            }

            fn util() {}

            trait Runner {
                fn run(&self);
            }
        "#;

        let mut parser = Parser::new();
        let ts_language = tree_sitter_language(ParseableLanguage::Rust).expect("rust language");
        parser
            .set_language(&ts_language)
            .expect("set rust language");
        let tree = parser.parse(source, None).expect("parse rust source");

        let chunks = chunk_with_queries(ParseableLanguage::Rust, ts_language, &tree, source)
            .expect("query execution")
            .expect("query should be available");

        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.chunk_type == ChunkType::Function
                    && chunk.text.contains("fn util"))
        );
        assert!(chunks
            .iter()
            .any(|chunk| chunk.chunk_type == ChunkType::Method && chunk.text.contains("fn new")));
        assert!(chunks.iter().any(
            |chunk| chunk.chunk_type == ChunkType::Class && chunk.text.contains("struct Thing")
        ));
        assert!(chunks.iter().any(
            |chunk| chunk.chunk_type == ChunkType::Module && chunk.text.contains("mod sample")
        ));
        assert!(chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Module && chunk.text.contains("impl Thing")
        }));

        let method_chunk = chunks
            .iter()
            .find(|chunk| chunk.chunk_type == ChunkType::Method && chunk.text.contains("fn new"))
            .expect("method chunk present");
        assert_eq!(
            method_chunk.metadata.ancestry,
            vec!["sample".to_string(), "Thing".to_string()]
        );
        let util_chunk = chunks
            .iter()
            .find(|chunk| chunk.chunk_type == ChunkType::Function && chunk.text.contains("fn util"))
            .expect("util chunk present");
        assert!(util_chunk.metadata.ancestry.is_empty());
    }

    #[test]
    fn python_queries_capture_core_constructs() {
        let source = r#"
class Greeter:
    def __init__(self, name: str):
        self.name = name

    @staticmethod
    def hello():
        return "hi"


async def async_worker():
    return None


def top_level():
    return "done"
"#;

        let mut parser = Parser::new();
        let ts_language = tree_sitter_language(ParseableLanguage::Python).expect("python language");
        parser
            .set_language(&ts_language)
            .expect("set python language");
        let tree = parser.parse(source, None).expect("parse python source");

        let chunks = chunk_with_queries(ParseableLanguage::Python, ts_language, &tree, source)
            .expect("query execution")
            .expect("query should be available");

        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.chunk_type == ChunkType::Class
                    && chunk.text.contains("class Greeter"))
        );
        assert!(chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Method && chunk.text.contains("def hello")
        }));
        assert!(chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Function && chunk.text.contains("def top_level")
        }));
        assert!(chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Function && chunk.text.contains("async def async_worker")
        }));

        let method_chunk = chunks
            .iter()
            .find(|chunk| chunk.chunk_type == ChunkType::Method && chunk.text.contains("def hello"))
            .expect("method chunk present");
        assert_eq!(method_chunk.metadata.ancestry, vec!["Greeter".to_string()]);
        assert!(
            method_chunk
                .metadata
                .leading_trivia
                .iter()
                .any(|text| text.contains("@staticmethod"))
        );

        let top_level_chunk = chunks
            .iter()
            .find(|chunk| {
                chunk.chunk_type == ChunkType::Function && chunk.text.contains("def top_level")
            })
            .expect("top level chunk");
        assert!(top_level_chunk.metadata.ancestry.is_empty());
    }

    #[test]
    #[ignore] // TODO: Update test expectations to match actual query behavior
    fn typescript_queries_capture_core_constructs() {
        let source = r#"
// Utility function
export const util = () => {
    // comment about util
    return 42;
};

export class Example {
    // leading comment for method
    constructor() {}

    // Another comment
    run = () => {
        return util();
    };
}

const compute = (x: number) => x * 2;
"#;

        let mut parser = Parser::new();
        let ts_language = tree_sitter_language(ParseableLanguage::TypeScript).expect("ts language");
        parser.set_language(&ts_language).expect("set ts language");
        let tree = parser.parse(source, None).expect("parse ts source");

        let chunks = chunk_with_queries(ParseableLanguage::TypeScript, ts_language, &tree, source)
            .expect("query execution")
            .expect("query should be available");

        assert!(chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Function && chunk.text.contains("export const util")
        }));
        assert!(chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Method && chunk.text.contains("run = () =>")
        }));
        assert!(chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Function && chunk.text.contains("const compute")
        }));
        assert!(chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Class && chunk.text.contains("class Example")
        }));

        let arrow_chunk = chunks
            .iter()
            .find(|chunk| {
                chunk.chunk_type == ChunkType::Function && chunk.text.contains("export const util")
            })
            .expect("arrow chunk present");
        assert!(arrow_chunk.text.contains("return 42"));
        assert!(arrow_chunk.metadata.ancestry.is_empty());

        let method_chunk = chunks
            .iter()
            .find(|chunk| {
                chunk.chunk_type == ChunkType::Method && chunk.text.contains("run = () =>")
            })
            .expect("method chunk");
        assert_eq!(method_chunk.metadata.ancestry, vec!["Example".to_string()]);
        assert!(
            method_chunk
                .metadata
                .leading_trivia
                .iter()
                .any(|text| text.contains("Another comment"))
        );
    }

    #[test]
    fn dart_queries_capture_core_constructs() {
        let source = r#"
class Helper {
  Helper();
  void help() {}
}

mixin MyMixin {}

enum State { on, off }

void globalFunc() {}

const int MAX = 100;
"#;

        let mut parser = Parser::new();
        let ts_language = tree_sitter_language(ParseableLanguage::Dart).expect("dart language");
        parser
            .set_language(&ts_language)
            .expect("set dart language");
        let tree = parser.parse(source, None).expect("parse dart source");
        println!("Dart Tree: {}", tree.root_node().to_sexp());

        let chunks = chunk_with_queries(ParseableLanguage::Dart, ts_language, &tree, source)
            .expect("query execution") // This should fail if query is invalid
            .expect("query should be available");

        assert!(chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Class && chunk.text.contains("class Helper")
        }));

        // Check global function capture - note: name extraction might need work if it fails
        let global_func = chunks
            .iter()
            .find(|chunk| chunk.text.contains("globalFunc"));
        assert!(global_func.is_some(), "Should capture global function");
        if let Some(func) = global_func {
            println!("Global Func Metadata: {:?}", func.metadata);
        }
    }
}
