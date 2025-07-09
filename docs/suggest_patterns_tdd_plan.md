# TDD Development Plan: `suggest_patterns`

## Overview

This plan follows Test-Driven Development (TDD) principles to implement `suggest_patterns` incrementally. We'll write tests first, implement the minimum code to pass, then refactor. This approach ensures we build exactly what's needed while maintaining quality.

## Phase 1: Core Foundation (Days 1-3)

### Day 1: Basic Types and MCP Integration

**Test 1: MCP Tool Registration**
```rust
// tests/test_suggest_patterns.rs
#[tokio::test]
async fn test_suggest_patterns_tool_registered() {
    let service = AstGrepService::new(Default::default());
    let tools = service.list_tools().await.unwrap();
    assert!(tools.iter().any(|tool| tool.name == "suggest_patterns"));
}
```

**Test 2: Basic Parameter Parsing**
```rust
#[tokio::test]
async fn test_suggest_patterns_accepts_basic_params() {
    let params = SuggestPatternsParam {
        code_examples: vec!["function test() {}".to_string()],
        language: "javascript".to_string(),
        max_suggestions: Some(5),
        specificity_levels: None,
    };

    let service = AstGrepService::new(Default::default());
    let result = service.suggest_patterns(params).await;
    assert!(result.is_ok());
}
```

**Implementation**: Create basic types and stub MCP integration.

### Day 2: Single Pattern Exact Match

**Test 3: Exact Pattern Generation**
```rust
#[tokio::test]
async fn test_exact_pattern_generation() {
    let examples = vec!["console.log('hello')".to_string()];
    let result = suggest_patterns(examples, "javascript").await.unwrap();

    assert_eq!(result.suggestions.len(), 1);
    assert_eq!(result.suggestions[0].pattern, "console.log('hello')");
    assert_eq!(result.suggestions[0].specificity, SpecificityLevel::Exact);
}
```

**Test 4: Multiple Identical Examples**
```rust
#[tokio::test]
async fn test_multiple_identical_examples() {
    let examples = vec![
        "console.log('test')".to_string(),
        "console.log('test')".to_string(),
    ];
    let result = suggest_patterns(examples, "javascript").await.unwrap();

    assert_eq!(result.suggestions.len(), 1);
    assert_eq!(result.suggestions[0].confidence, 1.0);
}
```

**Implementation**: Create exact pattern matching for identical code.

### Day 3: Basic Metavariable Generation

**Test 5: Simple Metavariable Substitution**
```rust
#[tokio::test]
async fn test_simple_metavariable_generation() {
    let examples = vec![
        "console.log('hello')".to_string(),
        "console.log('world')".to_string(),
    ];
    let result = suggest_patterns(examples, "javascript").await.unwrap();

    assert_eq!(result.suggestions.len(), 1);
    assert_eq!(result.suggestions[0].pattern, "console.log($MSG)");
    assert_eq!(result.suggestions[0].specificity, SpecificityLevel::General);
}
```

**Test 6: Function Name Metavariables**
```rust
#[tokio::test]
async fn test_function_name_metavariables() {
    let examples = vec![
        "function getUserData() {}".to_string(),
        "function getPostData() {}".to_string(),
    ];
    let result = suggest_patterns(examples, "javascript").await.unwrap();

    assert!(result.suggestions.iter().any(|s|
        s.pattern == "function $NAME() {}" ||
        s.pattern == "function get$TYPE() {}"
    ));
}
```

**Implementation**: Basic AST comparison and metavariable generation.

## Phase 2: Enhanced Pattern Matching (Days 4-7)

### Day 4: Structural Pattern Analysis

**Test 7: Nested Structure Patterns**
```rust
#[tokio::test]
async fn test_nested_structure_patterns() {
    let examples = vec![
        "if (condition) { doSomething(); }".to_string(),
        "if (otherCondition) { doOtherThing(); }".to_string(),
    ];
    let result = suggest_patterns(examples, "javascript").await.unwrap();

    assert!(result.suggestions.iter().any(|s|
        s.pattern == "if ($CONDITION) { $BODY }"
    ));
}
```

**Test 8: Multiple Statement Patterns**
```rust
#[tokio::test]
async fn test_multiple_statement_patterns() {
    let examples = vec![
        "function test() { let x = 1; return x; }".to_string(),
        "function other() { let y = 2; return y; }".to_string(),
    ];
    let result = suggest_patterns(examples, "javascript").await.unwrap();

    assert!(result.suggestions.iter().any(|s|
        s.pattern == "function $NAME() { $$$BODY }"
    ));
}
```

