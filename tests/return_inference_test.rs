//! Test-driven development for return value inference functionality
//!
//! Tests define the expected behavior for inferring what values should be returned
//! from extracted code fragments.

use ast_grep_mcp::refactoring::capture_analysis::{
    CaptureAnalysisEngine, ReturnStrategy
};

#[cfg(test)]
mod return_inference_tests {
    use super::*;

    #[test]
    fn test_simple_return_statement() {
        let engine = CaptureAnalysisEngine::new();
        
        let code = r#"
function process() {
    let result = computeValue();
    // Extract this
    return result;
}
"#;
        
        let fragment = "return result;";
        let analysis = engine.analyze_capture_simple(fragment, code, "javascript")
            .expect("Should analyze return statement");
        
        // Should detect explicit return
        assert_eq!(analysis.return_values.len(), 1);
        assert_eq!(analysis.return_values[0].expression, "result");
        
        // Should suggest single return strategy
        if let Some(ReturnStrategy::Single { expression, .. }) = &analysis.suggested_return {
            assert_eq!(expression, "result");
        } else {
            panic!("Expected single return strategy");
        }
    }

    #[test]
    fn test_expression_return_inference() {
        let engine = CaptureAnalysisEngine::new();
        
        let code = r#"
function calculate() {
    let x = 5;
    let y = 10;
    // Extract this
    let sum = x + y;
    let product = x * y;
    console.log(sum, product);
}
"#;
        
        let fragment = "let sum = x + y;\nlet product = x * y;\nconsole.log(sum, product);";
        let analysis = engine.analyze_capture_simple(fragment, code, "javascript")
            .expect("Should analyze expression return");
        
        // Should detect that sum and product are computed but not explicitly returned
        assert_eq!(analysis.internal_declarations.len(), 2);
        
        // Should suggest multiple return strategy for variables that could be useful
        if let Some(ReturnStrategy::Multiple { values }) = &analysis.suggested_return {
            assert!(values.contains(&"sum".to_string()));
            assert!(values.contains(&"product".to_string()));
        } else {
            panic!("Expected multiple return strategy, got: {:?}", analysis.suggested_return);
        }
    }

    #[test]
    fn test_void_function_inference() {
        let engine = CaptureAnalysisEngine::new();
        
        let code = r#"
function processData() {
    // Extract this
    console.log("Processing...");
    updateDatabase();
    sendNotification();
}
"#;
        
        let fragment = "console.log(\"Processing...\");\nupdateDatabase();\nsendNotification();";
        let analysis = engine.analyze_capture_simple(fragment, code, "javascript")
            .expect("Should analyze void function");
        
        // Should detect that no meaningful values are produced
        assert_eq!(analysis.internal_declarations.len(), 0);
        
        // Should suggest void return strategy
        if let Some(ReturnStrategy::Void) = &analysis.suggested_return {
            // Expected
        } else {
            panic!("Expected void return strategy, got: {:?}", analysis.suggested_return);
        }
    }

    #[test]
    fn test_mutation_return_inference() {
        let engine = CaptureAnalysisEngine::new();
        
        let code = r#"
function updateCounter() {
    let count = 0;
    // Extract this
    count += 5;
    count *= 2;
    return count;
}
"#;
        
        let fragment = "count += 5;\ncount *= 2;";
        let analysis = engine.analyze_capture_simple(fragment, code, "javascript")
            .expect("Should analyze mutation return");
        
        // Should detect that count is modified
        assert_eq!(analysis.external_writes.len(), 1);
        assert_eq!(analysis.external_writes[0].name, "count");
        
        // Should suggest in-place modification strategy
        if let Some(ReturnStrategy::InPlace { modified_params }) = &analysis.suggested_return {
            assert!(modified_params.contains(&"count".to_string()));
        } else {
            panic!("Expected in-place modification strategy, got: {:?}", analysis.suggested_return);
        }
    }

    #[test]
    fn test_conditional_return_inference() {
        let engine = CaptureAnalysisEngine::new();
        
        let code = r#"
function validate(input) {
    // Extract this
    if (input.length < 3) {
        return false;
    }
    if (input.contains("bad")) {
        return false;
    }
    return true;
}
"#;
        
        let fragment = r#"if (input.length < 3) {
        return false;
    }
    if (input.contains("bad")) {
        return false;
    }
    return true;"#;
        
        let analysis = engine.analyze_capture_simple(fragment, code, "javascript")
            .expect("Should analyze conditional return");
        
        // Should detect multiple return paths
        assert!(analysis.return_values.len() >= 2);
        
        // Should infer boolean return type
        if let Some(ReturnStrategy::Single { var_type, .. }) = &analysis.suggested_return {
            assert_eq!(var_type, &Some("boolean".to_string()));
        } else {
            panic!("Expected single return strategy with boolean type");
        }
    }

    #[test]
    fn test_loop_accumulator_inference() {
        let engine = CaptureAnalysisEngine::new();
        
        let code = r#"
function sumArray(arr) {
    let total = 0;
    // Extract this
    for (let item of arr) {
        total += item;
    }
    return total;
}
"#;
        
        let fragment = "for (let item of arr) {\n        total += item;\n    }";
        let analysis = engine.analyze_capture_simple(fragment, code, "javascript")
            .expect("Should analyze loop accumulator");
        
        // Should detect that total is modified in a loop
        assert_eq!(analysis.external_writes.len(), 1);
        assert_eq!(analysis.external_writes[0].name, "total");
        
        // Should suggest in-place modification since total is being accumulated
        if let Some(ReturnStrategy::InPlace { modified_params }) = &analysis.suggested_return {
            assert!(modified_params.contains(&"total".to_string()));
        } else {
            panic!("Expected in-place modification strategy for accumulator");
        }
    }
}