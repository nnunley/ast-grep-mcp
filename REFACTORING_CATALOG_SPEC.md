# Refactoring Catalog Specification for ast-grep

## Overview

This specification defines a catalog of refactoring patterns that can be implemented using ast-grep's pattern matching and transformation capabilities. Each refactoring is designed to be token-efficient for LLM usage while providing powerful code transformation capabilities.

## Core Concepts

### Pattern-Based Refactoring
Instead of coordinate-based selection, refactorings work by:
1. LLM provides a code pattern example
2. ast-grep finds all matching instances
3. Transformations are applied consistently across matches
4. Results are summarized to minimize token usage

### Refactoring Definition Structure

```yaml
id: unique_refactoring_id
name: Human-readable name
category: composing_methods|organizing_data|simplifying_conditionals|...
description: What this refactoring does
supported_languages: [javascript, typescript, python, rust, go, java]
complexity: simple|moderate|complex
pattern:
  # ast-grep pattern syntax
  match: |
    $PATTERN
  # Optional constraints
  constraints:
    - has: identifier: $VAR
    - inside: function_definition
transform:
  # Transformation rules
  replace: |
    $NEW_CODE
  # Optional extracted code
  extract:
    type: function|method|variable|class
    template: |
      $EXTRACTED_TEMPLATE
    placement: before|after|end_of_scope|end_of_file
variables:
  # Variable extraction and scope analysis
  extract_from_pattern:
    - $VAR
    - $ITEMS
  parameters: auto|manual|[$PARAM1, $PARAM2]
  return_values: auto|none|[$RETURN]
preconditions:
  - no_side_effects_in: $EXPRESSION
  - unique_name: $FUNCTION_NAME
  - valid_scope: $PATTERN
```

## Refactoring Catalog

### 1. Extract Method/Function

```yaml
id: extract_method
name: Extract Method
category: composing_methods
description: Extract repeated code blocks into a reusable method
supported_languages: [javascript, typescript, python, rust, go, java]
complexity: moderate
variants:
  - id: extract_loop_calculation
    pattern:
      match: |
        $INIT
        for ($ITEM in $ITEMS) {
          $BODY
        }
        $USE_RESULT
    transform:
      replace: |
        const $RESULT = $FUNCTION_NAME($ITEMS);
        $USE_RESULT
      extract:
        type: function
        template: |
          function $FUNCTION_NAME($ITEMS) {
            $INIT
            for ($ITEM in $ITEMS) {
              $BODY
            }
            return $RESULT;
          }
        placement: before_current_function

  - id: extract_conditional_logic
    pattern:
      match: |
        if ($CONDITION) {
          $TRUE_BRANCH
        } else {
          $FALSE_BRANCH
        }
    transform:
      replace: |
        $RESULT = $FUNCTION_NAME($PARAMS);
      extract:
        type: function
        template: |
          function $FUNCTION_NAME($PARAMS) {
            if ($CONDITION) {
              $TRUE_BRANCH
            } else {
              $FALSE_BRANCH
            }
          }
```

### 2. Extract Variable

```yaml
id: extract_variable
name: Extract Variable
category: composing_methods
description: Replace complex expressions with descriptive variables
supported_languages: [javascript, typescript, python, rust, go, java]
complexity: simple
variants:
  - id: extract_complex_expression
    pattern:
      match: |
        $COMPLEX_EXPR
    constraints:
      - inside: 
          any: [expression_statement, return_statement, assignment]
      - not:
          matches: identifier
    transform:
      replace: |
        $VARIABLE_NAME
      extract:
        type: variable
        template: |
          const $VARIABLE_NAME = $COMPLEX_EXPR;
        placement: before_usage
```

### 3. Inline Variable

```yaml
id: inline_variable
name: Inline Variable
category: composing_methods
description: Replace variable references with its value
supported_languages: [javascript, typescript, python, rust, go, java]
complexity: simple
pattern:
  match: |
    $DECL = $VALUE;
    $$$
    $USAGE
  constraints:
    - follows:
        declaration: $DECL
    - uses: $DECL
    - single_assignment: $DECL
transform:
  replace: |
    $$$
    $VALUE
```

