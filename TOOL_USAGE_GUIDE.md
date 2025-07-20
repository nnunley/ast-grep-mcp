# AST-Grep MCP Tool Usage Guide for LLMs

This guide explains how to effectively use each tool in the AST-Grep MCP server for code analysis and transformation.

## Quick Reference

### Core Search Tools
- **`search`** - Search patterns in code strings
- **`file_search`** - Search patterns across files using glob patterns
- **`generate_ast`** - View AST structure to understand node types

### Core Replace Tools
- **`replace`** - Replace patterns in code strings
- **`file_replace`** - Replace patterns across files (bulk operations)

### Rule-Based Tools (Advanced)
- **`rule_search`** - Search using YAML rule configurations
- **`rule_replace`** - Replace using YAML rule configurations
- **`validate_rule`** - Test and validate rule configurations

### Rule Management
- **`create_rule`** - Store rule configurations for reuse
- **`list_rules`** - List stored rules
- **`get_rule`** - Retrieve stored rule by ID
- **`delete_rule`** - Delete stored rule

### Utility
- **`list_languages`** - Get supported programming languages

## Pattern Syntax Guide

### Basic Patterns
- **Exact match**: `console.log("hello")`
- **Single capture**: `console.log($MSG)` - captures one argument
- **Multiple capture**: `console.log($$$ARGS)` - captures multiple arguments
- **Any node**: `$_` - matches any single node

### Common Pattern Examples

#### JavaScript/TypeScript
```javascript
// Function declarations
function $NAME($PARAMS) { $$$BODY }

// Function calls
$OBJ.$METHOD($$$ARGS)

// Variable declarations
const $VAR = $VALUE

// Import statements
import $IMPORT from "$MODULE"

// Class definitions
class $NAME extends $PARENT { $$$BODY }
```

#### Python
```python
# Function definitions
def $NAME($PARAMS): $$$BODY

# Class definitions
class $NAME($PARENT): $$$BODY

# Import statements
from $MODULE import $IMPORTS

# Variable assignments
$VAR = $VALUE
```

#### Rust
```rust
// Function definitions
fn $NAME($PARAMS) -> $RETURN { $$$BODY }

// Struct definitions
struct $NAME { $$$FIELDS }

// Implementation blocks
impl $TRAIT for $TYPE { $$$METHODS }
```

## Tool Usage Patterns

### 1. Exploratory Analysis

**Step 1: Check supported languages**
```json
{
  "tool": "list_languages"
}
```

**Step 2: Understand code structure**
```json
{
  "tool": "generate_ast",
  "code": "function example() { return 42; }",
  "language": "javascript"
}
```

**Step 3: Search for patterns**
```json
{
  "tool": "search",
  "code": "function example() { return 42; }",
  "pattern": "function $NAME() { $$$BODY }",
  "language": "javascript"
}
```

### 2. Codebase Analysis

**Find all function calls to a specific method:**
```json
{
  "tool": "file_search",
  "path_pattern": "**/*.js",
  "pattern": "console.log($$$ARGS)",
  "language": "javascript",
  "max_results": 50
}
```

**Find functions with specific patterns:**
```json
{
  "tool": "file_search",
  "path_pattern": "src/**/*.{ts,tsx}",
  "pattern": "function $NAME($PARAMS) { $$$BODY }",
  "language": "typescript",
  "context_lines": 2
}
```

### 3. Code Refactoring

**Simple replacement:**
```json
{
  "tool": "replace",
  "code": "console.log('debug message')",
  "pattern": "console.log($MSG)",
  "replacement": "console.debug($MSG)",
  "language": "javascript"
}
```

**Bulk file replacement (preview first):**
```json
{
  "tool": "file_replace",
  "path_pattern": "**/*.js",
  "pattern": "console.log($MSG)",
  "replacement": "logger.info($MSG)",
  "language": "javascript",
  "dry_run": true,
  "summary_only": true
}
```

**Execute bulk replacement:**
```json
{
  "tool": "file_replace",
  "path_pattern": "**/*.js",
  "pattern": "console.log($MSG)",
  "replacement": "logger.info($MSG)",
  "language": "javascript",
  "dry_run": false,
  "summary_only": false
}
```

### 4. Advanced Rule-Based Operations

