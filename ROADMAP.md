# 🗺️ ast-grep MCP Development Roadmap

This document outlines the technical roadmap for enhancing ast-grep MCP to become an AI-native code understanding and transformation platform.

## 🎯 Vision Statement

Transform ast-grep MCP from a pattern matching tool into an intelligent code companion that bridges the gap between natural language intent and structural code transformation, optimized for LLM-driven development workflows.

## 📋 Current Status (v0.1.0)

### ✅ Completed Features
- Core ast-grep pattern matching (`search`, `file_search`)
- Rule-based search and replace (`rule_search`, `rule_replace`)
- Tree-sitter node kind discovery (`generate_ast`)
- Token-efficient diff output for large codebases
- Rule management system (`create_rule`, `list_rules`, etc.)
- CLI integration with configurable root directories
- Comprehensive MCP service integration

### 🔧 Technical Debt
- [ ] Fix failing end-to-end rule tests (8 failing tests)
- [ ] Improve rule evaluation performance for large files
- [ ] Add more comprehensive error handling and recovery

---

## 🚀 Phase 1: LLM-Friendly Pattern Discovery (v0.2.0)

**Target: Q2 2024 | Focus: Reduce LLM friction in pattern creation**

### 1.1 `suggest_patterns` Tool

**Priority: Critical**

```rust
// Tool interface
pub struct SuggestPatternsParam {
    pub example_code: String,
    pub language: String,
    pub intent: Option<String>, // Optional natural language description
    pub max_suggestions: Option<usize>, // Default: 5
}

pub struct SuggestPatternsResult {
    pub suggestions: Vec<PatternSuggestion>,
}

pub struct PatternSuggestion {
    pub pattern: String,
    pub confidence: f32, // 0.0 - 1.0
    pub explanation: String,
    pub matches_example: bool,
    pub generalization_level: GeneralizationLevel, // Specific, Moderate, General
}
```

**Implementation Strategy:**
- Start with rule-based pattern generation using AST analysis
- Identify metavariable opportunities (`$VAR`, `$NAME`, etc.)
- Generate patterns at different abstraction levels
- Future: ML model for pattern quality scoring

**Technical Requirements:**
- New module: `src/pattern_suggestion.rs`
- AST traversal algorithms for common patterns
- Pattern confidence scoring system
- Integration with existing pattern cache

### 1.2 `analyze_change_impact` Tool

**Priority: Critical**

```rust
pub struct AnalyzeChangeImpactParam {
    pub pattern: String,
    pub replacement: String,
    pub language: String,
    pub path_pattern: String,
    pub include_tests: Option<bool>, // Default: true
}

pub struct ChangeImpactResult {
    pub total_files_affected: usize,
    pub total_matches: usize,
    pub risk_level: RiskLevel, // Low, Medium, High
    pub affected_functions: Vec<FunctionInfo>,
    pub breaking_change_probability: f32,
    pub test_coverage_impact: TestCoverageImpact,
    pub dependencies_affected: Vec<DependencyInfo>,
}
```

**Implementation Strategy:**
- Static analysis of affected code regions
- Function signature change detection
- Import/export impact analysis
- Test file correlation with source changes

**Technical Requirements:**
- New module: `src/impact_analysis.rs`
- Function boundary detection using Tree-sitter
- Dependency graph construction
- Risk scoring algorithms

### 1.3 `search_by_intent` Tool

**Priority: High**

```rust
pub struct SearchByIntentParam {
    pub intent: String, // Natural language description
    pub language: String,
    pub path_pattern: Option<String>,
    pub confidence_threshold: Option<f32>, // Default: 0.6
}

pub struct SearchByIntentResult {
    pub intent_understanding: IntentAnalysis,
    pub generated_patterns: Vec<String>,
    pub combined_results: Vec<MatchResult>,
    pub pattern_effectiveness: Vec<PatternStats>,
}
```

**Implementation Strategy:**
- Intent parsing using keyword extraction and NLP
- Pattern template library for common intents
- Multi-pattern search with result deduplication
- Future: LLM integration for intent understanding

**Technical Requirements:**
- Intent classification system
- Pattern template database
- Result ranking and deduplication
- Performance optimization for multi-pattern searches

---

## 🔧 Phase 2: Semantic Code Understanding (v0.3.0)

**Target: Q3 2024 | Focus: Deep code analysis and relationships**

### 2.1 `find_similar_patterns` Tool

```rust
pub struct FindSimilarPatternsParam {
    pub reference_code: String,
    pub language: String,
    pub similarity_threshold: Option<f32>, // Default: 0.7
    pub semantic_level: SemanticLevel, // Syntactic, Structural, Behavioral
}
```

**Implementation Strategy:**
- AST structure comparison algorithms
- Semantic similarity using control flow analysis
- Variable role analysis (loop counters, accumulators, etc.)
- Pattern abstraction and normalization

### 2.2 `analyze_code_context` Tool