### 4. Rename Symbol

```yaml
id: rename_symbol
name: Rename Symbol
category: organizing_code
description: Rename variables, functions, or classes consistently
supported_languages: all
complexity: moderate
pattern:
  match: |
    $OLD_NAME
  constraints:
    - kind: [identifier, function_name, class_name]
transform:
  replace: |
    $NEW_NAME
  scope_analysis:
    - find_declaration: $OLD_NAME
    - find_all_references: $OLD_NAME
    - check_conflicts: $NEW_NAME
```

### 5. Replace Conditional with Guard Clause

```yaml
id: replace_conditional_with_guard
name: Replace Conditional with Guard Clause
category: simplifying_conditionals
description: Replace nested conditionals with early returns
supported_languages: [javascript, typescript, python, rust, go, java]
complexity: simple
pattern:
  match: |
    function $FUNC($PARAMS) {
      if ($CONDITION) {
        $MAIN_LOGIC
      } else {
        $EARLY_RETURN
      }
    }
transform:
  replace: |
    function $FUNC($PARAMS) {
      if (!($CONDITION)) {
        $EARLY_RETURN
      }
      $MAIN_LOGIC
    }
```

### 6. Extract Class

```yaml
id: extract_class
name: Extract Class
category: organizing_data
description: Extract related methods and data into a new class
supported_languages: [javascript, typescript, python, java]
complexity: complex
pattern:
  match: |
    class $ORIGINAL_CLASS {
      $FIELD1
      $FIELD2
      $$$
      $METHOD1
      $METHOD2
    }
  constraints:
    - related_fields: [$FIELD1, $FIELD2]
    - related_methods: [$METHOD1, $METHOD2]
transform:
  replace: |
    class $ORIGINAL_CLASS {
      $EXTRACTED_CLASS_FIELD
      $$$
      $DELEGATING_METHODS
    }
  extract:
    type: class
    template: |
      class $NEW_CLASS {
        $FIELD1
        $FIELD2
        
        $METHOD1
        $METHOD2
      }
```

### 7. Replace Loop with Pipeline

```yaml
id: replace_loop_with_pipeline
name: Replace Loop with Pipeline
category: composing_methods
description: Replace imperative loops with functional pipeline
supported_languages: [javascript, typescript, python]
complexity: moderate
variants:
  - id: filter_map_reduce
    pattern:
      match: |
        $RESULT = $INIT;
        for ($ITEM of $ITEMS) {
          if ($FILTER_CONDITION) {
            $RESULT = $ACCUMULATOR;
          }
        }
    transform:
      replace: |
        $RESULT = $ITEMS
          .filter($ITEM => $FILTER_CONDITION)
          .reduce((acc, $ITEM) => $ACCUMULATOR, $INIT);
```

### 8. Consolidate Duplicate Conditional Fragments

```yaml
id: consolidate_duplicate_conditional
name: Consolidate Duplicate Conditional Fragments
category: simplifying_conditionals
description: Move duplicate code outside of conditional
supported_languages: all
complexity: simple
pattern:
  match: |
    if ($CONDITION) {
      $DUPLICATE_BEFORE
      $UNIQUE1
      $DUPLICATE_AFTER
    } else {
      $DUPLICATE_BEFORE
      $UNIQUE2
      $DUPLICATE_AFTER
    }
transform:
  replace: |
    $DUPLICATE_BEFORE
    if ($CONDITION) {
      $UNIQUE1
    } else {
      $UNIQUE2
    }
    $DUPLICATE_AFTER
```

### 9. Replace Magic Number with Constant

```yaml
id: replace_magic_number
name: Replace Magic Number with Constant
category: organizing_data
description: Replace hard-coded numbers with named constants
supported_languages: all
complexity: simple
pattern:
  match: |
    $NUMBER
  constraints:
    - kind: number_literal
    - not_in: [const_declaration, enum_declaration]
    - value_not_in: [0, 1, -1]  # Common non-magic numbers
transform:
  replace: |
    $CONSTANT_NAME
  extract:
    type: constant
    template: |
      const $CONSTANT_NAME = $NUMBER;
    placement: top_of_scope
```

