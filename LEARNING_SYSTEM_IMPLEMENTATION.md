# 🎓 Learning System Implementation Plan

**Target**: Add intelligent pattern validation and error recovery to existing ast-grep MCP service

## 📁 Implementation Strategy

### Phase 1: Add Learning Module (Day 1-2)

#### Step 1: Create Learning Module Structure
```bash
mkdir -p src/learning
mkdir -p src/data/patterns
```

#### Step 2: Create Learning Types
**File**: `src/learning/types.rs`
```rust
//! Learning system types and structures

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Enhanced error response with learning guidance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedError {
    pub error: String,
    pub learning_hint: Option<LearningHint>,
    pub suggested_fixes: Vec<PatternFix>,
    pub related_examples: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningHint {
    pub category: String,
    pub explanation: String,
    pub guidance: String,
    pub difficulty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternFix {
    pub description: String,
    pub fixed_pattern: String,
    pub confidence: f32,
    pub rationale: String,
}

/// Parameters for pattern validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatePatternParam {
    pub pattern: String,
    pub language: String,
    pub test_code: Option<String>,
    pub context: Option<String>,
}

/// Result of pattern validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub match_result: Option<crate::types::MatchResult>,
    pub analysis: PatternAnalysis,
    pub learning_insights: Vec<LearningInsight>,
    pub suggested_experiments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternAnalysis {
    pub complexity_score: f32,
    pub language_compatibility: Vec<String>,
    pub metavar_usage: Vec<MetavarInfo>,
    pub potential_issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetavarInfo {
    pub name: String,
    pub capture_type: String,
    pub usage_notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningInsight {
    pub category: String,
    pub insight: String,
    pub actionable_tip: String,
}

/// Parameters for pattern exploration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorePatternParam {
    pub language: Option<String>,
    pub category: Option<String>,
    pub complexity: Option<String>,
    pub search: Option<String>,
    pub limit: Option<u32>,
}

/// Pattern catalog response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternCatalog {
    pub patterns: Vec<CatalogPattern>,
    pub total_available: u32,
    pub learning_path: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogPattern {
    pub id: String,
    pub pattern: String,
    pub description: String,
    pub language: String,
    pub examples: Vec<String>,
    pub difficulty: String,
    pub category: String,
}
```