**Create a complex rule:**
```yaml
id: remove-unused-imports
language: javascript
rule:
  pattern: import $NAME from "$MODULE"
  constraints:
    $NAME:
      regex: "^[A-Z]"
fix: |
  // $NAME removed as unused
```

**Use the rule:**
```json
{
  "tool": "validate_rule",
  "rule_config": "id: remove-unused-imports\nlanguage: javascript\nrule:\n  pattern: import $NAME from \"$MODULE\"\nfix: |\n  // Import removed",
  "test_code": "import React from 'react';"
}
```

**Store rule for reuse:**
```json
{
  "tool": "create_rule",
  "rule_config": "id: remove-unused-imports\nlanguage: javascript\nrule:\n  pattern: import $NAME from \"$MODULE\"\nfix: |\n  // Import removed"
}
```

## Best Practices

### 1. Start Simple
- Begin with `search` on small code samples
- Use `generate_ast` to understand node structure
- Test patterns before applying to entire codebase

### 2. Use Context Lines
- Add `context_lines: 2` for better understanding
- Helpful for reviewing matches before replacement

### 3. Safe Bulk Operations
- Always use `dry_run: true` first
- Use `summary_only: true` for large operations
- Check `include_samples: true` to see examples

### 4. Pagination for Large Results
- Use `max_results` to limit output
- Implement cursor-based pagination for large codebases
- Monitor `max_file_size` to avoid memory issues

### 5. Language-Specific Considerations
- Check exact language names with `list_languages`
- Some languages have specific syntax requirements
- Tree-sitter grammar differences affect pattern matching

## Common Pitfalls

### 1. Pattern Matching Issues
- **Problem**: Pattern doesn't match expected code
- **Solution**: Use `generate_ast` to see actual AST structure
- **Example**: `console.log($MSG)` might need `console.log($$$ARGS)` for multiple arguments

### 2. Language Specification
- **Problem**: Wrong language causes no matches
- **Solution**: Use exact language names from `list_languages`
- **Example**: Use "typescript" not "ts", "javascript" not "js"

### 3. Greedy vs Specific Patterns
- **Problem**: Too many false positives
- **Solution**: Make patterns more specific
- **Example**: `$VAR = $VALUE` vs `const $VAR = $VALUE`

### 4. File Size Limits
- **Problem**: Large files skipped
- **Solution**: Adjust `max_file_size` parameter
- **Default**: 1MB limit for performance

### 5. Complex Replacement Logic
- **Problem**: Simple patterns can't handle complex transformations
- **Solution**: Use rule-based tools with YAML configurations
- **Example**: Conditional replacements, context-aware changes

## Error Handling

### Common Error Types
1. **Syntax Errors**: Invalid pattern syntax
2. **Language Errors**: Unsupported or misspelled language
3. **File Access**: Permission or file not found errors
4. **Resource Limits**: File too large or too many results

### Debugging Steps
1. Test pattern with `search` on small code sample
2. Use `generate_ast` to verify AST structure
3. Check language with `list_languages`
4. Validate rules with `validate_rule`
5. Start with small file sets before bulk operations

## Examples by Use Case

### Code Quality Analysis
```json
// Find all TODO comments
{
  "tool": "file_search",
  "path_pattern": "**/*.{js,ts,py}",
  "pattern": "// TODO: $MESSAGE",
  "language": "javascript"
}

// Find functions without return statements
{
  "tool": "file_search",
  "path_pattern": "**/*.js",
  "pattern": "function $NAME($PARAMS) { $$$BODY }",
  "language": "javascript"
}
```

### Security Analysis
```json
// Find potential SQL injection
{
  "tool": "file_search",
  "path_pattern": "**/*.js",
  "pattern": "query($SQL)",
  "language": "javascript"
}

// Find hardcoded credentials
{
  "tool": "file_search",
  "path_pattern": "**/*.py",
  "pattern": "password = \"$PASS\"",
  "language": "python"
}
```

### Migration Tasks
```json
// Update deprecated API calls
{
  "tool": "file_replace",
  "path_pattern": "**/*.js",
  "pattern": "oldAPI.$METHOD($$$ARGS)",
  "replacement": "newAPI.$METHOD($$$ARGS)",
  "language": "javascript",
  "dry_run": true
}
```

This guide should help LLMs understand how to effectively use the AST-Grep MCP tools for various code analysis and transformation tasks.