**Implementation**: AST traversal and structural pattern extraction.

### Day 5: Confidence Scoring

**Test 9: Coverage-Based Confidence**
```rust
#[tokio::test]
async fn test_confidence_scoring() {
    let examples = vec![
        "console.log('a')".to_string(),
        "console.log('b')".to_string(),
        "console.log('c')".to_string(),
    ];
    let result = suggest_patterns(examples, "javascript").await.unwrap();

    let general_pattern = result.suggestions.iter()
        .find(|s| s.pattern == "console.log($MSG)").unwrap();

    assert!(general_pattern.confidence > 0.8);
    assert_eq!(general_pattern.matching_examples, vec![0, 1, 2]);
}
```

**Test 10: Specificity vs Confidence Trade-off**
```rust
#[tokio::test]
async fn test_specificity_confidence_tradeoff() {
    let examples = vec![
        "getUserData(123)".to_string(),
        "getPostData(456)".to_string(),
    ];
    let result = suggest_patterns(examples, "javascript").await.unwrap();

    let general = result.suggestions.iter()
        .find(|s| s.pattern == "$FUNC($ARG)").unwrap();
    let specific = result.suggestions.iter()
        .find(|s| s.pattern == "get$TYPE($ARG)").unwrap();

    assert!(general.confidence > specific.confidence);
}
```

**Implementation**: Confidence calculation based on coverage and specificity.

### Day 6: Pattern Ranking and Filtering

**Test 11: Pattern Ranking**
```rust
#[tokio::test]
async fn test_pattern_ranking() {
    let examples = vec![
        "console.log('hello')".to_string(),
        "console.error('world')".to_string(),
    ];
    let result = suggest_patterns(examples, "javascript").await.unwrap();

    // Should be ranked by confidence
    assert!(result.suggestions.windows(2).all(|w|
        w[0].confidence >= w[1].confidence
    ));
}
```

**Test 12: Max Suggestions Limit**
```rust
#[tokio::test]
async fn test_max_suggestions_limit() {
    let examples = vec![
        "function a() {}".to_string(),
        "function b() {}".to_string(),
    ];
    let params = SuggestPatternsParam {
        code_examples: examples,
        language: "javascript".to_string(),
        max_suggestions: Some(2),
        specificity_levels: None,
    };

    let result = suggest_patterns_with_params(params).await.unwrap();
    assert!(result.suggestions.len() <= 2);
}
```

**Implementation**: Ranking algorithms and result filtering.

### Day 7: Error Handling and Edge Cases

**Test 13: Invalid Code Handling**
```rust
#[tokio::test]
async fn test_invalid_code_handling() {
    let examples = vec!["function incomplete(".to_string()];
    let result = suggest_patterns(examples, "javascript").await;

    // Should gracefully handle parsing errors
    assert!(result.is_ok());
    assert!(result.unwrap().suggestions.is_empty());
}
```

**Test 14: Empty Examples**
```rust
#[tokio::test]
async fn test_empty_examples() {
    let examples = vec![];
    let result = suggest_patterns(examples, "javascript").await;

    assert!(result.is_ok());
    assert!(result.unwrap().suggestions.is_empty());
}
```

**Test 15: Unsupported Language**
```rust
#[tokio::test]
async fn test_unsupported_language() {
    let examples = vec!["some code".to_string()];
    let result = suggest_patterns(examples, "unsupported").await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("unsupported language"));
}
```

**Implementation**: Robust error handling and edge case management.

## Phase 3: Multi-Language Support (Days 8-10)

### Day 8: TypeScript Support

**Test 16: TypeScript Interface Patterns**
```rust
#[tokio::test]
async fn test_typescript_interface_patterns() {
    let examples = vec![
        "interface User { name: string; }".to_string(),
        "interface Post { title: string; }".to_string(),
    ];
    let result = suggest_patterns(examples, "typescript").await.unwrap();

    assert!(result.suggestions.iter().any(|s|
        s.pattern == "interface $NAME { $FIELD: string; }"
    ));
}
```