#### Step 3: Create Pattern Validation Engine
**File**: `src/learning/validation.rs`
```rust
//! Pattern validation with learning guidance

use super::types::*;
use crate::errors::ServiceError;
use crate::types::{MatchResult, SearchParam};
use ast_grep_core::{Pattern};
use ast_grep_language::SupportLang;
use std::str::FromStr;

pub struct ValidationEngine {
    // Simple in-memory pattern knowledge for now
}

impl ValidationEngine {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn validate_pattern(&self, param: ValidatePatternParam) -> Result<ValidationResult, ServiceError> {
        // Parse language
        let lang = SupportLang::from_str(&param.language)
            .map_err(|_| ServiceError::UnsupportedLanguage(param.language.clone()))?;

        // Try to parse the pattern
        let pattern_result = Pattern::new(&param.pattern, lang);

        match pattern_result {
            Ok(pattern) => {
                // Pattern is valid - test it if code provided
                let match_result = if let Some(test_code) = &param.test_code {
                    self.test_pattern(&pattern, test_code, lang).await.ok()
                } else {
                    None
                };

                Ok(ValidationResult {
                    is_valid: true,
                    match_result,
                    analysis: self.analyze_pattern(&param.pattern),
                    learning_insights: self.generate_insights(&param.pattern, true),
                    suggested_experiments: self.suggest_experiments(&param.pattern),
                })
            }
            Err(e) => {
                // Pattern has errors - provide learning guidance
                Ok(ValidationResult {
                    is_valid: false,
                    match_result: None,
                    analysis: self.analyze_pattern(&param.pattern),
                    learning_insights: self.generate_error_insights(&param.pattern, &e.to_string()),
                    suggested_experiments: self.suggest_fixes(&param.pattern),
                })
            }
        }
    }

    async fn test_pattern(&self, pattern: &Pattern, code: &str, lang: SupportLang) -> Result<MatchResult, ServiceError> {
        use ast_grep_core::tree_sitter::StrDoc;

        let doc = StrDoc::new(code, lang);
        let grep = doc.ast_grep(pattern);

        if let Some(node_match) = grep.into_iter().next() {
            let range = node_match.range();
            let start_pos = doc.to_pos(range.start).unwrap_or_default();
            let end_pos = doc.to_pos(range.end).unwrap_or_default();

            let mut vars = std::collections::HashMap::new();
            for (name, captured) in node_match.get_env().get_matched() {
                if let Some(node) = captured.get_captured_node() {
                    vars.insert(name.clone(), node.text().to_string());
                }
            }

            Ok(MatchResult {
                start_line: start_pos.line,
                start_col: start_pos.col,
                end_line: end_pos.line,
                end_col: end_pos.col,
                text: node_match.text().to_string(),
                vars,
            })
        } else {
            Err(ServiceError::PatternMatchFailed("No matches found".to_string()))
        }
    }

    fn analyze_pattern(&self, pattern: &str) -> PatternAnalysis {
        let complexity = self.calculate_complexity(pattern);
        let metavars = self.analyze_metavariables(pattern);
        let compatibility = self.check_language_compatibility(pattern);

        PatternAnalysis {
            complexity_score: complexity,
            language_compatibility: compatibility,
            metavar_usage: metavars,
            potential_issues: self.identify_issues(pattern),
        }
    }

    fn calculate_complexity(&self, pattern: &str) -> f32 {
        let mut score = 0.0;

        // Base complexity from length
        score += pattern.len() as f32 * 0.01;

        // Metavariable complexity
        score += pattern.matches("$").count() as f32 * 0.1;
        score += pattern.matches("$$$").count() as f32 * 0.2;

        // Structure complexity
        score += pattern.matches('{').count() as f32 * 0.15;
        score += pattern.matches('(').count() as f32 * 0.1;

        // Normalize to 0.0-1.0
        (score / 5.0).min(1.0)
    }

    fn analyze_metavariables(&self, pattern: &str) -> Vec<MetavarInfo> {
        let mut metavars = Vec::new();

        // Find $VAR patterns
        if let Ok(re) = regex::Regex::new(r"\$([A-Z_][A-Z0-9_]*)") {
            for cap in re.captures_iter(pattern) {
                if let Some(name) = cap.get(1) {
                    metavars.push(MetavarInfo {
                        name: name.as_str().to_string(),
                        capture_type: "single_node".to_string(),
                        usage_notes: "Captures a single AST node".to_string(),
                    });
                }
            }
        }

        // Find $$$VAR patterns
        if let Ok(re) = regex::Regex::new(r"\$\$\$([A-Z_][A-Z0-9_]*)") {
            for cap in re.captures_iter(pattern) {
                if let Some(name) = cap.get(1) {
                    metavars.push(MetavarInfo {
                        name: name.as_str().to_string(),
                        capture_type: "multiple_nodes".to_string(),
                        usage_notes: "Captures multiple AST nodes (list)".to_string(),
                    });
                }
            }
        }

        metavars
    }

    fn check_language_compatibility(&self, pattern: &str) -> Vec<String> {
        let mut langs = Vec::new();

        if pattern.contains("function") || pattern.contains("const") || pattern.contains("let") {
            langs.extend(vec!["javascript".to_string(), "typescript".to_string()]);
        }
        if pattern.contains("fn ") || pattern.contains("impl ") {
            langs.push("rust".to_string());
        }
        if pattern.contains("def ") || pattern.contains("class ") {
            langs.push("python".to_string());
        }

        if langs.is_empty() {
            langs = vec!["javascript".to_string(), "typescript".to_string(), "python".to_string(), "rust".to_string()];
        }

        langs
    }

    fn identify_issues(&self, pattern: &str) -> Vec<String> {
        let mut issues = Vec::new();

        if pattern.len() > 100 {
            issues.push("Pattern is quite complex - consider breaking it down".to_string());
        }

        if pattern.matches("$").count() > 5 {
            issues.push("Many metavariables - ensure they're all necessary".to_string());
        }

        if !pattern.contains("$") {
            issues.push("No metavariables - this is exact matching only".to_string());
        }

        issues
    }

    fn generate_insights(&self, pattern: &str, is_valid: bool) -> Vec<LearningInsight> {
        let mut insights = Vec::new();

        if is_valid {
            insights.push(LearningInsight {
                category: "success".to_string(),
                insight: "Pattern syntax is valid!".to_string(),
                actionable_tip: "Try testing this pattern on different code samples to see how it behaves".to_string(),
            });
        }

        if pattern.contains("$$$") {
            insights.push(LearningInsight {
                category: "metavariables".to_string(),
                insight: "Using $$$ captures multiple nodes in a list".to_string(),
                actionable_tip: "This is useful for capturing function parameters, array elements, or statement blocks".to_string(),
            });
        }

        insights
    }

    fn generate_error_insights(&self, pattern: &str, error: &str) -> Vec<LearningInsight> {
        let mut insights = Vec::new();

        insights.push(LearningInsight {
            category: "error".to_string(),
            insight: format!("Pattern failed to parse: {}", error),
            actionable_tip: "Check for missing brackets, parentheses, or invalid syntax".to_string(),
        });

        if pattern.contains("{") && !pattern.contains("}") {
            insights.push(LearningInsight {
                category: "syntax".to_string(),
                insight: "Missing closing brace }".to_string(),
                actionable_tip: "Make sure all opening braces { have corresponding closing braces }".to_string(),
            });
        }

        insights
    }

    fn suggest_experiments(&self, pattern: &str) -> Vec<String> {
        let mut experiments = Vec::new();

        experiments.push("Try this pattern on some sample code".to_string());

        if pattern.contains("$") {
            experiments.push("Try changing metavariable names to see how they capture".to_string());
        }

        if !pattern.contains("$$$") {
            experiments.push("Try adding $$$ to capture multiple items".to_string());
        }

        experiments
    }

    fn suggest_fixes(&self, pattern: &str) -> Vec<String> {
        let mut fixes = Vec::new();

        if pattern.contains("{") && !pattern.contains("}") {
            fixes.push("Add missing closing brace }".to_string());
        }

        if pattern.contains("(") && !pattern.contains(")") {
            fixes.push("Add missing closing parenthesis )".to_string());
        }

        fixes.push("Check ast-grep documentation for pattern syntax".to_string());

        fixes
    }
}
```