### 10. Introduce Parameter Object

```yaml
id: introduce_parameter_object
name: Introduce Parameter Object
category: organizing_data
description: Replace multiple parameters with a single object
supported_languages: [javascript, typescript, python]
complexity: moderate
pattern:
  match: |
    function $FUNC($PARAM1, $PARAM2, $PARAM3, $$$MORE_PARAMS) {
      $BODY
    }
  constraints:
    - min_params: 4
    - related_params: [$PARAM1, $PARAM2, $PARAM3]
transform:
  replace: |
    function $FUNC($PARAM_OBJECT) {
      const { $PARAM1, $PARAM2, $PARAM3 } = $PARAM_OBJECT;
      $BODY
    }
  update_calls:
    match: |
      $FUNC($ARG1, $ARG2, $ARG3, $$$MORE_ARGS)
    replace: |
      $FUNC({ $PARAM1: $ARG1, $PARAM2: $ARG2, $PARAM3: $ARG3 })
```

## Implementation Guidelines

### Token Efficiency
1. **Pattern Matching**: Return only match count and file locations, not full content
2. **Batch Operations**: Apply transformations to all matches in one operation
3. **Summary Responses**: Provide concise summaries of changes made
4. **Preview Mode**: Show what would change without full file content

### MCP Tool Interface

```typescript
interface RefactoringRequest {
  refactoring_id: string;
  pattern_example?: string;  // Optional: override default pattern
  options?: {
    function_name?: string;
    variable_name?: string;
    class_name?: string;
    scope?: 'file' | 'directory' | 'project';
    preview?: boolean;
  };
}

interface RefactoringResponse {
  matches_found: number;
  files_affected: string[];
  changes_preview?: {
    total_lines_affected: number;
    example_transformation: string;  // One example, not all
  };
  applied?: boolean;
  error?: string;
}
```

### Language-Specific Considerations

#### Python
- Indentation-based blocks require special handling
- `self` parameter in methods
- Type hints in function signatures
- Decorators preservation

#### JavaScript/TypeScript
- Arrow functions vs regular functions
- `this` binding considerations
- Async/await handling
- Type annotations (TypeScript)

#### Rust
- Ownership and borrowing rules
- Lifetime annotations
- Method vs associated function
- Error handling patterns

### Precondition Checking
1. **Scope Analysis**: Ensure variables are accessible
2. **Name Conflicts**: Check for existing names in scope
3. **Side Effects**: Detect and handle side effects in extracted code
4. **Type Compatibility**: Ensure type safety (where applicable)

## Usage Examples

### Example 1: Extract Method
```yaml
# LLM provides pattern
request:
  refactoring_id: extract_method
  pattern_example: |
    let total = 0;
    for (const item of items) {
      total += item.price * item.quantity;
    }
  options:
    function_name: calculateTotal
    scope: project

# Service response
response:
  matches_found: 23
  files_affected: ["src/cart.js", "src/order.js", "src/invoice.js"]
  changes_preview:
    total_lines_affected: 92
    example_transformation: |
      - let total = 0;
      - for (const item of items) {
      -   total += item.price * item.quantity;
      - }
      + const total = calculateTotal(items);
```

### Example 2: Rename Symbol
```yaml
request:
  refactoring_id: rename_symbol
  pattern_example: "getUserInfo"
  options:
    new_name: "fetchUserProfile"
    scope: project

response:
  matches_found: 45
  files_affected: ["api/users.js", "components/Profile.js", ...]
  changes_preview:
    total_lines_affected: 45
    example_transformation: |
      - const data = await getUserInfo(userId);
      + const data = await fetchUserProfile(userId);
```

## Future Enhancements

1. **Composite Refactorings**: Chain multiple refactorings together
2. **AI-Suggested Refactorings**: Analyze code and suggest applicable refactorings
3. **Custom Pattern Definition**: Allow users to define their own refactoring patterns
4. **Cross-File Refactorings**: Handle imports, exports, and module dependencies
5. **Refactoring History**: Track and potentially undo refactorings
6. **Performance Optimization**: Suggest performance-improving refactorings
7. **Code Smell Detection**: Identify and suggest fixes for common code smells