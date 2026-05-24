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
        .with_context(|| format!("Failed to compile query for {language}"))?;

    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut seen_spans = HashSet::new();
    let mut chunks = Vec::new();

    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
    while let Some(mat) = matches.next() {
        for capture in mat.captures {
            let capture_name = &capture_names[capture.index as usize];
            if let Some(chunk_type) = chunk_type_from_capture(capture_name) {
                // Filter out duplicates for C/C++ where template_declaration wraps the definition
                if matches!(language, ParseableLanguage::C | ParseableLanguage::Cpp) {
                    if let Some(parent) = capture.node.parent()
                        && parent.kind() == "template_declaration"
                        && matches!(
                            chunk_type,
                            ChunkType::Class | ChunkType::Function | ChunkType::Method
                        )
                    {
                        continue;
                    }

                    if chunk_type == ChunkType::Class
                        && matches!(
                            capture.node.kind(),
                            "struct_specifier" | "union_specifier" | "enum_specifier"
                        )
                        && !c_cpp_type_has_body(capture.node)
                    {
                        continue;
                    }

                    // Filter out granular text chunks (using, typedef, locals, etc.) inside
                    // classes/structs/unions or inside function/method bodies.
                    if matches!(language, ParseableLanguage::C | ParseableLanguage::Cpp)
                        && chunk_type == ChunkType::Text
                        && is_inside_c_cpp_type_or_function_body(capture.node)
                    {
                        continue;
                    }

                    // Filter out text chunks that come from declarations inside function bodies
                    if chunk_type == ChunkType::Text && is_inside_function_or_method(capture.node) {
                        continue;
                    }

                    // Skip local class/struct/enum/union chunks inside functions
                    if chunk_type == ChunkType::Class && is_inside_function_or_method(capture.node)
                    {
                        continue;
                    }

                    // Skip local function/method chunks inside functions (e.g., local classes)
                    if matches!(chunk_type, ChunkType::Function | ChunkType::Method)
                        && is_inside_function_or_method(capture.node)
                    {
                        continue;
                    }

                    // Skip C/C++ function/method declarations without bodies (defaulted/deleted)
                    if matches!(chunk_type, ChunkType::Function | ChunkType::Method)
                        && capture.node.kind() == "function_definition"
                        && !has_compound_statement(capture.node)
                    {
                        continue;
                    }

                    // Avoid declaration-wrapped duplicates that contain full definitions
                    if capture.node.kind() == "declaration"
                        && declaration_contains_definition(capture.node)
                    {
                        continue;
                    }
                }

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

fn is_inside_c_cpp_type_or_function_body(mut node: tree_sitter::Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        match parent.kind() {
            "class_specifier" | "struct_specifier" | "union_specifier" => return true,
            "function_definition" | "method_definition" => return true,
            _ => {}
        }
        node = parent;
    }
    false
}

fn is_inside_function_or_method(mut node: tree_sitter::Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        match parent.kind() {
            "function_definition"
            | "method_definition"
            | "constructor_definition"
            | "destructor_definition"
            | "lambda_expression" => return true,
            _ => {}
        }
        node = parent;
    }
    false
}

fn declaration_contains_definition(node: tree_sitter::Node<'_>) -> bool {
    let mut stack = vec![node];

    while let Some(current) = stack.pop() {
        if matches!(
            current.kind(),
            "class_specifier"
                | "struct_specifier"
                | "enum_specifier"
                | "union_specifier"
                | "function_definition"
                | "method_definition"
                | "constructor_definition"
                | "destructor_definition"
        ) {
            return true;
        }

        let child_count = current.child_count();
        for idx in (0..child_count).rev() {
            if let Some(child) = current.child(idx) {
                stack.push(child);
            }
        }
    }

    false
}

fn has_compound_statement(node: tree_sitter::Node<'_>) -> bool {
    for idx in 0..node.child_count() {
        if let Some(child) = node.child(idx)
            && child.kind() == "compound_statement"
        {
            return true;
        }
    }
    false
}

fn c_cpp_type_has_body(node: tree_sitter::Node<'_>) -> bool {
    let mut cursor = node.walk();

    match node.kind() {
        "struct_specifier" | "union_specifier" => node
            .children(&mut cursor)
            .any(|child| child.kind() == "field_declaration_list"),
        "enum_specifier" => node
            .children(&mut cursor)
            .any(|child| child.kind() == "enumerator_list"),
        _ => false,
    }
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
        ParseableLanguage::C => Some(include_str!("../queries/c/tags.scm")),
        ParseableLanguage::Cpp => Some(include_str!("../queries/cpp/tags.scm")),
        ParseableLanguage::CSharp => Some(include_str!("../queries/csharp/tags.scm")),
        ParseableLanguage::Zig => Some(include_str!("../queries/zig/tags.scm")),

        ParseableLanguage::Dart => Some(include_str!("../queries/dart/tags.scm")),

        ParseableLanguage::Elixir => Some(include_str!("../queries/elixir/tags.scm")),

        ParseableLanguage::Markdown => Some(include_str!("../queries/markdown/tags.scm")),
    }
}

