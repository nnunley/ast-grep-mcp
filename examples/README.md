# AST-Grep MCP Rule Examples

This directory contains example rule configurations and test files to demonstrate the rule-based functionality of the ast-grep MCP server.

## Files

### Demo Files
- `rule_demo.js` - Example JavaScript file with patterns to match and transform
- `modernize_js.yaml` - Rule to convert `var` declarations to `const`
- `replace_console_log.yaml` - Rule to replace `console.log` with `console.debug`
- `composite_rule.yaml` - Example composite rule using `any` operator

## Testing the Rules

### 1. Validate a Rule Configuration

```bash
# Test rule syntax and validate against sample code
# (This would be done through the MCP client)
```

**Tool:** `validate_rule`
**Parameters:**
```json
{
  "rule_config": "id: test\nlanguage: javascript\nrule:\n  pattern: \"console.log($ARG)\"",
  "test_code": "console.log('hello');"
}
```

### 2. Search Using Rules

**Tool:** `rule_search`
**Parameters:**
```json
{
  "rule_config": "id: find-console-log\nlanguage: javascript\nmessage: \"Found console.log usage\"\nseverity: warning\nrule:\n  pattern: \"console.log($ARG)\"",
  "path_pattern": "examples/**/*.js"
}
```

### 3. Replace Using Rules

**Tool:** `rule_replace`
**Parameters:**
```json
{
  "rule_config": "id: modernize-var\nlanguage: javascript\nmessage: \"Modernize var to const\"\nrule:\n  pattern: \"var $NAME = $VALUE\"\nfix: \"const $NAME = $VALUE\"",
  "path_pattern": "examples/**/*.js",
  "dry_run": true
}
```

## Rule Configuration Format

### Basic Pattern Rule (YAML)
```yaml
id: unique-rule-identifier
language: javascript
message: "Human-readable description of what was found"
severity: warning  # info, warning, error
rule:
  pattern: "console.log($ARG)"
fix: "console.debug($ARG)"  # Optional: for replacements
```

### Basic Pattern Rule (JSON)
```json
{
  "id": "unique-rule-identifier",
  "language": "javascript",
  "message": "Human-readable description of what was found",
  "severity": "warning",
  "rule": {
    "pattern": "console.log($ARG)"
  },
  "fix": "console.debug($ARG)"
}
```

### Composite Rule Example
```yaml
id: find-any-console
language: javascript
message: "Found console method usage"
rule:
  any:
    - pattern: "console.log($ARG)"
    - pattern: "console.error($ARG)"
    - pattern: "console.warn($ARG)"
```

## Supported Features

### ✅ Implemented
- Basic pattern matching with meta-variables (`$VAR`)
- YAML and JSON rule configuration parsing
- Rule validation with optional test code
- File-based search and replacement
- Simple composite rules (`all`, `any`, `not`) - limited support
- Dry-run and summary modes for replacements
- Pattern caching for performance

### 🚧 Limited Support
- Composite rules (uses first pattern only)
- Complex pattern strictness levels

### ⏳ Planned
- Full composite rule evaluation
- Relational rules (`inside`, `has`, `follows`, `precedes`)
- Rule template management
- Batch rule execution

## Pattern Syntax

The pattern syntax follows ast-grep conventions:

- `$VAR` - Captures a single AST node
- `$$$STATEMENTS` - Captures multiple statements
- `$FUNC($ARGS)` - Captures function calls with arguments
- Exact text matching for keywords and operators

For more details, see the [ast-grep pattern guide](https://ast-grep.github.io/guide/pattern-syntax.html).