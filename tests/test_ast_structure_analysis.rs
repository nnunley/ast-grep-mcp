use ast_grep_language::SupportLang;
use ast_grep_mcp::ast_utils::AstParser;

#[test]
fn test_rust_struct_ast_structure() {
    let parser = AstParser::new();

    // Analyze the AST structure of a struct with update syntax
    let code_with_update = r#"
let param = FileSearchParam {
    path_pattern: "**/*.js".to_string(),
    pattern: "console.log($VAR)".to_string(),
    language: "javascript".to_string(),
    ..Default::default()
};"#;

    let ast_string = parser.generate_ast_debug_string(code_with_update, SupportLang::Rust);

    println!("AST for struct with update syntax:");
    println!("{ast_string}");

    // Analyze struct without update syntax
    let code_without_update = r#"
let param = FileSearchParam {
    path_pattern: "**/*.js".to_string(),
    pattern: "console.log($VAR)".to_string(),
    language: "javascript".to_string(),
};"#;

    let ast_string2 = parser.generate_ast_debug_string(code_without_update, SupportLang::Rust);

    println!("\nAST for struct without update syntax:");
    println!("{ast_string2}");

    // The AST should show us:
    // 1. How struct update syntax is represented in the tree
    // 2. The node types we need to match
    // 3. The structure that would allow proper insertion
}

#[test]
fn test_finding_struct_update_node() {
    use ast_grep_core::{AstGrep, Pattern};
    use ast_grep_language::SupportLang;

    let code = r#"
let param = MyStruct {
    field1: value1,
    field2: value2,
    ..Default::default()
};"#;

    let ast = AstGrep::new(code, SupportLang::Rust);

    // Try to find the struct update syntax specifically
    // In Rust's tree-sitter grammar, this might be a "field_initializer" with ".." prefix

    // Pattern 1: Try to match the base of struct update
    let pattern1 = Pattern::new("..Default::default()", SupportLang::Rust);
    let root = ast.root();
    let matches1: Vec<_> = root.find_all(pattern1).collect();

    println!("Matches for '..Default::default()': {}", matches1.len());

    // Pattern 2: Try to match the whole struct
    let pattern2 = Pattern::new(
        r#"MyStruct {
    $$$FIELDS
}"#,
        SupportLang::Rust,
    );
    let root2 = ast.root();
    let matches2: Vec<_> = root2.find_all(pattern2).collect();

    for m in &matches2 {
        println!("Struct match: {}", m.text());

        // Explore the match structure
        let node = m.get_node();
        for child in node.children() {
            println!("  Child kind: {}, text: {}", child.kind(), child.text());
        }
    }
}

#[test]
fn test_tree_sitter_struct_generation() {
    // Test if we can use tree-sitter to generate correct struct syntax

    // Tree-sitter parses but doesn't generate - it's a parser, not a pretty-printer
    // However, we can understand the structure to write better patterns

    // The key insight: In Rust's AST, a struct expression has:
    // - struct name
    // - field initializers (field: value pairs)
    // - optional base expression (..expr)

    // The base expression must come last in the source, and tree-sitter preserves this

    println!("Tree-sitter insight: struct update syntax is a 'base_field_initializer' node");
    println!("It must be the last child in a 'field_initializer_list' node");
}
