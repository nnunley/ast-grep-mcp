# Design Document: `suggest_patterns` Tool

## Overview

The `suggest_patterns` tool is the highest priority feature for ast-grep-mcp v0.2.0, designed to bridge the gap between natural language descriptions and ast-grep patterns. This tool will analyze code examples and suggest matching ast-grep patterns with confidence scores, significantly reducing pattern-writing friction for LLMs.

## Problem Statement

Currently, LLMs and developers need to manually craft ast-grep patterns, which requires:
- Deep understanding of AST structure
- Knowledge of metavariable syntax
- Trial-and-error pattern refinement
- Language-specific node kind familiarity

This creates a significant barrier to adoption and slows down AI-assisted development workflows.

## Solution Design

### Core Functionality

The `suggest_patterns` tool will:
1. **Analyze code examples** to identify structural patterns
2. **Generate multiple pattern suggestions** with varying specificity
3. **Provide confidence scores** for each suggestion
4. **Offer explanations** for why patterns were suggested
5. **Support multiple languages** through Tree-sitter integration

### API Design

```json
{
  "tool_name": "suggest_patterns",
  "parameters": {
    "code_examples": [
      "function getUserData(id) { return api.get(`/users/${id}`); }",
      "function getPostData(slug) { return api.get(`/posts/${slug}`); }"
    ],
    "language": "javascript",
    "max_suggestions": 5,
    "specificity_levels": ["general", "specific", "exact"]
  }
}
```

**Response Format:**
```json
{
  "suggestions": [
    {
      "pattern": "function $NAME($PARAMS) { return api.get($URL); }",
      "confidence": 0.95,
      "specificity": "general",
      "explanation": "Matches functions that return API GET calls",
      "matching_examples": [0, 1],
      "node_kinds": ["function_declaration", "return_statement", "call_expression"]
    },
    {
      "pattern": "function get$TYPE($PARAM) { return api.get($URL); }",
      "confidence": 0.87,
      "specificity": "specific",
      "explanation": "Matches getter functions with specific naming pattern",
      "matching_examples": [0, 1],
      "node_kinds": ["function_declaration", "identifier", "return_statement"]
    }
  ],
  "language": "javascript",
  "total_suggestions": 2
}
```

### Technical Architecture

#### 1. Pattern Analysis Engine

**Core Components:**
- **AST Parser**: Uses existing Tree-sitter integration
- **Pattern Extractor**: Identifies common structural elements
- **Similarity Detector**: Finds shared patterns across examples
- **Confidence Calculator**: Scores pattern quality

**Implementation Location:** `src/pattern_analysis/`

#### 2. Suggestion Generator

**Strategy Layers:**
1. **Exact Match**: Literal patterns for identical code
2. **Structural Match**: AST-based pattern generalization
3. **Semantic Match**: Intent-based pattern suggestions
4. **Fuzzy Match**: Relaxed pattern matching

**Algorithm Flow:**
```rust
impl PatternSuggester {
    fn suggest_patterns(&self, examples: &[CodeExample]) -> Vec<PatternSuggestion> {
        let asts = self.parse_examples(examples);
        let common_structures = self.find_common_structures(&asts);
        let patterns = self.generate_patterns(&common_structures);
        self.rank_and_filter(patterns)
    }
}
```

#### 3. Confidence Scoring

**Scoring Factors:**
- **Coverage**: How many examples match the pattern
- **Precision**: How specific the pattern is
- **Complexity**: Balance between generality and usefulness
- **Language Conventions**: Adherence to language-specific patterns

**Scoring Formula:**
```
confidence = (coverage_score * 0.4) + (precision_score * 0.3) + (complexity_score * 0.2) + (convention_score * 0.1)
```

### Implementation Plan

#### Phase 1: Core Infrastructure (Week 1)
- Create `PatternSuggester` struct
- Implement AST parsing for code examples
- Build basic pattern extraction logic
- Add MCP tool integration

#### Phase 2: Pattern Generation (Week 2)
- Implement structural pattern matching
- Add metavariable generation logic
- Create confidence scoring system
- Build pattern ranking algorithms

#### Phase 3: Multi-Language Support (Week 3)
- Extend to TypeScript, Python, Rust
- Add language-specific pattern templates
- Implement language convention scoring
- Create comprehensive test suite

#### Phase 4: Advanced Features (Week 4)
- Add fuzzy matching capabilities
- Implement semantic similarity detection
- Create pattern explanation generator
- Add performance optimizations

### Integration with Existing Codebase

**Modified Files:**
- `src/ast_grep_service.rs`: Add `suggest_patterns` tool
- `src/types.rs`: Add `SuggestPatternsParam` and `SuggestPatternsResult`
- `src/pattern_analysis/mod.rs`: New module for pattern analysis
- `tests/test_suggest_patterns.rs`: Comprehensive test suite

**Dependencies:**
- Existing Tree-sitter language support
- Current AST parsing infrastructure
- Pattern matching utilities from `src/search.rs`

### Testing Strategy

**Unit Tests:**
- Pattern extraction accuracy
- Confidence scoring correctness
- Language-specific behavior
- Edge case handling

**Integration Tests:**
- End-to-end suggestion workflow
- Multi-language pattern generation
- Performance benchmarks
- MCP protocol compliance

**Test Data:**
- Curated code examples for each language
- Known good patterns for validation
- Performance benchmarks
- Real-world usage scenarios

### Performance Considerations

**Optimization Targets:**
- Pattern generation: <500ms for typical examples
- Memory usage: <50MB for large code samples
- Concurrent processing: Support multiple requests
- Caching: Store common patterns for reuse

**Scalability:**
- Streaming responses for large suggestion sets
- Incremental pattern refinement
- Background processing for complex analysis
- Resource pooling for Tree-sitter parsers

### Success Metrics

**Quantitative:**
- Pattern accuracy: >80% useful suggestions
- Response time: <1s for typical requests
- Coverage: Support for 10+ languages
- Adoption: Used in >50% of MCP interactions

**Qualitative:**
- Developer satisfaction with suggestions
- Reduction in pattern-writing time
- Improved LLM code analysis capabilities
- Enhanced ast-grep adoption

### Future Enhancements

**Phase 2 Integration:**
- Connect with `analyze_change_impact` for safer refactoring
- Link to `search_by_intent` for natural language queries
- Integrate with `find_similar_patterns` for semantic analysis

**Advanced Features:**
- Machine learning-based pattern suggestion
- User feedback integration for improvement
- Custom pattern template creation
- Integration with code completion systems

## Conclusion

The `suggest_patterns` tool represents a crucial step toward making ast-grep more accessible to LLMs and developers. By automatically generating pattern suggestions from code examples, it removes the primary barrier to adoption and enables more sophisticated AI-assisted development workflows.

This feature aligns perfectly with the project's vision of transforming ast-grep into an AI-native platform that understands intent rather than just syntax, providing the foundation for the more advanced features planned in subsequent phases.