#### Step 4: Create Pattern Discovery Service
**File**: `src/learning/discovery.rs`
```rust
//! Pattern discovery and exploration

use super::types::*;
use crate::errors::ServiceError;

pub struct DiscoveryService {
    patterns: Vec<CatalogPattern>,
}

impl DiscoveryService {
    pub fn new() -> Self {
        Self {
            patterns: Self::load_embedded_patterns(),
        }
    }

    pub async fn explore_patterns(&self, param: ExplorePatternParam) -> Result<PatternCatalog, ServiceError> {
        let mut filtered_patterns = self.patterns.clone();

        // Apply filters
        if let Some(language) = &param.language {
            filtered_patterns.retain(|p| p.language == *language);
        }

        if let Some(category) = &param.category {
            filtered_patterns.retain(|p| p.category == *category);
        }

        if let Some(complexity) = &param.complexity {
            filtered_patterns.retain(|p| p.difficulty == *complexity);
        }

        if let Some(search) = &param.search {
            filtered_patterns.retain(|p|
                p.pattern.contains(search) ||
                p.description.contains(search)
            );
        }

        // Apply limit
        let limit = param.limit.unwrap_or(20) as usize;
        filtered_patterns.truncate(limit);

        Ok(PatternCatalog {
            patterns: filtered_patterns,
            total_available: self.patterns.len() as u32,
            learning_path: self.generate_learning_path(&param),
        })
    }

    fn load_embedded_patterns() -> Vec<CatalogPattern> {
        vec![
            // JavaScript patterns
            CatalogPattern {
                id: "js_function_declaration".to_string(),
                pattern: "function $NAME($$$PARAMS) { $$$BODY }".to_string(),
                description: "Match function declarations".to_string(),
                language: "javascript".to_string(),
                examples: vec![
                    "function test() { return 42; }".to_string(),
                    "function add(a, b) { return a + b; }".to_string(),
                ],
                difficulty: "beginner".to_string(),
                category: "functions".to_string(),
            },
            CatalogPattern {
                id: "js_console_log".to_string(),
                pattern: "console.log($$$ARGS)".to_string(),
                description: "Match console.log statements".to_string(),
                language: "javascript".to_string(),
                examples: vec![
                    "console.log('hello')".to_string(),
                    "console.log(variable, 'debug')".to_string(),
                ],
                difficulty: "beginner".to_string(),
                category: "debugging".to_string(),
            },
            CatalogPattern {
                id: "js_variable_declaration".to_string(),
                pattern: "const $NAME = $VALUE".to_string(),
                description: "Match const variable declarations".to_string(),
                language: "javascript".to_string(),
                examples: vec![
                    "const x = 5".to_string(),
                    "const message = 'hello'".to_string(),
                ],
                difficulty: "beginner".to_string(),
                category: "variables".to_string(),
            },

            // Rust patterns
            CatalogPattern {
                id: "rust_function_declaration".to_string(),
                pattern: "fn $NAME($$$PARAMS) -> $RETURN { $$$BODY }".to_string(),
                description: "Match Rust function declarations with return type".to_string(),
                language: "rust".to_string(),
                examples: vec![
                    "fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
                ],
                difficulty: "intermediate".to_string(),
                category: "functions".to_string(),
            },
            CatalogPattern {
                id: "rust_println".to_string(),
                pattern: "println!($$$ARGS)".to_string(),
                description: "Match println! macro calls".to_string(),
                language: "rust".to_string(),
                examples: vec![
                    "println!(\"Hello, world!\")".to_string(),
                    "println!(\"Value: {}\", x)".to_string(),
                ],
                difficulty: "beginner".to_string(),
                category: "debugging".to_string(),
            },

            // Python patterns
            CatalogPattern {
                id: "python_function_def".to_string(),
                pattern: "def $NAME($$$PARAMS): $$$BODY".to_string(),
                description: "Match Python function definitions".to_string(),
                language: "python".to_string(),
                examples: vec![
                    "def greet(name): return f'Hello {name}'".to_string(),
                ],
                difficulty: "beginner".to_string(),
                category: "functions".to_string(),
            },
        ]
    }

    fn generate_learning_path(&self, param: &ExplorePatternParam) -> Vec<String> {
        if param.language.as_deref() == Some("javascript") {
            vec![
                "Start with simple variable patterns: const $VAR = $VALUE".to_string(),
                "Learn function patterns: function $NAME() { $$$ }".to_string(),
                "Practice with console.log patterns: console.log($$$)".to_string(),
                "Try more complex patterns with multiple metavariables".to_string(),
            ]
        } else if param.language.as_deref() == Some("rust") {
            vec![
                "Start with println! patterns: println!($$$)".to_string(),
                "Learn function patterns: fn $NAME() -> $TYPE { $$$ }".to_string(),
                "Practice struct patterns: struct $NAME { $$$ }".to_string(),
                "Advanced: impl blocks and trait patterns".to_string(),
            ]
        } else {
            vec![
                "Choose a programming language to start".to_string(),
                "Begin with simple patterns using single metavariables".to_string(),
                "Practice with multiple metavariables using $$$".to_string(),
                "Explore language-specific patterns".to_string(),
            ]
        }
    }
}
```

