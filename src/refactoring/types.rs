//! # Refactoring System Types
//!
//! This module contains all type definitions for the refactoring system,
//! including request/response types and internal data structures.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Request parameters for performing a refactoring operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactoringRequest {
    /// The ID of the refactoring to perform (e.g., "extract_method", "rename_symbol")
    pub refactoring_id: String,
    
    /// Optional pattern example to override the default pattern
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern_example: Option<String>,
    
    /// Options for the refactoring operation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<RefactoringOptions>,
}

/// Options for customizing refactoring behavior
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RefactoringOptions {
    /// Name for extracted function/method
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_name: Option<String>,
    
    /// Name for extracted variable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variable_name: Option<String>,
    
    /// Name for extracted class
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    
    /// New name for rename operations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_name: Option<String>,
    
    /// Scope of the refactoring operation
    #[serde(default = "default_scope")]
    pub scope: RefactoringScope,
    
    /// Whether to preview changes without applying them
    #[serde(default = "default_true")]
    pub preview: bool,
    
    /// Maximum number of matches to process
    #[serde(default = "default_max_matches")]
    pub max_matches: usize,
    
    /// Path pattern for file-based refactoring
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_pattern: Option<String>,
    
    /// Programming language
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// Scope of refactoring operation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RefactoringScope {
    /// Apply to single file
    File,
    /// Apply to directory and subdirectories
    Directory,
    /// Apply to entire project
    Project,
}

impl Default for RefactoringScope {
    fn default() -> Self {
        RefactoringScope::File
    }
}

/// Response from a refactoring operation
#[derive(Debug, Serialize, Deserialize)]
pub struct RefactoringResponse {
    /// Number of matches found for the pattern
    pub matches_found: usize,
    
    /// Files that would be or were affected
    pub files_affected: Vec<String>,
    
    /// Preview of changes (when preview=true or always in token-efficient mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changes_preview: Option<ChangesPreview>,
    
    /// Whether changes were actually applied
    pub applied: bool,
    
    /// Error message if refactoring failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    
    /// Warnings about potential issues
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<String>>,
}

/// Preview of changes for token-efficient display
#[derive(Debug, Serialize, Deserialize)]
pub struct ChangesPreview {
    /// Total number of lines affected across all files
    pub total_lines_affected: usize,
    
    /// Example transformation (one representative example)
    pub example_transformation: String,
    
    /// Summary of changes by type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_summary: Option<HashMap<String, usize>>,
}

/// Internal representation of a refactoring definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactoringDefinition {
    /// Unique identifier for the refactoring
    pub id: String,
    
    /// Human-readable name
    pub name: String,
    
    /// Category of refactoring
    pub category: RefactoringCategory,
    
    /// Description of what this refactoring does
    pub description: String,
    
    /// Languages this refactoring supports
    pub supported_languages: Vec<String>,
    
    /// Complexity level
    pub complexity: RefactoringComplexity,
    
    /// Pattern definition
    pub pattern: PatternDefinition,
    
    /// Transformation rules
    pub transform: TransformDefinition,
    
    /// Variable extraction and scope analysis
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<VariableDefinition>,
    
    /// Preconditions that must be met
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preconditions: Option<Vec<Precondition>>,
    
    /// Variants of this refactoring
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variants: Option<Vec<RefactoringVariant>>,
}

/// Category of refactoring operation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RefactoringCategory {
    ComposingMethods,
    OrganizingData,
    SimplifyingConditionals,
    OrganizingCode,
}

/// Complexity level of refactoring
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RefactoringComplexity {
    Simple,
    Moderate,
    Complex,
}

/// Pattern definition for matching code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternDefinition {
    /// The ast-grep pattern to match
    pub r#match: String,
    
    /// Optional constraints on the pattern
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraints: Option<Vec<PatternConstraint>>,
}

/// Constraint on pattern matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternConstraint {
    /// Type of constraint
    #[serde(flatten)]
    pub constraint_type: ConstraintType,
}

/// Types of constraints that can be applied to patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintType {
    /// Pattern must have a specific identifier
    Has { identifier: String },
    
    /// Pattern must be inside a specific context
    Inside { context: String },
    
    /// Pattern must follow another pattern
    Follows { declaration: String },
    
    /// Pattern must use a specific identifier
    Uses { identifier: String },
    
    /// Identifier must have single assignment
    SingleAssignment { identifier: String },
    
    /// Minimum number of parameters
    MinParams { count: usize },
    
    /// Related parameters that should be grouped
    RelatedParams { params: Vec<String> },
    
    /// Pattern must not match something
    Not { matches: String },
    
    /// Pattern must match a specific kind
    Kind { kinds: Vec<String> },
    
    /// Value must not be in list
    ValueNotIn { values: Vec<String> },
    
    /// Pattern must not be in specific contexts
    NotIn { contexts: Vec<String> },
}