```rust
pub struct AnalyzeCodeContextParam {
    pub file_path: String,
    pub context_depth: Option<u32>, // Default: 2 (immediate dependencies)
}

pub struct CodeContextResult {
    pub function_signatures: Vec<FunctionSignature>,
    pub imports: Vec<ImportInfo>,
    pub exports: Vec<ExportInfo>,
    pub call_graph: CallGraph,
    pub inheritance_hierarchy: Vec<ClassRelation>,
    pub variable_flow: Vec<VariableUsage>,
}
```

### 2.3 `bulk_refactor` Tool

```rust
pub struct BulkRefactorParam {
    pub transformation_pipeline: Vec<TransformationStep>,
    pub conflict_resolution: ConflictResolution, // Abort, Skip, Interactive
    pub validation_rules: Vec<ValidationRule>,
}

pub struct TransformationStep {
    pub rule_id: String,
    pub priority: u32,
    pub dependencies: Vec<String>, // Other rule IDs this depends on
    pub rollback_on_failure: bool,
}
```

### 2.4 `verify_transformation_safety` Tool

```rust
pub struct VerifyTransformationSafetyParam {
    pub original_code: String,
    pub transformed_code: String,
    pub language: String,
    pub checks: Vec<SafetyCheck>, // Syntax, Semantics, Types, Control Flow
}

pub struct SafetyVerificationResult {
    pub is_safe: bool,
    pub safety_score: f32,
    pub potential_issues: Vec<SafetyIssue>,
    pub preservation_analysis: PreservationAnalysis,
}
```

---

## 🧠 Phase 3: Project Intelligence (v0.4.0)

**Target: Q4 2024 | Focus: Adaptive and learning capabilities**

### 3.1 Machine Learning Integration

- Pattern suggestion quality improvement using ML
- Code similarity learning from project history
- Custom pattern generation based on project conventions

### 3.2 Advanced Analysis Tools

- `learn_project_patterns` - Discover project-specific conventions
- `detect_code_smells` - Extensible anti-pattern detection
- `generate_test_cases` - Automated test generation for transformations
- `extract_documentation` - Smart documentation extraction and formatting

### 3.3 Integration Enhancements

- IDE plugin support (VS Code, IntelliJ)
- Git hook integration for automated code quality
- CI/CD pipeline integration
- Custom rule marketplace/sharing

---

## 🏗️ Technical Architecture Evolution

### Current Architecture
```
ast-grep-mcp/
├── src/
│   ├── ast_grep_service.rs    # Main MCP service
│   ├── search.rs              # Pattern search logic
│   ├── replace.rs             # Pattern replacement logic
│   ├── rules/                 # Rule evaluation system
│   └── types.rs               # Shared type definitions
```

### Target Architecture (v0.4.0)
```
ast-grep-mcp/
├── src/
│   ├── core/                  # Core ast-grep functionality
│   ├── intelligence/          # AI/ML-powered features
│   │   ├── pattern_suggestion.rs
│   │   ├── impact_analysis.rs
│   │   ├── intent_parsing.rs
│   │   └── similarity_engine.rs
│   ├── analysis/              # Code analysis tools
│   │   ├── context_analyzer.rs
│   │   ├── relationship_mapper.rs
│   │   └── safety_verifier.rs
│   ├── learning/              # Adaptive features
│   │   ├── project_learner.rs
│   │   ├── pattern_trainer.rs
│   │   └── convention_detector.rs
│   └── integrations/          # External tool integrations
```

## 📊 Success Metrics

### Phase 1 Targets
- [ ] 80% reduction in LLM pattern-writing time
- [ ] 95% accuracy in change impact analysis
- [ ] 70% intent understanding success rate

### Phase 2 Targets
- [ ] Support for 10+ semantic similarity algorithms
- [ ] 90% success rate in multi-step refactoring
- [ ] Context analysis for 20+ programming languages

### Phase 3 Targets
- [ ] Project-specific pattern learning accuracy >85%
- [ ] Real-time code smell detection
- [ ] Automated test case generation coverage >80%

## 🤝 Community Contributions

### High-Impact Opportunities
1. **Pattern Template Library** - Community-driven pattern collection
2. **Language Support Expansion** - New Tree-sitter grammar integration
3. **ML Model Training** - Contribute training data for pattern suggestion
4. **Performance Optimization** - Algorithmic improvements for large codebases

### Getting Started
- **Documentation**: Improve examples and tutorials
- **Testing**: Add test coverage for edge cases
- **Features**: Implement Phase 1 tools using provided interfaces
- **Research**: Semantic analysis algorithms and techniques

---

## 📅 Release Schedule

| Version | Target Date | Focus Area | Key Features |
|---------|-------------|------------|--------------|
| v0.1.1 | Q1 2024 | Stability | Fix failing tests, performance improvements |
| v0.2.0 | Q2 2024 | Pattern Discovery | `suggest_patterns`, `analyze_change_impact`, `search_by_intent` |
| v0.3.0 | Q3 2024 | Semantic Analysis | `find_similar_patterns`, `analyze_code_context`, `bulk_refactor` |
| v0.4.0 | Q4 2024 | Intelligence | ML integration, project learning, advanced analysis |

This roadmap represents our commitment to transforming ast-grep MCP into the definitive tool for AI-assisted code understanding and transformation.