#### Step 5: Create Learning Module Declaration
**File**: `src/learning/mod.rs`
```rust
//! Learning system for ast-grep pattern education

pub mod types;
pub mod validation;
pub mod discovery;

pub use types::*;
pub use validation::ValidationEngine;
pub use discovery::DiscoveryService;

/// Main learning service coordinator
pub struct LearningService {
    pub validation: ValidationEngine,
    pub discovery: DiscoveryService,
}

impl LearningService {
    pub fn new() -> Self {
        Self {
            validation: ValidationEngine::new(),
            discovery: DiscoveryService::new(),
        }
    }
}

impl Default for LearningService {
    fn default() -> Self {
        Self::new()
    }
}
```

### Phase 2: Integrate with MCP Tools (Day 3)

#### Step 6: Add Learning Service to Main Service
**File**: Modify `src/ast_grep_service.rs`
```rust
// Add to imports at top
use crate::learning::{LearningService, ValidatePatternParam, ValidationResult, ExplorePatternParam, PatternCatalog};

// Add to AstGrepService struct (find the struct and add this field)
pub struct AstGrepService {
    // ... existing fields ...

    /// Learning system for pattern validation and discovery
    pub learning: LearningService,
}

// Modify the constructor (find `impl AstGrepService` and update)
impl AstGrepService {
    pub fn with_config(config: ServiceConfig) -> Self {
        // ... existing initialization ...

        let learning = LearningService::new();

        Self {
            // ... existing fields ...
            learning,
        }
    }

    // Add new method handlers
    pub async fn handle_validate_pattern(&self, param: ValidatePatternParam) -> Result<ValidationResult, crate::errors::ServiceError> {
        self.learning.validation.validate_pattern(param).await
    }

    pub async fn handle_explore_patterns(&self, param: ExplorePatternParam) -> Result<PatternCatalog, crate::errors::ServiceError> {
        self.learning.discovery.explore_patterns(param).await
    }
}
```

