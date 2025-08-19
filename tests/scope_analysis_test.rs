//! Test-driven development for scope analysis functionality
//!
//! Tests define the expected behavior before implementation.

use ast_grep_mcp::refactoring::capture_analysis::{
    CaptureAnalysisEngine, ScopeType, UsageType
};

#[cfg(test)]
mod scope_analysis_tests {
    use super::*;

    #[test]
    fn test_simple_function_scope() {
        let engine = CaptureAnalysisEngine::new();
        
        let code = r#"
function outer() {
    let x = 1;
    let y = 2;
    // Extract this block
    let result = x + y;
    return result;
}
"#;
        
        let fragment = "let result = x + y;\nreturn result;";
        let scope_info = engine.analyze_scope_context(fragment, code, "javascript")
            .expect("Should analyze scope successfully");
        
        // Should detect function scope
        assert_eq!(scope_info.current_scope.scope_type, ScopeType::Function);
        assert_eq!(scope_info.current_scope.depth, 1);
        
        // Should identify x and y as external reads from parent scope
        assert_eq!(scope_info.external_variables.len(), 2);
        assert!(scope_info.external_variables.contains_key("x"));
        assert!(scope_info.external_variables.contains_key("y"));
        
        // Should identify result as internal declaration
        assert_eq!(scope_info.internal_variables.len(), 1);
        assert!(scope_info.internal_variables.contains_key("result"));
    }

    #[test]
    fn test_nested_block_scopes() {
        let engine = CaptureAnalysisEngine::new();
        
        let code = r#"
function outer() {
    let x = 1;
    if (condition) {
        let y = 2;
        {
            // Extract this block
            let z = x + y;
            console.log(z);
        }
    }
}
"#;
        
        let fragment = "let z = x + y;\nconsole.log(z);";
        let scope_info = engine.analyze_scope_context(fragment, code, "javascript")
            .expect("Should analyze nested scopes");
        
        // Should be in a nested block scope (depth 3: function -> if -> block)
        assert_eq!(scope_info.current_scope.depth, 3);
        assert_eq!(scope_info.current_scope.scope_type, ScopeType::Block);
        
        // x comes from function scope (depth 1), y from if scope (depth 2)
        let x_scope = &scope_info.external_variables["x"];
        let y_scope = &scope_info.external_variables["y"];
        
        assert_eq!(x_scope.declared_at_depth, 1);
        assert_eq!(y_scope.declared_at_depth, 2);
        assert_eq!(x_scope.scope_type, ScopeType::Function);
        assert_eq!(y_scope.scope_type, ScopeType::Block);
    }

    #[test]
    fn test_variable_shadowing_detection() {
        let engine = CaptureAnalysisEngine::new();
        
        let code = r#"
function outer() {
    let x = 1;  // outer x
    if (condition) {
        let x = 2;  // shadows outer x
        // Extract this
        let result = x * 2;  // should use inner x
        return result;
    }
}
"#;
        
        let fragment = "let result = x * 2;\nreturn result;";
        let scope_info = engine.analyze_scope_context(fragment, code, "javascript")
            .expect("Should detect shadowing");
        
        // Should identify that x is from the immediate parent scope (if block)
        let x_scope = &scope_info.external_variables["x"];
        assert_eq!(x_scope.declared_at_depth, 2); // if block depth
        assert!(x_scope.is_shadowed);
        assert_eq!(x_scope.shadowed_scopes.len(), 1); // shadows function-level x
    }

    #[test]
    fn test_closure_capture_analysis() {
        let engine = CaptureAnalysisEngine::new();
        
        let code = r#"
function createCounter() {
    let count = 0;
    return function() {
        // Extract this
        count++;
        return count;
    };
}
"#;
        
        let fragment = "count++;\nreturn count;";
        let scope_info = engine.analyze_scope_context(fragment, code, "javascript")
            .expect("Should analyze closure capture");
        
        // count should be identified as a closure-captured variable
        let count_scope = &scope_info.external_variables["count"];
        assert_eq!(count_scope.usage_type, UsageType::ReadWrite);
        assert!(count_scope.is_closure_captured);
        assert_eq!(count_scope.declared_at_depth, 1); // from outer function
    }

    #[test]
    fn test_class_scope_analysis() {
        let engine = CaptureAnalysisEngine::new();
        
        let code = r#"
class Calculator {
    constructor() {
        this.value = 0;
    }
    
    add(x) {
        // Extract this
        this.value += x;
        return this.value;
    }
}
"#;
        
        let fragment = "this.value += x;\nreturn this.value;";
        let scope_info = engine.analyze_scope_context(fragment, code, "javascript")
            .expect("Should analyze class scope");
        
        // Should be in method scope within class
        assert_eq!(scope_info.current_scope.scope_type, ScopeType::Method);
        
        // x should be parameter, this.value should be instance member
        let x_scope = &scope_info.external_variables["x"];
        assert_eq!(x_scope.scope_type, ScopeType::Parameter);
        
        // Should detect instance member access
        assert!(scope_info.instance_members.contains(&"value".to_string()));
    }

    #[test]
    fn test_scope_boundary_crossing() {
        let engine = CaptureAnalysisEngine::new();
        
        let code = r#"
function outer() {
    let outerVar = 1;
    
    function inner() {
        let innerVar = 2;
        // Fragment spans multiple scopes - should be rejected or flagged
        return outerVar + innerVar;
    }
    
    return inner();
}
"#;
        
        let fragment = r#"function inner() {
        let innerVar = 2;
        return outerVar + innerVar;
    }"#;
        
        let scope_info = engine.analyze_scope_context(fragment, code, "javascript")
            .expect("Should handle scope boundary crossing");
        
        // Should detect that extraction spans multiple scope levels
        assert!(scope_info.crosses_scope_boundaries);
        assert_eq!(scope_info.scope_violations.len(), 1);
        assert!(scope_info.scope_violations[0].contains("function boundary"));
    }

    #[test]
    fn test_python_scope_analysis() {
        let engine = CaptureAnalysisEngine::new();
        
        let code = r#"
def outer_function():
    x = 1
    
    def inner_function():
        nonlocal x
        # Extract this
        x += 1
        return x
    
    return inner_function()
"#;
        
        let fragment = "x += 1\nreturn x";
        let scope_info = engine.analyze_scope_context(fragment, code, "python")
            .expect("Should analyze Python scopes");
        
        // Should detect nonlocal variable usage
        let x_scope = &scope_info.external_variables["x"];
        assert!(x_scope.is_nonlocal);
        assert_eq!(x_scope.usage_type, UsageType::ReadWrite);
    }

    #[test]
    fn test_parameter_conflict_detection() {
        let engine = CaptureAnalysisEngine::new();
        
        let code = r#"
function process(data, callback) {
    let result = [];
    // Extract this - 'data' conflicts with parameter
    for (let data of items) {
        result.push(transform(data));
    }
    return result;
}
"#;
        
        let fragment = r#"for (let data of items) {
        result.push(transform(data));
    }"#;
        
        let scope_info = engine.analyze_scope_context(fragment, code, "javascript")
            .expect("Should detect parameter conflicts");
        
        // Should detect naming conflict
        assert!(!scope_info.naming_conflicts.is_empty());
        assert!(scope_info.naming_conflicts.contains_key("data"));
        
        // Should suggest parameter renaming
        let suggestions = engine.suggest_parameter_names(&scope_info);
        assert!(suggestions.contains_key("items"));
        assert_ne!(suggestions["items"], "data"); // Should suggest different name
    }
}