/// Transformation definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformDefinition {
    /// Replacement pattern
    pub replace: String,
    
    /// Optional code extraction
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extract: Option<ExtractDefinition>,
    
    /// Scope analysis requirements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_analysis: Option<Vec<ScopeAnalysis>>,
    
    /// Update call sites (for certain refactorings)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_calls: Option<UpdateCallsDefinition>,
}

/// Definition for extracting code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractDefinition {
    /// Type of extraction
    pub r#type: ExtractType,
    
    /// Template for extracted code
    pub template: String,
    
    /// Where to place extracted code
    pub placement: PlacementStrategy,
}

/// Type of code extraction
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ExtractType {
    Function,
    Method,
    Variable,
    Class,
    Constant,
}

/// Where to place extracted code
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PlacementStrategy {
    Before,
    After,
    EndOfScope,
    EndOfFile,
    BeforeCurrentFunction,
    TopOfScope,
}

/// Scope analysis operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeAnalysis {
    FindDeclaration { identifier: String },
    FindAllReferences { identifier: String },
    CheckConflicts { identifier: String },
}

/// Definition for updating call sites
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCallsDefinition {
    /// Pattern to match call sites
    pub r#match: String,
    
    /// Replacement for call sites
    pub replace: String,
}

/// Variable extraction and analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDefinition {
    /// Variables to extract from pattern
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extract_from_pattern: Option<Vec<String>>,
    
    /// Parameter handling strategy
    pub parameters: ParameterStrategy,
    
    /// Return value handling
    pub return_values: ReturnValueStrategy,
}

/// How to handle parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParameterStrategy {
    Auto,
    Manual,
    Explicit(Vec<String>),
}

/// How to handle return values
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ReturnValueStrategy {
    Auto,
    None,
    Explicit(Vec<String>),
}

/// Precondition that must be met
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Precondition {
    NoSideEffectsIn { expression: String },
    UniqueName { name: String },
    ValidScope { pattern: String },
}

/// Variant of a refactoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactoringVariant {
    /// Variant identifier
    pub id: String,
    
    /// Pattern for this variant
    pub pattern: PatternDefinition,
    
    /// Transformation for this variant
    pub transform: TransformDefinition,
}

/// Request for validating a refactoring pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateRefactoringRequest {
    /// The refactoring ID to validate
    pub refactoring_id: String,
    
    /// Code to test the pattern against
    pub test_code: String,
    
    /// Programming language
    pub language: String,
    
    /// Optional custom pattern to test
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_pattern: Option<String>,
}

/// Response from pattern validation
#[derive(Debug, Serialize, Deserialize)]
pub struct ValidateRefactoringResponse {
    /// Whether the pattern is valid
    pub is_valid: bool,
    
    /// Matches found in test code
    pub matches: Vec<PatternMatch>,
    
    /// Any parsing errors
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<String>>,
    
    /// Expected transformation result
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_result: Option<String>,
}

/// A single pattern match
#[derive(Debug, Serialize, Deserialize)]
pub struct PatternMatch {
    /// Matched text
    pub text: String,
    
    /// Start position
    pub start: Position,
    
    /// End position
    pub end: Position,
    
    /// Captured variables
    pub variables: HashMap<String, String>,
}

/// Position in source code
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position {
    /// Line number (0-based)
    pub line: usize,
    
    /// Column number (0-based)
    pub column: usize,
}

// Default functions for serde

fn default_scope() -> RefactoringScope {
    RefactoringScope::File
}

fn default_true() -> bool {
    true
}

fn default_max_matches() -> usize {
    1000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refactoring_request_serialization() {
        let request = RefactoringRequest {
            refactoring_id: "extract_method".to_string(),
            pattern_example: Some("console.log($VAR)".to_string()),
            options: Some(RefactoringOptions {
                function_name: Some("logMessage".to_string()),
                scope: RefactoringScope::Project,
                preview: true,
                ..Default::default()
            }),
        };

        let json = serde_json::to_string_pretty(&request).unwrap();
        let deserialized: RefactoringRequest = serde_json::from_str(&json).unwrap();
        
        assert_eq!(request.refactoring_id, deserialized.refactoring_id);
        assert_eq!(request.pattern_example, deserialized.pattern_example);
    }

    #[test]
    fn test_refactoring_definition_parsing() {
        let yaml = r#"
id: extract_method
name: Extract Method
category: composing_methods
description: Extract repeated code into a method
supported_languages: [javascript, typescript]
complexity: moderate
pattern:
  match: |
    console.log($VAR)
transform:
  replace: |
    logMessage($VAR)
  extract:
    type: function
    template: |
      function logMessage(message) {
        console.log(message);
      }
    placement: before_current_function
"#;

        let definition: RefactoringDefinition = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(definition.id, "extract_method");
        assert_eq!(definition.category, RefactoringCategory::ComposingMethods);
        assert_eq!(definition.complexity, RefactoringComplexity::Moderate);
    }
}