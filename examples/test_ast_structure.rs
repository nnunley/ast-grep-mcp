use ast_grep_mcp::GenerateAstParam;
use ast_grep_mcp::ast_grep_service::AstGrepService;

#[tokio::main]
async fn main() {
    let service = AstGrepService::new();

    let class_code = r#"class MyClass {
    debug() {
        console.log("debugging");
    }
}
"#;

    let func_code = "function test() { return 42; }";

    println!("=== Class AST Structure ===");
    let class_param = GenerateAstParam {
        code: class_code.to_string(),
        language: "javascript".to_string(),
    };

    match service.generate_ast(class_param).await {
        Ok(result) => {
            println!("AST:\n{}", result.ast);
            println!("\nNode kinds: {:?}", result.node_kinds);
        }
        Err(e) => println!("Error: {e}"),
    }

    println!("\n=== Function AST Structure ===");
    let func_param = GenerateAstParam {
        code: func_code.to_string(),
        language: "javascript".to_string(),
    };

    match service.generate_ast(func_param).await {
        Ok(result) => {
            println!("AST:\n{}", result.ast);
            println!("\nNode kinds: {:?}", result.node_kinds);
        }
        Err(e) => println!("Error: {e}"),
    }
}