**Test 17: Generic Type Patterns**
```rust
#[tokio::test]
async fn test_generic_type_patterns() {
    let examples = vec![
        "function getId<T>(item: T): string { return item.id; }".to_string(),
        "function getName<T>(item: T): string { return item.name; }".to_string(),
    ];
    let result = suggest_patterns(examples, "typescript").await.unwrap();

    assert!(result.suggestions.iter().any(|s|
        s.pattern == "function get$PROP<T>(item: T): string { return item.$PROP; }"
    ));
}
```

**Implementation**: TypeScript-specific pattern recognition.

### Day 9: Python Support

**Test 18: Python Function Patterns**
```rust
#[tokio::test]
async fn test_python_function_patterns() {
    let examples = vec![
        "def get_user_data():\n    return data".to_string(),
        "def get_post_data():\n    return data".to_string(),
    ];
    let result = suggest_patterns(examples, "python").await.unwrap();

    assert!(result.suggestions.iter().any(|s|
        s.pattern == "def get_$TYPE():\n    return data"
    ));
}
```

**Test 19: Python Class Patterns**
```rust
#[tokio::test]
async fn test_python_class_patterns() {
    let examples = vec![
        "class UserModel:\n    def __init__(self):\n        pass".to_string(),
        "class PostModel:\n    def __init__(self):\n        pass".to_string(),
    ];
    let result = suggest_patterns(examples, "python").await.unwrap();

    assert!(result.suggestions.iter().any(|s|
        s.pattern == "class $NAMEModel:\n    def __init__(self):\n        pass"
    ));
}
```

**Implementation**: Python-specific pattern recognition.

### Day 10: Rust Support

**Test 20: Rust Function Patterns**
```rust
#[tokio::test]
async fn test_rust_function_patterns() {
    let examples = vec![
        "fn get_user() -> User { User::new() }".to_string(),
        "fn get_post() -> Post { Post::new() }".to_string(),
    ];
    let result = suggest_patterns(examples, "rust").await.unwrap();

    assert!(result.suggestions.iter().any(|s|
        s.pattern == "fn get_$TYPE() -> $TYPE { $TYPE::new() }"
    ));
}
```

**Test 21: Rust Struct Patterns**
```rust
#[tokio::test]
async fn test_rust_struct_patterns() {
    let examples = vec![
        "struct User { name: String }".to_string(),
        "struct Post { title: String }".to_string(),
    ];
    let result = suggest_patterns(examples, "rust").await.unwrap();

    assert!(result.suggestions.iter().any(|s|
        s.pattern == "struct $NAME { $FIELD: String }"
    ));
}
```

**Implementation**: Rust-specific pattern recognition.

## Phase 4: Advanced Features (Days 11-14)

### Day 11: Explanation Generation

**Test 22: Pattern Explanations**
```rust
#[tokio::test]
async fn test_pattern_explanations() {
    let examples = vec![
        "console.log('hello')".to_string(),
        "console.log('world')".to_string(),
    ];
    let result = suggest_patterns(examples, "javascript").await.unwrap();

    let pattern = result.suggestions.iter()
        .find(|s| s.pattern == "console.log($MSG)").unwrap();

    assert!(pattern.explanation.contains("console.log"));
    assert!(pattern.explanation.contains("message"));
}
```

**Test 23: Node Kind Information**
```rust
#[tokio::test]
async fn test_node_kind_information() {
    let examples = vec!["function test() {}".to_string()];
    let result = suggest_patterns(examples, "javascript").await.unwrap();

    let pattern = &result.suggestions[0];
    assert!(pattern.node_kinds.contains(&"function_declaration".to_string()));
}
```

**Implementation**: Explanation generation and node kind extraction.

### Day 12: Performance Optimization

**Test 24: Performance Benchmarks**
```rust
#[tokio::test]
async fn test_performance_benchmarks() {
    let examples = (0..100).map(|i| format!("console.log('test{}')", i)).collect();

    let start = std::time::Instant::now();
    let result = suggest_patterns(examples, "javascript").await.unwrap();
    let duration = start.elapsed();

    assert!(duration.as_millis() < 1000); // Under 1 second
    assert!(!result.suggestions.is_empty());
}
```

**Test 25: Memory Usage Validation**
```rust
#[tokio::test]
async fn test_memory_usage() {
    let large_examples = (0..1000).map(|i|
        format!("function test{}() {{ return {}; }}", i, i)
    ).collect();

    // Should not crash or use excessive memory
    let result = suggest_patterns(large_examples, "javascript").await;
    assert!(result.is_ok());
}
```