#### Step 7: Add New MCP Tools
**File**: Modify `src/tools.rs`
```rust
// Add these tools to the tools vector in `list_tools()` method
// Find the vec![ and add these entries:

Tool {
    name: "validate_pattern".into(),
    description: Some("Validate ast-grep pattern syntax and test against sample code. Provides detailed analysis, learning hints, and suggested improvements. Perfect for testing patterns before applying to large codebases.".into()),
    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
        "type": "object",
        "properties": {
            "pattern": { "type": "string", "description": "AST pattern to validate" },
            "language": { "type": "string", "description": "Programming language for validation" },
            "test_code": { "type": "string", "description": "Optional code sample to test the pattern against" },
            "context": { "type": "string", "enum": ["learning", "production"], "description": "Context for validation guidance" }
        },
        "required": ["pattern", "language"]
    })).unwrap()),
    annotations: None,
},

Tool {
    name: "explore_patterns".into(),
    description: Some("Browse and discover ast-grep patterns by language, category, or complexity. Returns curated pattern library with examples and learning guidance.".into()),
    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
        "type": "object",
        "properties": {
            "language": { "type": "string", "description": "Filter patterns by programming language" },
            "category": { "type": "string", "enum": ["functions", "variables", "classes", "debugging"], "description": "Filter by pattern category" },
            "complexity": { "type": "string", "enum": ["beginner", "intermediate", "advanced"], "description": "Filter by complexity level" },
            "search": { "type": "string", "description": "Search query for finding specific patterns" },
            "limit": { "type": "integer", "minimum": 1, "maximum": 50, "default": 20, "description": "Maximum number of patterns to return" }
        }
    })).unwrap()),
    annotations: None,
},
```

#### Step 8: Add Tool Routing
**File**: Modify `src/tool_router.rs`
```rust
// Add to imports at top
use crate::learning::{ValidatePatternParam, ExplorePatternParam};

// In the `route_tool_call` method, add these cases:
// Find the match statement and add:

"validate_pattern" => {
    let param: ValidatePatternParam = Self::parse_params(request)?;
    let result = service.handle_validate_pattern(param).await
        .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
    Self::create_formatted_response(&result, "🧠 Pattern Validation Complete".to_string())
}

"explore_patterns" => {
    let param: ExplorePatternParam = Self::parse_params(request)?;
    let result = service.handle_explore_patterns(param).await
        .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
    Self::create_formatted_response(&result, "🔍 Pattern Exploration Results".to_string())
}
```

