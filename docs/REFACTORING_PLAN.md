# AST-Grep MCP Refactoring Plan

## Current Status: Step 1 Complete ✅

### Completed Steps

#### Step 1: Basic Module Structure ✅
- [x] Extract modules from ast_grep_service.rs
- [x] Create separate modules: config, errors, types, pattern, search, replace, rules
- [x] Fix compilation errors and Position API usage
- [x] Add DRY helper functions (MatchResult::from_node_match)
- [x] All tests passing

### Remaining Steps

#### Step 2: Type Definitions Cleanup
- [ ] Move remaining types from ast_grep_service.rs to appropriate modules
- [ ] Remove duplicate type definitions
- [ ] Ensure all rule-related types are in rules/types.rs
- [ ] Update imports across codebase

#### Step 3: Service Method Delegation
- [ ] Replace ast_grep_service methods with delegation to sub-services
- [ ] Update search methods to use SearchService
- [ ] Update replace methods to use ReplaceService
- [ ] Update rule methods to use RuleService

#### Step 4: Final Cleanup
- [ ] Remove unused code from ast_grep_service.rs
- [ ] Ensure all functionality is properly encapsulated
- [ ] Add comprehensive pagination tests
- [ ] Performance testing

## Future Enhancement: Enum-based Rule AST

### Motivation
Currently, rules are represented as loosely-typed structs with optional fields. This leads to:
- Runtime validation errors
- Unclear rule structure
- Difficult maintenance
- No compile-time guarantees

### Proposed Solution
Replace the current `RuleObject` struct with a type-safe enum hierarchy:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rule {
    // Atomic rules
    Pattern(PatternRule),
    Kind(String),
    Regex(String),
    
    // Composite rules
    All(Vec<Rule>),
    Any(Vec<Rule>),
    Not(Box<Rule>),
    
    // Relational rules
    Inside {
        rule: Box<Rule>,
        stopby: Option<StopBy>,
    },
    Has {
        rule: Box<Rule>,
        stopby: Option<StopBy>,
    },
    Follows {
        rule: Box<Rule>,
        stopby: Option<StopBy>,
    },
    Precedes {
        rule: Box<Rule>,
        stopby: Option<StopBy>,
    },
    
    // Advanced matching
    Matches(String), // meta-variable constraint
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PatternRule {
    Simple(String),
    Advanced {
        context: String,
        selector: Option<String>,
        transform: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopBy {
    Neighbor,
    End,
    Rule(Box<Rule>),
}
```

### Benefits
1. **Type Safety**: Compile-time validation of rule structure
2. **Self-Documenting**: Clear representation of all possible rule types
3. **Better Error Messages**: Rust's pattern matching provides better error reporting
4. **Performance**: No runtime parsing overhead
5. **Maintainability**: Changes to rule structure are caught at compile time

### Implementation Plan
1. Complete current refactoring steps
2. Define enum hierarchy in rules/types.rs
3. Implement serde serialization/deserialization
4. Update RuleEvaluator to use enum pattern matching
5. Add comprehensive tests
6. Update documentation

## Testing Strategy
- [ ] Unit tests for each service module
- [ ] Integration tests for end-to-end functionality
- [ ] Performance benchmarks
- [ ] Pagination edge case testing
- [ ] Error handling validation

## Performance Considerations
- Pattern caching implementation
- Efficient file traversal with proper pagination
- Memory usage optimization for large codebases
- Concurrent processing where appropriate