# Language Injection - Remaining Work

## Current Status

✅ **Completed:**
- Basic automatic language injection for HTML/JS/CSS
- Detection based on file extension + pattern language
- Integration with search and file_search methods
- Tests for basic HTML/JS scenarios
- Documentation updates

## Remaining Work

### 1. Fix Extraction Patterns
The current extraction patterns are too simple and may not handle:
- Script tags with attributes: `<script type="text/javascript">`
- Script tags with src attribute (should be skipped)
- Inline event handlers: `onclick="..."`
- CSS in style attributes: `style="color: red"`
- Multi-line script/style tags with proper indentation

### 2. Improve CSS Extraction
- Fix CSS extraction pattern to handle `<style>` tags with attributes
- Add support for CSS-in-JS patterns (styled-components, emotion)
- Handle scoped styles in Vue components

### 3. Add More Language Combinations
- Python with embedded SQL (in string literals)
- JavaScript with embedded GraphQL (in template literals)
- Markdown with embedded code blocks
- PHP with embedded HTML/JS

### 4. Handle Edge Cases
- Empty script/style tags
- Malformed HTML
- CDATA sections in XML/HTML
- Comments within embedded code blocks
- Multiple script/style tags in one file

### 5. Performance Optimization
- Cache extraction results for repeated searches
- Avoid unnecessary async runtime creation
- Batch process multiple embedded blocks

### 6. Configuration Support
- Allow custom language injection rules via sgconfig.yml
- Support for project-specific extraction patterns
- Override automatic detection when needed

### 7. Better Error Handling
- Graceful fallback when extraction fails
- Clear error messages for unsupported combinations
- Warnings for potentially missed embedded code

### 8. Additional Tests Needed
- Test script tags with attributes
- Test multiple script blocks
- Test error cases
- Test performance with large files
- Test Vue and JSX scenarios
- Test CSS-in-JS patterns

## Example Improvements Needed

### Script Tag Variations
```html
<!-- Currently works -->
<script>console.log('test')</script>

<!-- Needs to work -->
<script type="text/javascript">console.log('test')</script>
<script type="module">import { x } from './y.js'</script>
<script src="external.js"></script> <!-- Should be skipped -->
```

### CSS Variations
```html
<!-- Currently works -->
<style>body { color: red; }</style>

<!-- Needs to work -->
<style type="text/css">body { color: red; }</style>
<style scoped>body { color: red; }</style>
<div style="color: red;">Inline CSS</div>
```

### Better Extraction Patterns
```rust
// Current pattern (too simple)
extraction_pattern: "<script>$JS_CODE</script>"

// Better pattern (handles attributes)
extraction_pattern: "<script$ATTRS>$JS_CODE</script>"
// With additional logic to check ATTRS doesn't contain src=

// Or use AST-based extraction with proper node types
selector: "script_element"
// Then extract text content from the node
```

## Integration with sgconfig.yml

Add support for custom language injections:
```yaml
languageInjections:
  - hostLanguage: python
    rule:
      pattern: |
        $SQL = """
        $QUERY
        """
    injected: sql

  - hostLanguage: javascript
    rule:
      pattern: gql`$QUERY`
    injected: graphql
```

This would allow users to define their own embedded language patterns for their specific use cases.