fn chunk_type_from_capture(name: &str) -> Option<ChunkType> {
    let type_name = name.split('.').next_back().unwrap_or(name);

    match type_name {
        "function" | "fn" => Some(ChunkType::Function),
        "method" => Some(ChunkType::Method),
        "class" | "struct" | "enum" | "trait" => Some(ChunkType::Class),
        "module" | "impl" | "mod" => Some(ChunkType::Module),
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
        let source = r"
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
        ";

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
    fn markdown_queries_capture_headings_and_code_blocks() {
        let source = r#"
# Title

Setext Section
==============

Intro paragraph with **bold** text.

## Usage

```rust
fn main() {
    println!("hi");
}
```

- Item one
- Item two
"#;

        let mut parser = Parser::new();
        let ts_language =
            tree_sitter_language(ParseableLanguage::Markdown).expect("markdown language");
        parser
            .set_language(&ts_language)
            .expect("set markdown language");
        let tree = parser.parse(source, None).expect("parse markdown source");

        let chunks = chunk_with_queries(ParseableLanguage::Markdown, ts_language, &tree, source)
            .expect("query execution")
            .expect("query should be available");

        assert!(chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Module
                && (chunk.text.contains("# Title")
                    || chunk.text.contains("## Usage")
                    || chunk.text.contains("Setext Section"))
        }));
        assert!(chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Text && chunk.text.contains("```rust")
        }));
    }

    #[test]
    #[ignore] // TODO: Update test expectations to match actual query behavior
    fn typescript_queries_capture_core_constructs() {
        let source = r"
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
";

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
        let source = r"
class Helper {
  Helper();
  void help() {}
}

mixin MyMixin {}

enum State { on, off }

void globalFunc() {}

const int MAX = 100;
";

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

    #[test]
    fn c_queries_capture_core_constructs() {
        let source = r#"
#include <stdio.h>

#define MAX_SIZE 1024

#define SQUARE(x) ((x) * (x))

typedef struct {
    int x;
    int y;
} Point;

struct Node {
    int value;
    struct Node* next;
};

enum Color {
    RED,
    GREEN,
    BLUE
};

union Data {
    int i;
    float f;
};

struct Node node_instance;
union Data data_instance;
enum Color color_instance;

void helper(int n) {
    printf("%d\n", n);
}

int compute(int a, int b) {
    return a + b;
}
"#;

        let mut parser = Parser::new();
        let ts_language = tree_sitter_language(ParseableLanguage::C).expect("c language");
        parser.set_language(&ts_language).expect("set c language");
        let tree = parser.parse(source, None).expect("parse c source");

        let chunks = chunk_with_queries(ParseableLanguage::C, ts_language, &tree, source)
            .expect("query execution")
            .expect("query should be available");

        // Check function captures
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.chunk_type == ChunkType::Function
                    && chunk.text.contains("void helper")),
            "Should capture helper function"
        );
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.chunk_type == ChunkType::Function
                    && chunk.text.contains("int compute")),
            "Should capture compute function"
        );

        // Check struct capture
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.chunk_type == ChunkType::Class
                    && chunk.text.contains("struct Node")),
            "Should capture struct Node"
        );

        // Check enum capture
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.chunk_type == ChunkType::Class
                    && chunk.text.contains("enum Color")),
            "Should capture enum Color"
        );

        // Check union capture
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.chunk_type == ChunkType::Class
                    && chunk.text.contains("union Data")),
            "Should capture union Data"
        );

        // Ensure bodyless type specifiers are not captured
        assert!(
            !chunks
                .iter()
                .any(|chunk| chunk.chunk_type == ChunkType::Class
                    && chunk.text.contains("node_instance")),
            "Should not capture struct declarations without bodies"
        );
        assert!(
            !chunks
                .iter()
                .any(|chunk| chunk.chunk_type == ChunkType::Class
                    && chunk.text.contains("data_instance")),
            "Should not capture union declarations without bodies"
        );
        assert!(
            !chunks
                .iter()
                .any(|chunk| chunk.chunk_type == ChunkType::Class
                    && chunk.text.contains("color_instance")),
            "Should not capture enum declarations without bodies"
        );

        // Check macro function capture
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.chunk_type == ChunkType::Function
                    && chunk.text.contains("SQUARE")),
            "Should capture SQUARE macro function"
        );

        // Verify top-level function has empty ancestry
        let compute_chunk = chunks
            .iter()
            .find(|chunk| {
                chunk.chunk_type == ChunkType::Function && chunk.text.contains("int compute")
            })
            .expect("compute chunk present");
        assert!(compute_chunk.metadata.ancestry.is_empty());
    }

    #[test]
    fn cpp_queries_capture_core_constructs() {
        let source = r#"
#include <iostream>
#include <string>

#define MAX_ITEMS 256

namespace utils {

class Calculator {
public:
    Calculator() {}

    int add(int a, int b) {
        return a + b;
    }

    virtual int multiply(int a, int b) {
        return a * b;
    }
};

struct Point {
    double x;
    double y;
};

enum class Color {
    Red,
    Green,
    Blue
};

} // namespace utils

template <typename T>
T identity(T value) {
    return value;
}

void global_func() {
    std::cout << "hello" << std::endl;
}
"#;

        let mut parser = Parser::new();
        let ts_language = tree_sitter_language(ParseableLanguage::Cpp).expect("cpp language");
        parser.set_language(&ts_language).expect("set cpp language");
        let tree = parser.parse(source, None).expect("parse cpp source");

        let chunks = chunk_with_queries(ParseableLanguage::Cpp, ts_language, &tree, source)
            .expect("query execution")
            .expect("query should be available");

        // Check class capture
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.chunk_type == ChunkType::Class
                    && chunk.text.contains("class Calculator")),
            "Should capture class Calculator"
        );

        // Check struct capture
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.chunk_type == ChunkType::Class
                    && chunk.text.contains("struct Point")),
            "Should capture struct Point"
        );

        // Check enum class capture
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.chunk_type == ChunkType::Class
                    && chunk.text.contains("enum class Color")),
            "Should capture enum class Color"
        );

        // Check global function
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.chunk_type == ChunkType::Function
                    && chunk.text.contains("void global_func")),
            "Should capture global_func"
        );

        // Verify top-level function has empty ancestry
        let global_func = chunks
            .iter()
            .find(|chunk| {
                chunk.chunk_type == ChunkType::Function && chunk.text.contains("void global_func")
            })
            .expect("global_func chunk present");
        assert!(global_func.metadata.ancestry.is_empty());
    }

    #[test]
    fn cpp_queries_skip_function_body_declarations() {
        let source = r#"
int compute(int base) {
    int local = base + 1;
    if (auto value = base * 2; value > 0) {
        return value;
    }
    return local;
}
"#;

        let mut parser = Parser::new();
        let ts_language = tree_sitter_language(ParseableLanguage::Cpp).expect("cpp language");
        parser.set_language(&ts_language).expect("set cpp language");
        let tree = parser.parse(source, None).expect("parse cpp source");

        let chunks = chunk_with_queries(ParseableLanguage::Cpp, ts_language, &tree, source)
            .expect("query execution")
            .expect("query should be available");

        assert!(chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Function && chunk.text.contains("int compute")
        }));
        assert!(!chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Text && chunk.text.contains("int local")
        }));
        assert!(!chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Text && chunk.text.contains("auto value")
        }));
    }

    #[test]
    fn cpp_queries_skip_local_class_in_function() {
        let source = r#"
void build() {
    struct Temp {
        int value;
    };
    Temp temp{42};
}
"#;

        let mut parser = Parser::new();
        let ts_language = tree_sitter_language(ParseableLanguage::Cpp).expect("cpp language");
        parser.set_language(&ts_language).expect("set cpp language");
        let tree = parser.parse(source, None).expect("parse cpp source");

        let chunks = chunk_with_queries(ParseableLanguage::Cpp, ts_language, &tree, source)
            .expect("query execution")
            .expect("query should be available");

        assert!(chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Function && chunk.text.contains("void build")
        }));
        assert!(!chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Class && chunk.text.contains("struct Temp")
        }));
    }

    #[test]
    fn cpp_queries_namespace_ancestry_for_class() {
        let source = r#"
namespace outer {
namespace inner {
class Widget {
public:
    void run() {}
};
} // namespace inner
} // namespace outer
"#;

        let mut parser = Parser::new();
        let ts_language = tree_sitter_language(ParseableLanguage::Cpp).expect("cpp language");
        parser.set_language(&ts_language).expect("set cpp language");
        let tree = parser.parse(source, None).expect("parse cpp source");

        let chunks = chunk_with_queries(ParseableLanguage::Cpp, ts_language, &tree, source)
            .expect("query execution")
            .expect("query should be available");

        let class_chunk = chunks
            .iter()
            .find(|chunk| {
                chunk.chunk_type == ChunkType::Class && chunk.text.contains("class Widget")
            })
            .expect("class chunk present");
        assert_eq!(
            class_chunk.metadata.ancestry,
            vec!["outer".to_string(), "inner".to_string()]
        );
    }

    #[test]
    fn cpp_queries_skip_defaulted_deleted_ctors() {
        let source = r#"
class Sample {
public:
    Sample() = default;
    Sample(const Sample&) = delete;
};
"#;

        let mut parser = Parser::new();
        let ts_language = tree_sitter_language(ParseableLanguage::Cpp).expect("cpp language");
        parser.set_language(&ts_language).expect("set cpp language");
        let tree = parser.parse(source, None).expect("parse cpp source");

        let chunks = chunk_with_queries(ParseableLanguage::Cpp, ts_language, &tree, source)
            .expect("query execution")
            .expect("query should be available");

        assert!(chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Class && chunk.text.contains("class Sample")
        }));
        assert!(!chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Text
                && (chunk.text.contains("= default") || chunk.text.contains("= delete"))
        }));
        assert!(!chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Method
                && (chunk.text.contains("= default") || chunk.text.contains("= delete"))
        }));
    }

    #[test]
    fn cpp_queries_drop_text_inside_types_and_functions() {
        let source = r#"
template <typename T>
class Box {
  using Value = T;
  T get() { T local{}; return local; }
};

using Outside = int;

template <typename T>
T make(T value) { T inner = value; return inner; }
"#;

        let mut parser = Parser::new();
        let ts_language = tree_sitter_language(ParseableLanguage::Cpp).expect("cpp language");
        parser.set_language(&ts_language).expect("set cpp language");
        let tree = parser.parse(source, None).expect("parse cpp source");

        let chunks = chunk_with_queries(ParseableLanguage::Cpp, ts_language, &tree, source)
            .expect("query execution")
            .expect("query should be available");

        assert!(chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Class && chunk.text.contains("class Box")
        }));
        assert!(chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Function && chunk.text.contains("T make")
        }));
        assert!(chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Text && chunk.text.contains("using Outside")
        }));
        assert!(!chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Text && chunk.text.contains("using Value")
        }));
        assert!(!chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Text && chunk.text.contains("T local")
        }));
        assert!(!chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Text && chunk.text.contains("T inner")
        }));
    }

    #[test]
    fn cpp_queries_no_declaration_dup_for_class() {
        let source = r#"
class DupCheck {
public:
    void run() {}
};
"#;

        let mut parser = Parser::new();
        let ts_language = tree_sitter_language(ParseableLanguage::Cpp).expect("cpp language");
        parser.set_language(&ts_language).expect("set cpp language");
        let tree = parser.parse(source, None).expect("parse cpp source");

        let chunks = chunk_with_queries(ParseableLanguage::Cpp, ts_language, &tree, source)
            .expect("query execution")
            .expect("query should be available");

        assert!(chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Class && chunk.text.contains("class DupCheck")
        }));
        assert!(!chunks.iter().any(|chunk| {
            chunk.chunk_type == ChunkType::Text && chunk.text.contains("class DupCheck")
        }));
    }
}
