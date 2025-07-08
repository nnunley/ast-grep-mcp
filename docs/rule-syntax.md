# AST-Grep Rule Syntax Documentation

This document describes the rule syntax for ast-grep, which extends pattern matching with additional conditions and transformations.

## Rule Structure

A rule is defined in YAML or JSON format with the following fields:

```yaml
id: rule-identifier
message: "Description of what the rule does"
severity: error|warning|info|hint
language: javascript|typescript|python|rust|...
rule:
  # Rule conditions (see below)
fix: "Replacement template"
```

## Rule Conditions

### Pattern Rules

The most basic rule type matches a pattern:

```yaml
rule:
  pattern: console.log($ARG)
```

With advanced pattern features:
```yaml
rule:
  pattern:
    context: "function $FUNC() { $$$ }"
    selector: "function_declaration"
```

### Composite Rules

#### all
Matches when ALL sub-rules match:

```yaml
rule:
  all:
    - pattern: $VAR = $VALUE
    - inside:
        pattern: function $FUNC() { $$$ }
```

#### any
Matches when ANY sub-rule matches:

```yaml
rule:
  any:
    - pattern: var $VAR = $VALUE
    - pattern: let $VAR = $VALUE
    - pattern: const $VAR = $VALUE
```

#### not
Matches when the sub-rule does NOT match:

```yaml
rule:
  not:
    pattern: $_.toString()
```

### Relational Rules

#### inside
Matches nodes inside another pattern:

```yaml
rule:
  pattern: await $EXPR
  inside:
    kind: function_declaration
    not:
      has:
        pattern: async
```

#### has
Matches nodes that contain another pattern:

```yaml
rule:
  pattern: function $NAME($PARAMS) { $$$ }
  has:
    pattern: await $EXPR
```

#### follows
Matches nodes that follow another pattern:

```yaml
rule:
  pattern: $VAR = null
  follows:
    pattern: $VAR.close()
```

#### precedes
Matches nodes that precede another pattern:

```yaml
rule:
  pattern: const $VAR = require($MODULE)
  precedes:
    pattern: $VAR.$METHOD()
```

### Atomic Rules

#### kind
Matches nodes by their AST node type:

```yaml
rule:
  kind: arrow_function
```

#### regex
Matches nodes by regular expression:

```yaml
rule:
  regex: "TODO|FIXME"
```

#### matches
Reference to another rule:

```yaml
rule:
  matches: no-console-rule
```

## Fix Templates

The `fix` field specifies how to transform matched code:

```yaml
rule:
  pattern: $OBJ == null
fix: "$OBJ === null"
```

With multiple metavariables:
```yaml
rule:
  pattern: parseInt($STR)
fix: "parseInt($STR, 10)"
```

## Complex Rule Examples

### Async Function Detection

```yaml
id: require-async-await
message: "Async functions must use await"
rule:
  all:
    - kind: function_declaration
    - has:
        pattern: async
    - not:
        has:
          pattern: await $EXPR
```

### Deprecated API

```yaml
id: no-component-will-mount
message: "componentWillMount is deprecated"
rule:
  pattern:
    context: |
      class $CLASS extends React.Component {
        $$$
      }
    selector: method_definition[name="componentWillMount"]
fix: |
  componentDidMount() {
    // TODO: Migrate componentWillMount logic
    $$$
  }
```

### Security Rule

```yaml
id: no-eval
message: "Avoid using eval() for security reasons"
severity: error
rule:
  pattern: eval($CODE)
```

### Code Style Rule

```yaml
id: prefer-const
message: "Use const for variables that are never reassigned"
rule:
  all:
    - pattern: let $VAR = $VALUE
    - not:
        follows:
          pattern: $VAR = $NEWVALUE
fix: "const $VAR = $VALUE"
```

## Rule Constraints

Rules can include constraints on metavariables:

```yaml
rule:
  pattern: $FUNC($ARG)
  where:
    FUNC:
      regex: "^(eval|setTimeout|setInterval)$"
    ARG:
      kind: string_literal
```

## Rule Composition

Rules can reference other rules:

```yaml
id: complex-security-check
rule:
  all:
    - any:
        - matches: no-eval
        - matches: no-dynamic-require
    - inside:
        pattern: |
          if ($USER.role === 'admin') {
            $$$
          }
```

## Best Practices

1. **Clear Messages**: Write descriptive messages explaining why the rule exists
2. **Appropriate Severity**: Use error for bugs, warning for code smells
3. **Precise Patterns**: Make patterns specific to avoid false positives
4. **Safe Fixes**: Ensure automated fixes don't break code
5. **Test Coverage**: Test rules against various code examples

## Common Patterns

### Finding Anti-patterns

```yaml
rule:
  all:
    - pattern: if ($COND) { return true } else { return false }
fix: "return $COND"
```

### Enforcing Conventions

```yaml
rule:
  pattern: test($NAME, $FUNC)
  where:
    NAME:
      not:
        regex: "^should "
message: "Test names should start with 'should'"
```

### Migration Rules

```yaml
rule:
  pattern: import $NAMES from 'old-library'
fix: "import $NAMES from 'new-library'"
```

## Limitations

1. Rules operate on AST nodes, not runtime values
2. Cannot track variable values across scope boundaries
3. Limited support for type information
4. Some language-specific constructs may not be fully supported

## References

- [AST-Grep Rules Documentation](https://ast-grep.github.io/guide/rule-config.html)
- [Rule Examples](https://ast-grep.github.io/catalog/)
