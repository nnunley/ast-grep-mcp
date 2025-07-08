/// Example showing basic usage of the ast-grep MCP service
use ast_grep_mcp::ast_grep_service::AstGrepService;
use ast_grep_mcp::SearchParam;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = AstGrepService::new();

    let param = SearchParam {
        code: "function greet() { console.log('Hello!'); }".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
    };

    let result = service.search(param).await?;

    println!("Found {} matches:", result.matches.len());
    for (i, match_result) in result.matches.iter().enumerate() {
        println!("  {}. {}", i + 1, match_result.text);
        for (var, value) in &match_result.vars {
            println!("     ${} = {}", var, value);
        }
    }

    Ok(())
}
