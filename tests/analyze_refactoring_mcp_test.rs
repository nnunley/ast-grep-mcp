//! Integration test for the analyze_refactoring MCP tool
//! This test verifies the MCP service method works correctly

use ast_grep_mcp::{
    ast_grep_service::AstGrepService,
    types::AnalyzeRefactoringParam,
};

#[tokio::test]
async fn test_analyze_refactoring_mcp_integration() {
    let service = AstGrepService::new();
    
    // Test JavaScript code fragment extraction
    let param = AnalyzeRefactoringParam {
        fragment: "let result = x + y;\nconsole.log(result);".to_string(),
        context: "function calculate() { let x = 5; let y = 10; let result = x + y; console.log(result); return result; }".to_string(),
        language: "javascript".to_string(),
    };

    let result = service.analyze_refactoring(param).await.unwrap();

    // Verify external reads (parameters needed)
    assert_eq!(result.external_reads.len(), 2);
    let var_names: Vec<&str> = result.external_reads.iter().map(|v| v.name.as_str()).collect();
    assert!(var_names.contains(&"x"));
    assert!(var_names.contains(&"y"));

    // Verify internal declarations
    assert_eq!(result.internal_declarations.len(), 1);
    assert_eq!(result.internal_declarations[0].name, "result");

    // Verify side effects detected (console.log)
    assert!(!result.side_effects.is_empty());
    let side_effect_types: Vec<&str> = result.side_effects.iter().map(|s| s.effect_type.as_str()).collect();
    assert!(side_effect_types.contains(&"io_operation") || side_effect_types.contains(&"function_call"));

    // Verify suggested signature
    assert_eq!(result.suggested_signature.name, "extractedFunction");
    assert_eq!(result.suggested_signature.parameters.len(), 2);
    assert!(!result.suggested_signature.is_pure); // console.log makes it impure

    // Verify return strategy (should be void since console.log doesn't return)
    if let Some(strategy) = &result.suggested_return_strategy {
        assert_eq!(strategy.strategy_type, "void");
    }

    println!("✅ analyze_refactoring MCP tool integration test passed");
    println!("📊 External reads: {}", result.external_reads.len());
    println!("📊 Side effects: {}", result.side_effects.len());
    println!("📊 Function purity: {}", if result.suggested_signature.is_pure { "Pure" } else { "Impure" });
}

#[tokio::test]
async fn test_analyze_refactoring_with_return_value() {
    let service = AstGrepService::new();
    
    // Test fragment that returns a value
    let param = AnalyzeRefactoringParam {
        fragment: "let sum = a + b;\nreturn sum;".to_string(),
        context: "function add(a, b) { let sum = a + b; return sum; }".to_string(),
        language: "javascript".to_string(),
    };

    let result = service.analyze_refactoring(param).await.unwrap();

    // Should have return values detected
    assert!(!result.return_values.is_empty());
    assert_eq!(result.return_values[0].expression, "sum");

    // Should suggest single return strategy
    if let Some(strategy) = &result.suggested_return_strategy {
        assert_eq!(strategy.strategy_type, "single");
        assert_eq!(strategy.expression.as_ref().unwrap(), "sum");
    }

    // Should be pure (no side effects)
    assert!(result.side_effects.is_empty());
    assert!(result.suggested_signature.is_pure);

    println!("✅ analyze_refactoring return value test passed");
}

#[tokio::test]
async fn test_analyze_refactoring_python() {
    let service = AstGrepService::new();
    
    // Test Python code
    let param = AnalyzeRefactoringParam {
        fragment: "result = x + y\nprint(result)".to_string(),
        context: "def calculate():\n    x = 5\n    y = 10\n    result = x + y\n    print(result)\n    return result".to_string(),
        language: "python".to_string(),
    };

    let result = service.analyze_refactoring(param).await.unwrap();

    // Should detect external variables
    assert_eq!(result.external_reads.len(), 2);
    let var_names: Vec<&str> = result.external_reads.iter().map(|v| v.name.as_str()).collect();
    assert!(var_names.contains(&"x"));
    assert!(var_names.contains(&"y"));

    // Should detect side effects (print)
    assert!(!result.side_effects.is_empty());

    println!("✅ analyze_refactoring Python test passed");
}