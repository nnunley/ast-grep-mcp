# Rule Management Example for LLMs

This example demonstrates how LLMs can create, manage, and use custom ast-grep rules through the MCP server.

## Step 1: Create a Custom Rule

Create a rule to detect unsafe TypeScript type assertions:

**Tool:** `create_rule`
**Parameters:**
```json
{
  "rule_config": "id: no-any-assertions\nlanguage: typescript\nmessage: \"Avoid 'as any' assertions - use 'as unknown' instead\"\nseverity: warning\nrule:\n  pattern: \"$EXPR as any\"\nfix: \"$EXPR as unknown\""
}
```

**Expected Response:**
```json
{
  "rule_id": "no-any-assertions",
  "file_path": ".ast-grep-rules/no-any-assertions.yaml",
  "created": true
}
```

## Step 2: List Available Rules

**Tool:** `list_rules`
**Parameters:**
```json
{
  "language": "typescript"
}
```

**Expected Response:**
```json
{
  "rules": [
    {
      "id": "no-any-assertions",
      "language": "typescript",
      "message": "Avoid 'as any' assertions - use 'as unknown' instead",
      "severity": "warning",
      "file_path": ".ast-grep-rules/no-any-assertions.yaml",
      "has_fix": true
    }
  ]
}
```

## Step 3: Test the Rule

**Tool:** `validate_rule`
**Parameters:**
```json
{
  "rule_config": "id: no-any-assertions\nlanguage: typescript\nmessage: \"Test\"\nrule:\n  pattern: \"$EXPR as any\"",
  "test_code": "const data = response as any;"
}
```

## Step 4: Apply the Rule to Find Issues

**Tool:** `rule_search`
**Parameters:**
```json
{
  "rule_config": "id: no-any-assertions\nlanguage: typescript\nmessage: \"Avoid 'as any' assertions\"\nrule:\n  pattern: \"$EXPR as any\"",
  "path_pattern": "src/**/*.ts"
}
```

## Step 5: Apply Fixes Using the Stored Rule

First, retrieve the stored rule:

**Tool:** `get_rule`
**Parameters:**
```json
{
  "rule_id": "no-any-assertions"
}
```

Then apply the fix:

**Tool:** `rule_replace`
**Parameters:**
```json
{
  "rule_config": "[YAML content from get_rule response]",
  "path_pattern": "src/**/*.ts",
  "dry_run": true
}
```

## Step 6: Update an Existing Rule

**Tool:** `create_rule`
**Parameters:**
```json
{
  "rule_config": "id: no-any-assertions\nlanguage: typescript\nmessage: \"Updated: Avoid 'as any' assertions - use 'as unknown' instead for type safety\"\nseverity: error\nrule:\n  pattern: \"$EXPR as any\"\nfix: \"$EXPR as unknown\"",
  "overwrite": true
}
```

## Step 7: Clean Up

**Tool:** `delete_rule`
**Parameters:**
```json
{
  "rule_id": "no-any-assertions"
}
```

## Common LLM Workflows

### Workflow 1: Custom Linting Rules
1. `create_rule` - Create domain-specific linting rules
2. `rule_search` - Apply rules across codebase to find issues
3. `list_rules` - Discover existing rules for similar checks
4. `rule_replace` - Apply fixes automatically

### Workflow 2: Code Modernization
1. `create_rule` - Define modernization patterns (var→const, etc.)
2. `validate_rule` - Test patterns on sample code
3. `rule_replace` - Apply transformations with dry-run first
4. `get_rule` - Retrieve rules for reuse in similar projects

### Workflow 3: Team Standards Enforcement
1. `create_rule` - Encode team coding standards as rules
2. `list_rules` - Organize rules by language/severity
3. `rule_search` - Regular scanning for compliance
4. Rule persistence allows standards to be applied consistently

## Benefits for LLMs

- **Persistent Memory**: Rules are saved between sessions
- **Reusability**: Create once, apply many times
- **Organization**: Filter and organize rules by language/severity
- **Evolution**: Update rules as standards change
- **Sharing**: Rules can be exported/imported between projects
