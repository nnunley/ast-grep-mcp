# AST-Grep Pattern Syntax Documentation

This document describes the pattern syntax used by ast-grep for code matching and transformation.

## Basic Pattern Syntax

### Metavariables

Metavariables are placeholders that match any code construct. They start with `$`:

- `$VAR` - Matches any single node (identifier, expression, etc.)
- `$_` - Anonymous metavariable (matches anything but doesn't capture)
- `$$` - Matches zero or more statements
- `$$$` - Matches zero or more nodes (including expressions)

Examples:
```javascript
// Pattern: console.log($ARG)
// Matches: console.log("hello")
//          console.log(42)
//          console.log(user.name)

// Pattern: function $NAME() { $$$ }
// Matches: function foo() { return 1; }
//          function bar() { }
//          function baz() { x = 1; y = 2; return x + y; }
```

### Named vs Anonymous Metavariables

- Named: `$VAR`, `$NAME`, `$ARG` - Can be referenced in replacements
- Anonymous: `$_` - Matches but doesn't capture

### Multi-node Metavariables

- `$$` - Matches zero or more statements (statement context)
- `$$$` - Matches zero or more items (any context)

Example:
```javascript
// Pattern: if ($COND) { $$BODY }
// Matches: if (x > 0) { console.log(x); return x; }
```

## Advanced Pattern Features

### Pattern with Constraints

Patterns can include constraints on metavariables:

```yaml
pattern:
  context: "function $FUNC($_) { $$$ }"
  constraints:
    FUNC:
      regex: "^test"  # Function name must start with "test"
```

### Nested Patterns

Patterns can match nested structures:

```javascript
// Pattern: $OBJ.map(($ITEM) => $ITEM.$PROP)
// Matches: users.map((user) => user.name)
//          items.map((item) => item.id)
```

## Pattern Context

Some patterns require specific context to parse correctly:

```yaml
pattern:
  context: "class $_ { $$$ }"  # Class context
  selector: "method_definition"  # Specific node type
```

## Special Syntax

### Ellipsis

The ellipsis (`...`) in patterns can match various constructs:

```javascript
// Pattern: foo(...$ARGS)
// Matches: foo(1, 2, 3)
//          foo(a, b)
//          foo()
```

### Type Annotations (TypeScript)

```typescript
// Pattern: function $NAME($PARAM: $TYPE): $RETURN { $$$ }
// Matches: function add(x: number): number { return x + 1; }
```

## Wildcards and Flexibility

- `$_` matches any single node
- `$$` matches any sequence of statements
- `$$$` matches any sequence of nodes

## Limitations

1. Patterns must be syntactically valid in the target language
2. Some complex syntax may require specific context
3. Whitespace and formatting are generally ignored
4. Comments are not matched by patterns

## Examples

### JavaScript Patterns

```javascript
// Match console.log calls
console.log($ARG)

// Match arrow functions
($PARAMS) => $BODY

// Match array methods
$ARRAY.$METHOD($FUNC)

// Match object destructuring
const { $PROPS } = $OBJ

// Match class methods
class $CLASS {
  $METHOD($PARAMS) {
    $$$
  }
}
```

### Common Use Cases

1. **Finding deprecated APIs**:
   ```javascript
   // Pattern: $_.componentWillMount()
   ```

2. **Identifying test files**:
   ```javascript
   // Pattern: describe($DESC, () => { $$$ })
   ```

3. **Finding TODO comments** (requires regex rule):
   ```yaml
   regex: "TODO|FIXME|HACK"
   ```

## Best Practices

1. Start with simple patterns and add complexity as needed
2. Use named metavariables for replacements
3. Test patterns on sample code before applying broadly
4. Consider edge cases and variations in coding style
5. Use `$$$` for flexible matching within blocks

## References

- [AST-Grep Documentation](https://ast-grep.github.io/)
- [Pattern Playground](https://ast-grep.github.io/playground.html)