**Implementation**: Performance monitoring and optimization.

### Day 13: Integration Testing

**Test 26: End-to-End MCP Integration**
```rust
#[tokio::test]
async fn test_end_to_end_mcp_integration() {
    let service = AstGrepService::new(Default::default());

    let params = json!({
        "code_examples": ["function test() {}", "function other() {}"],
        "language": "javascript",
        "max_suggestions": 5
    });

    let result = service.call_tool("suggest_patterns", params).await;
    assert!(result.is_ok());

    let content = result.unwrap().content;
    assert!(content.len() > 0);
}
```

**Test 27: Integration with Existing Tools**
```rust
#[tokio::test]
async fn test_integration_with_file_search() {
    let service = AstGrepService::new(Default::default());

    // First, get pattern suggestions
    let suggest_result = service.suggest_patterns(SuggestPatternsParam {
        code_examples: vec!["console.log('test')".to_string()],
        language: "javascript".to_string(),
        max_suggestions: Some(1),
        specificity_levels: None,
    }).await.unwrap();

    // Then use suggested pattern in file search
    let pattern = &suggest_result.suggestions[0].pattern;
    let search_result = service.file_search(FileSearchParam {
        path_pattern: "**/*.js".to_string(),
        pattern: pattern.clone(),
        language: "javascript".to_string(),
        max_results: Some(10),
        cursor: None,
        max_file_size: None,
    }).await;

    assert!(search_result.is_ok());
}
```

**Implementation**: Full integration testing with existing systems.

### Day 14: Production Readiness

**Test 28: Concurrent Request Handling**
```rust
#[tokio::test]
async fn test_concurrent_requests() {
    let service = Arc::new(AstGrepService::new(Default::default()));

    let mut handles = vec![];
    for i in 0..10 {
        let service = service.clone();
        let handle = tokio::spawn(async move {
            let examples = vec![format!("function test{}() {{}}", i)];
            service.suggest_patterns(SuggestPatternsParam {
                code_examples: examples,
                language: "javascript".to_string(),
                max_suggestions: Some(5),
                specificity_levels: None,
            }).await
        });
        handles.push(handle);
    }

    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }
}
```

**Test 29: Error Recovery**
```rust
#[tokio::test]
async fn test_error_recovery() {
    let service = AstGrepService::new(Default::default());

    // Send invalid request
    let _ = service.suggest_patterns(SuggestPatternsParam {
        code_examples: vec!["invalid syntax(".to_string()],
        language: "javascript".to_string(),
        max_suggestions: Some(5),
        specificity_levels: None,
    }).await;

    // Service should still work for valid requests
    let result = service.suggest_patterns(SuggestPatternsParam {
        code_examples: vec!["function test() {}".to_string()],
        language: "javascript".to_string(),
        max_suggestions: Some(5),
        specificity_levels: None,
    }).await;

    assert!(result.is_ok());
}
```

**Implementation**: Production-ready error handling and recovery.

## Implementation Architecture

### Core Components

1. **Pattern Analyzer** (`src/pattern_analysis/analyzer.rs`)
   - AST parsing and comparison
   - Pattern extraction algorithms
   - Confidence scoring

2. **Suggestion Generator** (`src/pattern_analysis/generator.rs`)
   - Pattern generation strategies
   - Metavariable substitution
   - Ranking and filtering

3. **Language Support** (`src/pattern_analysis/languages/`)
   - Language-specific pattern templates
   - Node kind mappings
   - Convention scoring

4. **MCP Integration** (`src/ast_grep_service.rs`)
   - Tool registration
   - Parameter validation
   - Response formatting

### Development Workflow

1. **Write Test**: Define expected behavior
2. **Run Test**: Verify it fails (Red)
3. **Write Code**: Minimum implementation to pass (Green)
4. **Refactor**: Improve code quality while keeping tests passing
5. **Repeat**: Move to next test

### Benefits of This Approach

- **Quality**: Tests ensure correctness from day one
- **Simplicity**: Build only what's needed
- **Confidence**: Refactoring is safe with comprehensive tests
- **Documentation**: Tests serve as living documentation
- **Debugging**: Failed tests pinpoint exact issues

This TDD approach will deliver a robust, well-tested implementation in approximately 2-3 weeks with high confidence in correctness and maintainability.
