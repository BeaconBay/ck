use tree_sitter::Parser;

fn main() {
    let code = r#"
factorial :: Integer -> Integer
factorial 0 = 1
factorial n = n * factorial (n - 1)

data Tree a = Empty | Leaf a
"#;

    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_haskell::LANGUAGE.into()).unwrap();

    let tree = parser.parse(code, None).unwrap();

    fn print_tree(node: tree_sitter::Node, source: &str, depth: usize) {
        let indent = "  ".repeat(depth);
        let kind = node.kind();
        let text = &source[node.start_byte()..node.end_byte()];
        let preview = text.lines().next().unwrap_or("");
        println!("{}{} | {:?}", indent, kind, preview);

        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                print_tree(cursor.node(), source, depth + 1);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    print_tree(tree.root_node(), code, 0);
}