#### Step 9: Update Library Exports
**File**: Modify `src/lib.rs`
```rust
// Add to module declarations
pub mod learning;

// Add to re-exports at bottom
pub use learning::{LearningService, ValidatePatternParam, ValidationResult, ExplorePatternParam, PatternCatalog};
```

### Phase 3: Testing (Day 4)

#### Step 10: Create Tests
**File**: `tests/learning_test.rs`
```rust
//! Tests for learning system functionality

use ast_grep_mcp::learning::{LearningService, ValidatePatternParam, ExplorePatternParam};

#[tokio::test]
async fn test_validate_valid_pattern() {
    let service = LearningService::new();

    let param = ValidatePatternParam {
        pattern: "function $NAME() { $$$BODY }".to_string(),
        language: "javascript".to_string(),
        test_code: Some("function test() { return 42; }".to_string()),
        context: Some("learning".to_string()),
    };

    let result = service.validation.validate_pattern(param).await;
    assert!(result.is_ok());

    let validation = result.unwrap();
    assert!(validation.is_valid);
    assert!(validation.match_result.is_some());
}

#[tokio::test]
async fn test_validate_invalid_pattern() {
    let service = LearningService::new();

    let param = ValidatePatternParam {
        pattern: "function $NAME() { ".to_string(), // Missing closing brace
        language: "javascript".to_string(),
        test_code: None,
        context: Some("learning".to_string()),
    };

    let result = service.validation.validate_pattern(param).await;
    assert!(result.is_ok());

    let validation = result.unwrap();
    assert!(!validation.is_valid);
    assert!(!validation.learning_insights.is_empty());
}

#[tokio::test]
async fn test_explore_patterns() {
    let service = LearningService::new();

    let param = ExplorePatternParam {
        language: Some("javascript".to_string()),
        category: Some("functions".to_string()),
        complexity: None,
        search: None,
        limit: Some(5),
    };

    let result = service.discovery.explore_patterns(param).await;
    assert!(result.is_ok());

    let catalog = result.unwrap();
    assert!(!catalog.patterns.is_empty());
    assert!(!catalog.learning_path.is_empty());
}
```

#### Step 11: Build and Test
```bash
# Build with learning system
cargo build

# Run tests
cargo test learning_test

# Test MCP tools directly
echo '{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "validate_pattern",
    "arguments": {
      "pattern": "function $NAME() { $$$BODY }",
      "language": "javascript",
      "test_code": "function test() { return 42; }"
    }
  }
}' | target/debug/ast-grep-mcp
```

## 🚀 Testing Your Implementation

### Real MCP Testing Commands

```bash
# Test pattern validation
echo '{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "validate_pattern",
    "arguments": {
      "pattern": "console.log($MSG)",
      "language": "javascript",
      "test_code": "console.log(\"hello world\");",
      "context": "learning"
    }
  }
}' | target/debug/ast-grep-mcp

# Test pattern exploration
echo '{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "explore_patterns",
    "arguments": {
      "language": "javascript",
      "category": "functions",
      "limit": 3
    }
  }
}' | target/debug/ast-grep-mcp

# Test invalid pattern
echo '{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "validate_pattern",
    "arguments": {
      "pattern": "function $NAME() {",
      "language": "javascript"
    }
  }
}' | target/debug/ast-grep-mcp
```

### Expected Benefits

1. **Pattern Validation**: LLMs can test patterns before using them
2. **Learning Guidance**: Error messages include helpful hints
3. **Pattern Discovery**: Browse patterns by language and category
4. **Progressive Learning**: Guided learning paths for skill development

### Future Enhancements

1. **Batch Intelligence**: Add preview_batch tool for safe bulk operations
2. **Enhanced Patterns**: Load patterns from external files
3. **Error Recovery**: Smart fix suggestions for failed patterns
4. **Interactive Tutorials**: Step-by-step learning workflows

This implementation provides a solid foundation for the learning system while maintaining compatibility with existing functionality!
