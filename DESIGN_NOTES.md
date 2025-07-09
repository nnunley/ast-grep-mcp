# Design Notes: Duplicate Rule ID Handling

## Official Documentation

The ast-grep documentation states that rule `id` should be "a unique short string for the rule" but does not specify what happens when uniqueness is violated. The behavior with duplicate IDs is undocumented.

## ast-grep CLI Behavior (Empirically Verified)

1. **Rule Loading**: ast-grep loads ALL rules, including those with duplicate IDs
2. **Rule Execution**: When scanning, ALL rules with the same ID are executed
3. **Different Patterns**: Rules with the same ID can have completely different patterns and all will be applied
4. **Reporting**: Each match is reported with its specific message/severity from the matching rule

Example:
- `rules1/console-check.yml` - matches `console.log()` with error severity
- `rules2/console-check.yml` - matches `console.error()` with warning severity
- Both are applied and reported separately

## Current Implementation Behavior

Our implementation currently follows a "first wins" strategy:
- Only the first rule with a given ID is kept
- Subsequent rules with the same ID are ignored
- This is DIFFERENT from ast-grep's behavior

## Design Considerations

### For Listing Rules
- **Option 1**: Show all rules including duplicates (matches ast-grep)
- **Option 2**: Show only unique rule IDs (current implementation)

### For Getting Rule by ID
- **Option 1**: Return the first matching rule
- **Option 2**: Return all rules with that ID
- **Option 3**: Return an error if duplicates exist

### For Rule Execution (search/replace)
- **Option 1**: Apply all rules with the same ID (matches ast-grep)
- **Option 2**: Apply only the first rule

## Recommendation

To match ast-grep behavior, we should:
1. Load and keep ALL rules, including duplicates
2. When listing, show all instances (maybe with file path to distinguish)
3. When getting by ID, either return all or document that it returns the first
4. When executing rules, apply ALL rules with matching IDs

This would require changes to:
- RuleStorage: Remove duplicate filtering
- RuleInfo: Maybe add a way to distinguish instances
- Rule execution: Apply all matching rules
