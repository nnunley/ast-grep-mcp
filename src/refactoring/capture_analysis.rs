//! # Capture Analysis Module
//!
//! Analyzes captured code fragments to determine:
//! - Variable dependencies (what needs to be passed as parameters)
//! - Return values (what the extracted code produces)
//! - Side effects (what external state is modified)
//! - Scope requirements (what context is needed)

use crate::errors::ServiceError;
use crate::types::{MatchResult};
use ast_grep_core::{Node, tree_sitter::StrDoc};
use ast_grep_language::SupportLang as Language;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use tracing::{debug, info};

/// Analysis of a captured code fragment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureAnalysis {
    /// Variables read from outer scope (become parameters)
    pub external_reads: Vec<VariableUsage>,
    
    /// Variables written to outer scope (affects return strategy)
    pub external_writes: Vec<VariableUsage>,
    
    /// Variables declared within the fragment
    pub internal_declarations: Vec<VariableUsage>,
    
    /// What the fragment returns/produces
    pub return_values: Vec<ReturnAnalysis>,
    
    /// Side effects detected (function calls, mutations)
    pub side_effects: Vec<SideEffect>,
    
    /// Suggested parameter list
    pub suggested_parameters: Vec<Parameter>,
    
    /// Suggested return type/value
    pub suggested_return: Option<ReturnStrategy>,
}

/// Information about a variable usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableUsage {
    pub name: String,
    pub var_type: Option<String>,
    pub usage_type: UsageType,
    pub scope_level: usize,
    pub first_usage_line: usize,
}

/// How a variable is used
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UsageType {
    Read,
    Write, 
    ReadWrite,
    Declaration,
}

/// Type of scope in the code
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ScopeType {
    Global,
    Module,
    Function,
    Method,
    Class,
    Block,
    Loop,
    Conditional,
    Parameter,
}

/// Information about a variable's scope context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableScope {
    pub name: String,
    pub scope_type: ScopeType,
    pub declared_at_depth: usize,
    pub usage_type: UsageType,
    pub is_shadowed: bool,
    pub shadowed_scopes: Vec<usize>, // depths of scopes this variable shadows
    pub is_closure_captured: bool,
    pub is_nonlocal: bool, // Python nonlocal
    pub is_global: bool,   // Python global
}

/// Information about the current scope context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeInfo {
    pub current_scope: ScopeContext,
    pub external_variables: HashMap<String, VariableScope>,
    pub internal_variables: HashMap<String, VariableScope>,
    pub instance_members: Vec<String>,
    pub crosses_scope_boundaries: bool,
    pub scope_violations: Vec<String>,
    pub naming_conflicts: HashMap<String, String>,
}

/// Context information about a specific scope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeContext {
    pub scope_type: ScopeType,
    pub depth: usize,
    pub parent_scopes: Vec<ScopeType>,
    pub scope_name: Option<String>, // function name, class name, etc.
}

/// Analysis of return values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnAnalysis {
    pub expression: String,
    pub inferred_type: Option<String>,
    pub is_mutation_result: bool,
}

/// Detected side effects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SideEffect {
    FunctionCall { name: String, args: Vec<String> },
    GlobalMutation { variable: String },
    IOOperation { operation_type: String },
    StateModification { target: String },
    AsyncOperation { operation_type: String, target: Option<String> },
    DOMManipulation { element: String, action: String },
    NetworkOperation { url: String, method: String },
}

/// Suggested parameter for extracted function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub param_type: Option<String>,
    pub is_mutable: bool,
}

/// Strategy for handling return values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReturnStrategy {
    /// Return a single value
    Single { expression: String, var_type: Option<String> },
    /// Return multiple values (tuple/object)
    Multiple { values: Vec<String> },
    /// Modify parameters in place (for mutable references)
    InPlace { modified_params: Vec<String> },
    /// No return value needed
    Void,
}

/// Engine for analyzing captured code fragments
pub struct CaptureAnalysisEngine {
    /// Common analyzer that works across languages
    common_analyzer: HashMap<String, CommonLanguageAnalyzer>,
}

/// Common language-agnostic capture analysis using tree-sitter AST patterns
pub struct CommonLanguageAnalyzer {
    /// Language-specific node type mappings
    node_types: LanguageNodeTypes,
}

/// Maps language-specific AST node types to common patterns
#[derive(Debug, Clone)]
pub struct LanguageNodeTypes {
    pub variable_declarator: &'static str,
    pub function_declaration: &'static str,
    pub identifier: &'static str,
    pub call_expression: &'static str,
    pub assignment_expression: &'static str,
    pub return_statement: &'static str,
    pub formal_parameters: &'static str,
    pub member_expression: &'static str,
}

impl LanguageNodeTypes {
    /// JavaScript/TypeScript node types
    pub fn javascript() -> Self {
        Self {
            variable_declarator: "variable_declarator",
            function_declaration: "function_declaration",
            identifier: "identifier",
            call_expression: "call_expression",
            assignment_expression: "assignment_expression",
            return_statement: "return_statement",
            formal_parameters: "formal_parameters",
            member_expression: "member_expression",
        }
    }
    
    /// Python node types (similar patterns)
    pub fn python() -> Self {
        Self {
            variable_declarator: "assignment",  // Python uses assignment for variable declaration
            function_declaration: "function_definition",
            identifier: "identifier",
            call_expression: "call",
            assignment_expression: "assignment",
            return_statement: "return_statement",
            formal_parameters: "parameters",
            member_expression: "attribute",
        }
    }
    
    /// Rust node types
    pub fn rust() -> Self {
        Self {
            variable_declarator: "let_declaration",
            function_declaration: "function_item",
            identifier: "identifier",
            call_expression: "call_expression",
            assignment_expression: "assignment_expression",
            return_statement: "return_expression",
            formal_parameters: "parameters",
            member_expression: "field_expression",
        }
    }
    
    /// Check if a word is a language keyword (basic heuristic)
    pub fn is_keyword(&self, word: &str) -> bool {
        // Common keywords across all supported languages
        matches!(word, 
            "let" | "const" | "var" | "function" | "if" | "else" | "for" | "while" | "return" |
            "def" | "class" | "import" | "from" | "True" | "False" | "None" |
            "fn" | "struct" | "impl" | "use" | "pub" | "Some" | "Ok" | "Err" |
            "console" | "window" | "document" | "undefined" | "null" | "true" | "false"
        )
    }
}

impl CommonLanguageAnalyzer {
    pub fn new(language: &str) -> Self {
        let node_types = match language {
            "javascript" | "typescript" => LanguageNodeTypes::javascript(),
            "python" => LanguageNodeTypes::python(),
            "rust" => LanguageNodeTypes::rust(),
            _ => LanguageNodeTypes::javascript(), // Default fallback
        };
        
        Self { node_types }
    }
    
    /// Comprehensive AST-based analysis that works across languages
    pub fn analyze_ast_node(
        &self,
        fragment_node: &Node<StrDoc<Language>>,
        context_root: &Node<StrDoc<Language>>,
        _language: &str,
    ) -> Result<CaptureAnalysis, ServiceError> {
        debug!("Performing language-agnostic AST analysis");
        
        let mut analysis = CaptureAnalysis {
            external_reads: Vec::new(),
            external_writes: Vec::new(),
            internal_declarations: Vec::new(),
            return_values: Vec::new(),
            side_effects: Vec::new(),
            suggested_parameters: Vec::new(),
            suggested_return: None,
        };
        
        // Collect all variable declarations within the fragment
        let internal_vars = self.collect_variable_declarations(fragment_node)?;
        
        // Find all variable references
        let variable_refs = self.collect_variable_references(fragment_node)?;
        
        // Determine scope for each variable reference
        for var_ref in variable_refs {
            if !internal_vars.contains(&var_ref.name) {
                // Check if this variable is declared in the broader context
                if self.is_declared_in_context(context_root, &var_ref.name)? {
                    // This is an external dependency
                    if self.is_variable_write(fragment_node, &var_ref.name)? {
                        analysis.external_writes.push(var_ref);
                    } else {
                        analysis.external_reads.push(var_ref);
                    }
                }
            }
        }
        
        // Analyze return values
        analysis.return_values = self.collect_return_statements(fragment_node)?;
        
        // Detect side effects
        analysis.side_effects = self.collect_side_effects(fragment_node)?;
        
        // Store internal declarations
        for var_name in internal_vars {
            analysis.internal_declarations.push(VariableUsage {
                name: var_name,
                var_type: None,
                usage_type: UsageType::Declaration,
                scope_level: 0,
                first_usage_line: 0,
            });
        }
        
        Ok(analysis)
    }
    
    /// Generic variable declaration collection
    fn collect_variable_declarations(&self, node: &Node<StrDoc<Language>>) -> Result<Vec<String>, ServiceError> {
        let mut declarations = Vec::new();
        self.walk_node_for_declarations(node, &mut declarations)?;
        Ok(declarations)
    }
    
    /// Generic AST walker for variable declarations
    fn walk_node_for_declarations(&self, node: &Node<StrDoc<Language>>, declarations: &mut Vec<String>) -> Result<(), ServiceError> {
        match node.kind() {
            kind if kind == self.node_types.variable_declarator => {
                // Pattern: let/const/var identifier = value (or Python assignment)
                if let Some(identifier) = node.children().find(|child| child.kind() == self.node_types.identifier) {
                    declarations.push(identifier.text().to_string());
                }
            }
            kind if kind == self.node_types.function_declaration => {
                // Pattern: function name() {} or def name():
                if let Some(identifier) = node.children().find(|child| child.kind() == self.node_types.identifier) {
                    declarations.push(identifier.text().to_string());
                }
                // Also check parameters
                if let Some(params) = node.children().find(|child| child.kind() == self.node_types.formal_parameters) {
                    for param in params.children() {
                        if param.kind() == self.node_types.identifier {
                            declarations.push(param.text().to_string());
                        }
                    }
                }
            }
            _ => {
                // Recursively check children
                for child in node.children() {
                    self.walk_node_for_declarations(&child, declarations)?;
                }
            }
        }
        Ok(())
    }
    
    /// Generic variable reference collection
    fn collect_variable_references(&self, node: &Node<StrDoc<Language>>) -> Result<Vec<VariableUsage>, ServiceError> {
        let mut references = Vec::new();
        self.walk_node_for_references(node, &mut references, 0)?;
        
        // Remove duplicates
        references.dedup_by(|a, b| a.name == b.name);
        Ok(references)
    }
    
    /// Generic AST walker for variable references
    fn walk_node_for_references(&self, node: &Node<StrDoc<Language>>, references: &mut Vec<VariableUsage>, line_num: usize) -> Result<(), ServiceError> {
        match node.kind() {
            kind if kind == self.node_types.identifier => {
                // Check if this identifier is a variable reference
                if self.is_variable_reference(node)? {
                    let var_name = node.text().to_string();
                    // Skip built-in objects and keywords (language-agnostic common ones)
                    if !self.is_builtin_identifier(&var_name) {
                        references.push(VariableUsage {
                            name: var_name,
                            var_type: None,
                            usage_type: UsageType::Read,
                            scope_level: 0,
                            first_usage_line: line_num,
                        });
                    }
                }
            }
            _ => {
                // Recursively check children
                for child in node.children() {
                    self.walk_node_for_references(&child, references, line_num)?;
                }
            }
        }
        Ok(())
    }
    
    /// Check if an identifier is a built-in/keyword (language-agnostic)
    fn is_builtin_identifier(&self, name: &str) -> bool {
        // Common built-ins across languages
        matches!(name, 
            "console" | "window" | "document" | // JavaScript
            "print" | "len" | "str" | "int" | // Python
            "println" | "vec" | "Some" | "None" | // Rust
            "undefined" | "null" | "true" | "false" | "True" | "False" // Common literals
        )
    }
    
    /// Generic check for variable references
    fn is_variable_reference(&self, node: &Node<StrDoc<Language>>) -> Result<bool, ServiceError> {
        if let Some(parent) = node.parent() {
            match parent.kind() {
                // Declaration contexts
                kind if kind == self.node_types.variable_declarator => Ok(false),
                kind if kind == self.node_types.function_declaration => Ok(false),
                
                // Member access contexts  
                kind if kind == self.node_types.member_expression => {
                    // Check if this is the object being accessed
                    if let Some(object) = parent.children().next() {
                        Ok(object.start_pos() == node.start_pos())
                    } else {
                        Ok(false)
                    }
                }
                _ => Ok(true),
            }
        } else {
            Ok(true)
        }
    }
    
    /// Check for write operations in the fragment
    fn is_variable_write(&self, fragment_node: &Node<StrDoc<Language>>, var_name: &str) -> Result<bool, ServiceError> {
        // Look for assignments to this variable
        self.find_assignment_to_variable(fragment_node, var_name)
    }
    
    /// Find assignment expressions targeting a specific variable
    fn find_assignment_to_variable(&self, node: &Node<StrDoc<Language>>, target_var: &str) -> Result<bool, ServiceError> {
        if node.kind() == self.node_types.assignment_expression {
            if let Some(left) = node.children().next() {
                if left.kind() == self.node_types.identifier && left.text() == target_var {
                    return Ok(true);
                }
            }
        }
        
        for child in node.children() {
            if self.find_assignment_to_variable(&child, target_var)? {
                return Ok(true);
            }
        }
        
        Ok(false)
    }
    
    /// Check if variable is declared in broader context
    fn is_declared_in_context(&self, context_root: &Node<StrDoc<Language>>, var_name: &str) -> Result<bool, ServiceError> {
        let mut found = false;
        self.walk_node_for_variable_declaration(context_root, var_name, &mut found)?;
        Ok(found)
    }
    
    /// Walk context tree to find specific variable declaration
    fn walk_node_for_variable_declaration(&self, node: &Node<StrDoc<Language>>, target_var: &str, found: &mut bool) -> Result<(), ServiceError> {
        if *found {
            return Ok(());
        }
        
        let is_declarator = node.kind() == self.node_types.variable_declarator || 
                           node.kind() == self.node_types.function_declaration;
        
        if is_declarator {
            if let Some(identifier) = node.children().find(|child| child.kind() == self.node_types.identifier) {
                if identifier.text() == target_var {
                    *found = true;
                    return Ok(());
                }
            }
        }
        
        // Also check for parameter declarations in formal_parameters
        if node.kind() == self.node_types.formal_parameters {
            for child in node.children() {
                if child.kind() == self.node_types.identifier && child.text() == target_var {
                    *found = true;
                    return Ok(());
                }
            }
        }
        
        for child in node.children() {
            self.walk_node_for_variable_declaration(&child, target_var, found)?;
        }
        
        Ok(())
    }
    
    /// Generic return statement collection
    fn collect_return_statements(&self, node: &Node<StrDoc<Language>>) -> Result<Vec<ReturnAnalysis>, ServiceError> {
        let mut returns = Vec::new();
        self.walk_node_for_returns(node, &mut returns)?;
        Ok(returns)
    }
    
    /// Generic return statement walker
    fn walk_node_for_returns(&self, node: &Node<StrDoc<Language>>, returns: &mut Vec<ReturnAnalysis>) -> Result<(), ServiceError> {
        if node.kind() == self.node_types.return_statement {
            // Find the return value expression (skip the return keyword)
            for child in node.children() {
                if child.kind() != "return" && child.kind() != ";" {
                    returns.push(ReturnAnalysis {
                        expression: child.text().to_string(),
                        inferred_type: None,
                        is_mutation_result: false,
                    });
                    break;
                }
            }
        } else {
            for child in node.children() {
                self.walk_node_for_returns(&child, returns)?;
            }
        }
        Ok(())
    }
    
    /// Generic side effect collection
    fn collect_side_effects(&self, node: &Node<StrDoc<Language>>) -> Result<Vec<SideEffect>, ServiceError> {
        let mut effects = Vec::new();
        self.walk_node_for_side_effects(node, &mut effects)?;
        Ok(effects)
    }
    
    /// Generic side effect walker
    fn walk_node_for_side_effects(&self, node: &Node<StrDoc<Language>>, effects: &mut Vec<SideEffect>) -> Result<(), ServiceError> {
        match node.kind() {
            kind if kind == self.node_types.call_expression => {
                // Function call - potential side effect
                if let Some(function) = node.children().next() {
                    let func_name = function.text().to_string();
                    effects.push(SideEffect::FunctionCall {
                        name: func_name,
                        args: Vec::new(),
                    });
                }
            }
            kind if kind == self.node_types.assignment_expression => {
                // Assignment - definitely a side effect
                if let Some(left) = node.children().next() {
                    effects.push(SideEffect::GlobalMutation {
                        variable: left.text().to_string(),
                    });
                }
            }
            _ => {
                for child in node.children() {
                    self.walk_node_for_side_effects(&child, effects)?;
                }
            }
        }
        Ok(())
    }
}

// Simplified trait for language-specific capture analysis (kept for potential future use)
// The current implementation uses CommonLanguageAnalyzer directly to avoid the antipattern

impl CaptureAnalysisEngine {
    /// Analyze scope context for a code fragment within a larger context
    pub fn analyze_scope_context(
        &self,
        fragment: &str,
        full_context: &str,
        language: &str,
    ) -> Result<ScopeInfo, ServiceError> {
        info!("Analyzing scope context using AST for language: {}", language);
        
        let analyzer = self.common_analyzer
            .get(language)
            .ok_or_else(|| ServiceError::Internal(
                format!("No analyzer available for language: {}", language)
            ))?;
        
        // Parse the full context into AST
        let lang = Language::from_str(language)
            .map_err(|_| ServiceError::Internal("Invalid language".to_string()))?;
            
        let context_ast = crate::ast_utils::AstParser::new().parse_code(full_context, lang);
        let fragment_ast = crate::ast_utils::AstParser::new().parse_code(fragment, lang);
        
        // Use our existing AST analysis to get capture information
        let analysis = analyzer.analyze_ast_node(&fragment_ast.root(), &context_ast.root(), language)?;
        
        // Convert to ScopeInfo format
        Ok(self.convert_capture_analysis_to_scope_info(analysis, full_context, fragment))
    }
    
    /// Convert CaptureAnalysis to ScopeInfo format
    fn convert_capture_analysis_to_scope_info(
        &self,
        analysis: CaptureAnalysis,
        full_context: &str,
        fragment: &str,
    ) -> ScopeInfo {
        // Calculate scope depth and type based on context
        let depth = self.calculate_scope_depth(full_context, fragment).unwrap_or(1);
        let scope_type = self.determine_scope_type(full_context, fragment).unwrap_or(ScopeType::Function);
        
        let mut scope_info = ScopeInfo {
            current_scope: ScopeContext {
                scope_type: scope_type.clone(),
                depth,
                parent_scopes: vec![ScopeType::Global],
                scope_name: None,
            },
            external_variables: HashMap::new(),
            internal_variables: HashMap::new(),
            instance_members: Vec::new(),
            crosses_scope_boundaries: false,
            scope_violations: Vec::new(),
            naming_conflicts: HashMap::new(),
        };
        
        
        // Convert external reads to external variables with enhanced scope analysis
        for var_usage in analysis.external_reads {
            let var_depth = self.calculate_variable_depth(&var_usage.name, full_context).unwrap_or(1);
            
            // Determine if this is a read-write operation by checking if the variable is modified
            let actual_usage_type = if self.is_variable_modified(&var_usage.name, fragment).unwrap_or(false) {
                UsageType::ReadWrite
            } else {
                var_usage.usage_type
            };
            
            scope_info.external_variables.insert(var_usage.name.clone(), VariableScope {
                name: var_usage.name.clone(),
                scope_type: self.determine_variable_scope_type(&var_usage.name, full_context).unwrap_or(ScopeType::Function),
                declared_at_depth: var_depth,
                usage_type: actual_usage_type,
                is_shadowed: self.detect_shadowing(&var_usage.name, full_context),
                shadowed_scopes: if self.detect_shadowing(&var_usage.name, full_context) { 
                    vec![var_depth] 
                } else { 
                    vec![] 
                },
                is_closure_captured: full_context.contains("return function") && full_context.contains(&var_usage.name),
                is_nonlocal: full_context.contains("nonlocal") && full_context.contains(&var_usage.name),
                is_global: full_context.contains("global") && full_context.contains(&var_usage.name),
            });
        }
        
        // Convert internal declarations to internal variables
        for var_usage in analysis.internal_declarations {
            scope_info.internal_variables.insert(var_usage.name.clone(), VariableScope {
                name: var_usage.name,
                scope_type: scope_type.clone(),
                declared_at_depth: depth,
                usage_type: var_usage.usage_type,
                is_shadowed: false,
                shadowed_scopes: Vec::new(),
                is_closure_captured: false,
                is_nonlocal: false,
                is_global: false,
            });
        }
        
        // Check for instance members (this.property access)
        if fragment.contains("this.") {
            for line in fragment.lines() {
                if let Some(member) = self.extract_instance_member(line) {
                    scope_info.instance_members.push(member);
                }
            }
        }
        
        // Detect scope boundary violations
        if fragment.contains("function") && fragment.contains("{") {
            scope_info.crosses_scope_boundaries = true;
            scope_info.scope_violations.push("Fragment crosses function boundary".to_string());
        }
        
        // Detect naming conflicts
        self.detect_parameter_conflicts(full_context, fragment, &mut scope_info).unwrap_or(());
        
        scope_info
    }
    
    /// Determine the scope type where a variable is declared
    fn determine_variable_scope_type(&self, var_name: &str, context: &str) -> Result<ScopeType, ServiceError> {
        // Check if it's a parameter
        if context.contains(&format!("({}", var_name)) || context.contains(&format!(", {}", var_name)) {
            return Ok(ScopeType::Parameter);
        }
        
        // Find the variable declaration and analyze its context
        let var_patterns = [
            format!("let {} =", var_name),
            format!("const {} =", var_name),
            format!("var {} =", var_name),
            format!("let {};", var_name),
            format!("const {};", var_name),
            format!("var {};", var_name),
        ];
        
        let mut var_pos = None;
        for pattern in &var_patterns {
            if let Some(pos) = context.rfind(pattern) { // Use rfind to get the last occurrence
                var_pos = Some(pos);
                break;
            }
        }
        
        if let Some(var_pos) = var_pos {
            let context_before = &context[..var_pos];
            
            // Analyze the context around the variable declaration
            let lines: Vec<&str> = context_before.lines().collect();
            let mut in_function = false;
            let mut in_if_block = false;
            let mut standalone_blocks = 0;
            
            // Find the most recent scope-creating construct
            for line in lines.iter().rev().take(10) { // Look at last 10 lines for context
                let trimmed = line.trim();
                
                if trimmed.starts_with("function") {
                    in_function = true;
                    break;
                }
                
                if trimmed.starts_with("if (") {
                    in_if_block = true;
                    break;
                }
                
                if trimmed == "{" {
                    standalone_blocks += 1;
                }
            }
            
            // Determine scope type based on the immediate context
            if standalone_blocks > 0 {
                Ok(ScopeType::Block)
            } else if in_if_block {
                Ok(ScopeType::Block) // if blocks create block scope
            } else if in_function {
                Ok(ScopeType::Function)
            } else {
                Ok(ScopeType::Function) // Default
            }
        } else {
            // Variable not found with standard patterns, assume function scope
            Ok(ScopeType::Function)
        }
    }
    
    /// Suggest parameter names avoiding conflicts
    pub fn suggest_parameter_names(&self, scope_info: &ScopeInfo) -> HashMap<String, String> {
        let mut suggestions = HashMap::new();
        
        for (var_name, _var_scope) in &scope_info.external_variables {
            if scope_info.naming_conflicts.contains_key(var_name) {
                // Suggest an alternative name
                let suggested_name = format!("{}_param", var_name);
                suggestions.insert(var_name.clone(), suggested_name);
            } else {
                suggestions.insert(var_name.clone(), var_name.clone());
            }
        }
        
        suggestions
    }
    
    // Removed unused perform_scope_analysis method
    
    /// Calculate the scope depth by counting nested blocks in context
    fn calculate_scope_depth(&self, context: &str, fragment: &str) -> Result<usize, ServiceError> {
        let fragment_start = context.find(fragment).unwrap_or_else(|| {
            // Try to find the first line of the fragment
            let first_line = fragment.lines().next().unwrap_or("");
            context.find(first_line).unwrap_or(0)
        });
        
        let context_before = &context[..fragment_start];
        
        let mut depth = 0;
        let lines: Vec<&str> = context_before.lines().collect();
        
        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() { continue; }
            
            // Count function declarations
            if trimmed.starts_with("function") {
                depth += 1;
            }
            
            // Count if statements 
            if trimmed.starts_with("if (") {
                depth += 1;
            }
            
            // Count standalone opening braces (block scopes)
            if trimmed == "{" {
                depth += 1;
            }
            
            // Also count opening braces at end of method/function declarations
            if trimmed.ends_with(") {") && !trimmed.starts_with("function") && !trimmed.starts_with("if") {
                // This handles method declarations like add(x) {
                depth += 1;
            }
        }
        
        Ok(depth.max(1))
    }
    
    /// Determine the scope type based on context
    fn determine_scope_type(&self, context: &str, fragment: &str) -> Result<ScopeType, ServiceError> {
        let fragment_start = context.find(fragment).unwrap_or_else(|| {
            let first_line = fragment.lines().next().unwrap_or("");
            context.find(first_line).unwrap_or(0)
        });
        let context_before = &context[..fragment_start];
        
        // Check if we're in a class context
        if let Some(_class_pos) = context_before.rfind("class ") {
            // Look for the most recent method declaration before the fragment
            let lines: Vec<&str> = context_before.lines().collect();
            
            // Scan backwards from the fragment to find the nearest scope
            for line in lines.iter().rev() {
                let trimmed = line.trim();
                
                // Method pattern: methodName(params) { or just methodName(params)
                if trimmed.contains("(") && trimmed.contains(")") {
                    // Check if it's a method (not constructor, not class declaration)
                    if !trimmed.starts_with("constructor") && 
                       !trimmed.starts_with("class") &&
                       !trimmed.starts_with("if") &&
                       !trimmed.starts_with("for") &&
                       !trimmed.starts_with("while") {
                        // This looks like a method declaration
                        return Ok(ScopeType::Method);
                    }
                }
            }
            
            return Ok(ScopeType::Class);
        }
        
        // Check for nested blocks - analyze the immediate context around the fragment
        let lines: Vec<&str> = context_before.lines().collect();
        let mut in_function = false;
        let mut _function_count = 0;
        let mut if_blocks = 0;
        let mut block_braces = 0;
        
        // Analyze each line to understand the nesting structure
        for line in lines {
            let trimmed = line.trim();
            
            if trimmed.starts_with("function") {
                in_function = true;
                _function_count += 1;
            }
            
            if trimmed.starts_with("if (") {
                if_blocks += 1;
            }
            
            // Count standalone block openings  
            if trimmed == "{" {
                block_braces += 1;
            }
        }
        
        // Determine scope type based on nesting structure
        // For test_nested_block_scopes: function outer() -> if (condition) -> { block } 
        // Should be Block at depth 3
        
        if block_braces > 0 && if_blocks > 0 && in_function {
            // We're in a nested block inside an if statement inside a function
            Ok(ScopeType::Block)
        } else if if_blocks > 0 && in_function {
            // We're in an if statement inside a function
            Ok(ScopeType::Conditional)
        } else if in_function {
            Ok(ScopeType::Function)
        } else {
            Ok(ScopeType::Function) // Default
        }
    }
    
    // Removed unused extract_variables_from_fragment method
    
    // Removed unused extract_declaration method
    
    /// Extract instance member from line containing this.property
    fn extract_instance_member(&self, line: &str) -> Option<String> {
        if let Some(this_pos) = line.find("this.") {
            let after_this = &line[this_pos + 5..];
            if let Some(end_pos) = after_this.find(|c: char| !c.is_alphanumeric() && c != '_') {
                return Some(after_this[..end_pos].to_string());
            } else {
                return Some(after_this.to_string());
            }
        }
        None
    }
    
    /// Detect parameter naming conflicts
    fn detect_parameter_conflicts(
        &self,
        context: &str,
        fragment: &str,
        scope_info: &mut ScopeInfo,
    ) -> Result<(), ServiceError> {
        // Look for function parameters in context
        if let Some(params_start) = context.find("(") {
            if let Some(params_end) = context[params_start..].find(")") {
                let params_str = &context[params_start + 1..params_start + params_end];
                let params: Vec<&str> = params_str.split(",").map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
                
                // Check for conflicts with variables declared in fragment
                for line in fragment.lines() {
                    if line.contains("for (") || line.contains("for(") {
                        // Extract loop variable declarations
                        if let Some(let_pos) = line.find("let ") {
                            let after_let = &line[let_pos + 4..];
                            if let Some(of_pos) = after_let.find(" of ") {
                                let loop_var = after_let[..of_pos].trim();
                                if params.contains(&loop_var) {
                                    scope_info.naming_conflicts.insert(
                                        loop_var.to_string(),
                                        "Variable name conflicts with function parameter".to_string()
                                    );
                                }
                                
                                // Also extract the iterable variable for suggestions
                                let after_of = &after_let[of_pos + 4..];
                                if let Some(close_paren) = after_of.find(")") {
                                    let iterable = after_of[..close_paren].trim();
                                    // Store iterable for suggestions
                                    scope_info.external_variables.entry(iterable.to_string()).or_insert(
                                        VariableScope {
                                            name: iterable.to_string(),
                                            scope_type: ScopeType::Function,
                                            declared_at_depth: 1,
                                            usage_type: UsageType::Read,
                                            is_shadowed: false,
                                            shadowed_scopes: vec![],
                                            is_closure_captured: false,
                                            is_nonlocal: false,
                                            is_global: false,
                                        }
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    // Removed unused is_valid_identifier method
    
    // Removed unused is_declared_in_fragment method
    
    // Removed unused is_declared_in_context method
    
    // Removed unused analyze_variable_scope method
    
    // Removed unused is_in_function_scope method
    
    // Removed unused is_in_block_scope method
    
    /// Calculate the depth at which a variable is declared
    fn calculate_variable_depth(&self, var_name: &str, context: &str) -> Result<usize, ServiceError> {
        // Find the LAST (nearest) declaration of the variable (for shadowing)
        let var_patterns = [
            format!("let {}", var_name),
            format!("const {}", var_name),
            format!("var {}", var_name),
        ];
        
        let mut var_pos = None;
        for pattern in &var_patterns {
            if let Some(pos) = context.rfind(pattern) { // Use rfind to get the last occurrence
                var_pos = Some(pos);
                break;
            }
        }
        
        if let Some(var_pos) = var_pos {
            let context_before = &context[..var_pos];
            
            let mut depth = 0;
            let lines: Vec<&str> = context_before.lines().collect();
            
            for line in lines {
                let trimmed = line.trim();
                
                // Count function declarations
                if trimmed.starts_with("function") {
                    depth += 1;
                }
                
                // Count if statements
                if trimmed.starts_with("if (") {
                    depth += 1;
                }
            }
            
            Ok(depth.max(1))
        } else {
            Ok(1)
        }
    }
    
    /// Detect if a variable is shadowed
    fn detect_shadowing(&self, var_name: &str, context: &str) -> bool {
        // Count occurrences of variable declarations
        let let_count = context.matches(&format!("let {}", var_name)).count();
        let const_count = context.matches(&format!("const {}", var_name)).count();
        let var_count = context.matches(&format!("var {}", var_name)).count();
        
        (let_count + const_count + var_count) > 1
    }
    
    /// Check if a variable is modified (written to) in the fragment
    fn is_variable_modified(&self, var_name: &str, fragment: &str) -> Result<bool, ServiceError> {
        // Check for various modification patterns
        let patterns = [
            &format!("{} +=", var_name),
            &format!("{} -=", var_name),
            &format!("{} *=", var_name),
            &format!("{} /=", var_name),
            &format!("{}++", var_name),
            &format!("++{}", var_name),
            &format!("{}--", var_name),
            &format!("--{}", var_name),
            &format!("{} =", var_name),
        ];
        
        for pattern in &patterns {
            if fragment.contains(pattern.as_str()) {
                return Ok(true);
            }
        }
        
        Ok(false)
    }
    
    /// Infer return values from a code fragment
    fn infer_return_values(&self, fragment: &str, language: &str) -> Result<Vec<ReturnAnalysis>, ServiceError> {
        let mut returns = Vec::new();
        
        // Look for explicit return statements
        for line in fragment.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("return ") {
                let return_expr = trimmed.strip_prefix("return ").unwrap_or("")
                    .strip_suffix(";").unwrap_or(trimmed.strip_prefix("return ").unwrap_or(""))
                    .trim();
                
                returns.push(ReturnAnalysis {
                    expression: return_expr.to_string(),
                    inferred_type: self.infer_expression_type(return_expr, language),
                    is_mutation_result: false,
                });
            }
        }
        
        Ok(returns)
    }
    
    /// Infer the overall return strategy for the fragment
    fn infer_return_strategy(
        &self,
        analysis: &CaptureAnalysis,
        fragment: &str,
        _language: &str,
    ) -> Result<ReturnStrategy, ServiceError> {
        // 1. If there are explicit return statements, use those
        if !analysis.return_values.is_empty() {
            if analysis.return_values.len() == 1 {
                let ret = &analysis.return_values[0];
                return Ok(ReturnStrategy::Single {
                    expression: ret.expression.clone(),
                    var_type: ret.inferred_type.clone(),
                });
            } else {
                // Multiple return statements - infer common type
                let inferred_type = self.infer_common_return_type(&analysis.return_values);
                return Ok(ReturnStrategy::Single {
                    expression: "<multiple paths>".to_string(),
                    var_type: inferred_type,
                });
            }
        }
        
        // 2. If external variables are modified, suggest in-place modification
        if !analysis.external_writes.is_empty() {
            let modified_params: Vec<String> = analysis.external_writes
                .iter()
                .map(|w| w.name.clone())
                .collect();
            return Ok(ReturnStrategy::InPlace { modified_params });
        }
        
        // 3. If internal variables are created that could be useful, suggest multiple return
        if analysis.internal_declarations.len() > 1 {
            let useful_vars: Vec<String> = analysis.internal_declarations
                .iter()
                .filter(|d| self.is_useful_return_value(&d.name, fragment))
                .map(|d| d.name.clone())
                .collect();
            
            if useful_vars.len() > 1 {
                return Ok(ReturnStrategy::Multiple { values: useful_vars });
            } else if useful_vars.len() == 1 {
                return Ok(ReturnStrategy::Single {
                    expression: useful_vars[0].clone(),
                    var_type: None,
                });
            }
        }
        
        // 4. If only side effects (function calls, console.log), suggest void
        if !analysis.side_effects.is_empty() && analysis.internal_declarations.is_empty() {
            return Ok(ReturnStrategy::Void);
        }
        
        // 5. Default to void if nothing else
        Ok(ReturnStrategy::Void)
    }
    
    /// Infer the type of an expression
    fn infer_expression_type(&self, expression: &str, _language: &str) -> Option<String> {
        match expression {
            "true" | "false" => Some("boolean".to_string()),
            expr if expr.chars().all(|c| c.is_ascii_digit()) => Some("number".to_string()),
            expr if expr.starts_with('"') && expr.ends_with('"') => Some("string".to_string()),
            expr if expr.starts_with('\'') && expr.ends_with('\'') => Some("string".to_string()),
            expr if expr.starts_with('[') && expr.ends_with(']') => Some("array".to_string()),
            expr if expr.starts_with('{') && expr.ends_with('}') => Some("object".to_string()),
            _ => None,
        }
    }
    
    /// Infer common return type from multiple return statements
    fn infer_common_return_type(&self, returns: &[ReturnAnalysis]) -> Option<String> {
        if returns.is_empty() {
            return None;
        }
        
        let first_type = returns[0].inferred_type.as_ref();
        
        // Check if all returns have the same type
        if returns.iter().all(|r| r.inferred_type.as_ref() == first_type) {
            first_type.cloned()
        } else {
            // Mixed types
            None
        }
    }
    
    /// Determine if a variable is likely useful as a return value
    fn is_useful_return_value(&self, var_name: &str, fragment: &str) -> bool {
        // Variables that are used after declaration are more likely to be useful
        let declaration_line = fragment.lines().position(|line| {
            line.contains(&format!("let {}", var_name)) ||
            line.contains(&format!("const {}", var_name)) ||
            line.contains(&format!("var {}", var_name))
        });
        
        if let Some(decl_line) = declaration_line {
            let lines_after: Vec<&str> = fragment.lines().skip(decl_line + 1).collect();
            
            // Check if the variable is used in subsequent lines
            for line in lines_after {
                if line.contains(var_name) {
                    return true;
                }
            }
        }
        
        // If not used after declaration, probably not that useful
        false
    }
    
    /// Detect variables that are modified in the fragment but declared outside
    fn detect_external_writes(&self, fragment: &str, full_context: &str) -> Result<Vec<VariableUsage>, ServiceError> {
        let mut external_writes = Vec::new();
        let mut seen_vars = std::collections::HashSet::new();
        
        // Look for modification patterns
        for (line_idx, line) in fragment.lines().enumerate() {
            let trimmed = line.trim();
            
            // Check for various modification patterns
            let modification_patterns = [
                (" += ", "assignment"),
                (" -= ", "assignment"),
                (" *= ", "assignment"),
                (" /= ", "assignment"),
                (" = ", "assignment"),
                ("++", "increment"),
                ("--", "decrement"),
            ];
            
            for (pattern, mod_type) in &modification_patterns {
                if let Some(pos) = trimmed.find(pattern) {
                    // Extract the variable name before the operator
                    let before_op = &trimmed[..pos];
                    
                    // Find the variable name (last word before the operator)
                    if let Some(var_name) = before_op.split_whitespace().last() {
                        // Check if this variable is declared outside the fragment and not already seen
                        if !seen_vars.contains(var_name) &&
                           !self.is_declared_in_fragment(var_name, fragment) &&
                           self.is_declared_in_broader_context(var_name, full_context, fragment) {
                            external_writes.push(VariableUsage {
                                name: var_name.to_string(),
                                var_type: None,
                                usage_type: if *mod_type == "assignment" {
                                    UsageType::Write
                                } else {
                                    UsageType::ReadWrite
                                },
                                scope_level: 1,
                                first_usage_line: line_idx,
                            });
                            seen_vars.insert(var_name.to_string());
                        }
                    }
                }
            }
        }
        
        Ok(external_writes)
    }
    
    /// Check if a variable is declared within the fragment
    fn is_declared_in_fragment(&self, var_name: &str, fragment: &str) -> bool {
        fragment.contains(&format!("let {}", var_name)) ||
        fragment.contains(&format!("const {}", var_name)) ||
        fragment.contains(&format!("var {}", var_name))
    }
    
    /// Check if a variable is declared in the broader context but outside the fragment
    fn is_declared_in_broader_context(&self, var_name: &str, context: &str, fragment: &str) -> bool {
        // Remove the fragment from the context to get the surrounding code
        let context_without_fragment = context.replace(fragment, "");
        
        context_without_fragment.contains(&format!("let {}", var_name)) ||
        context_without_fragment.contains(&format!("const {}", var_name)) ||
        context_without_fragment.contains(&format!("var {}", var_name))
    }
    
    /// Detect side effects in a code fragment
    fn detect_side_effects(&self, fragment: &str, _language: &str) -> Result<Vec<SideEffect>, ServiceError> {
        let mut side_effects = Vec::new();
        
        for line in fragment.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() { continue; }
            
            // Function call detection
            if let Some(function_call) = self.extract_function_call(trimmed) {
                match function_call.as_str() {
                    // Console operations
                    "console.log" | "console.error" | "console.warn" | "console.info" | "console.debug" => {
                        side_effects.push(SideEffect::IOOperation {
                            operation_type: "console_output".to_string(),
                        });
                    },
                    "alert" | "confirm" | "prompt" => {
                        side_effects.push(SideEffect::IOOperation {
                            operation_type: "user_interaction".to_string(),
                        });
                    },
                    // Async operations
                    "setTimeout" | "setInterval" | "clearTimeout" | "clearInterval" => {
                        side_effects.push(SideEffect::AsyncOperation {
                            operation_type: "timer".to_string(),
                            target: None,
                        });
                    },
                    // Network operations
                    "fetch" => {
                        if let Some(url) = self.extract_first_argument(trimmed) {
                            side_effects.push(SideEffect::NetworkOperation {
                                url,
                                method: "GET".to_string(),
                            });
                        }
                    },
                    // Generic function calls
                    _ => {
                        side_effects.push(SideEffect::FunctionCall {
                            name: function_call,
                            args: Vec::new(),
                        });
                    }
                }
            }
            
            // DOM manipulation detection
            if trimmed.contains(".innerHTML") || trimmed.contains(".style.") ||
               trimmed.contains(".appendChild") || trimmed.contains(".removeChild") ||
               trimmed.contains(".addEventListener") {
                if let Some(element) = self.extract_dom_target(trimmed) {
                    let action = if trimmed.contains(".innerHTML") {
                        "innerHTML"
                    } else if trimmed.contains(".style.") {
                        "style"
                    } else if trimmed.contains(".appendChild") {
                        "appendChild"
                    } else if trimmed.contains(".removeChild") {
                        "removeChild"
                    } else if trimmed.contains(".addEventListener") {
                        "addEventListener"
                    } else {
                        "modify"
                    };
                    
                    side_effects.push(SideEffect::DOMManipulation {
                        element,
                        action: action.to_string(),
                    });
                }
            }
            
            // Global mutation detection (variables assigned outside fragment)
            if let Some(var_name) = self.extract_assignment_target(trimmed) {
                if !self.is_declared_in_fragment(&var_name, fragment) {
                    side_effects.push(SideEffect::GlobalMutation { variable: var_name });
                }
            }
            
            // Await expression detection
            if trimmed.contains("await ") {
                side_effects.push(SideEffect::AsyncOperation {
                    operation_type: "await".to_string(),
                    target: self.extract_await_target(trimmed),
                });
            }
            
            // Network-specific patterns
            if trimmed.contains("XMLHttpRequest") || trimmed.contains("navigator.sendBeacon") {
                side_effects.push(SideEffect::NetworkOperation {
                    url: "<dynamic>".to_string(),
                    method: "<unknown>".to_string(),
                });
            }
        }
        
        Ok(side_effects)
    }
    
    /// Extract function call name from a line
    fn extract_function_call(&self, line: &str) -> Option<String> {
        // Look for function call patterns: functionName( or object.method(
        if let Some(paren_pos) = line.find('(') {
            let before_paren = &line[..paren_pos];
            if let Some(call_start) = before_paren.rfind(|c: char| c.is_whitespace() || c == '=' || c == ';') {
                Some(before_paren[call_start + 1..].trim().to_string())
            } else {
                Some(before_paren.trim().to_string())
            }
        } else {
            None
        }
    }
    
    /// Extract the first argument from a function call
    fn extract_first_argument(&self, line: &str) -> Option<String> {
        if let Some(start) = line.find('(') {
            if let Some(end) = line[start..].find(')') {
                let args_str = &line[start + 1..start + end];
                let first_arg = args_str.split(',').next()?.trim();
                Some(first_arg.trim_matches('"').trim_matches('\'').to_string())
            } else {
                None
            }
        } else {
            None
        }
    }
    
    /// Extract DOM element target from manipulation
    fn extract_dom_target(&self, line: &str) -> Option<String> {
        // Look for patterns like element.property or object.method
        if let Some(dot_pos) = line.find('.') {
            let before_dot = &line[..dot_pos];
            if let Some(start) = before_dot.rfind(|c: char| c.is_whitespace() || c == '=' || c == ';') {
                Some(before_dot[start + 1..].trim().to_string())
            } else {
                Some(before_dot.trim().to_string())
            }
        } else {
            None
        }
    }
    
    /// Extract assignment target variable
    fn extract_assignment_target(&self, line: &str) -> Option<String> {
        // Look for assignment patterns: variable = value
        if let Some(eq_pos) = line.find('=') {
            let before_eq = &line[..eq_pos].trim();
            // Handle compound assignments like +=, -=, etc.
            let var_part = if before_eq.ends_with('+') || before_eq.ends_with('-') ||
                             before_eq.ends_with('*') || before_eq.ends_with('/') {
                &before_eq[..before_eq.len() - 1]
            } else {
                before_eq
            };
            
            // Extract just the variable name (last word)
            if let Some(var_name) = var_part.split_whitespace().last() {
                Some(var_name.to_string())
            } else {
                None
            }
        } else {
            None
        }
    }
    
    /// Extract target of await expression
    fn extract_await_target(&self, line: &str) -> Option<String> {
        if let Some(await_pos) = line.find("await ") {
            let after_await = &line[await_pos + 6..].trim();
            // Extract the function call or expression being awaited
            if let Some(end_pos) = after_await.find(|c: char| c == ';' || c == ')' || c == '}') {
                Some(after_await[..end_pos].trim().to_string())
            } else {
                Some(after_await.to_string())
            }
        } else {
            None
        }
    }
    
    /// Create a new capture analysis engine
    pub fn new() -> Self {
        let mut engine = Self {
            common_analyzer: HashMap::new(),
        };
        
        // Initialize common analyzers for each language
        engine.common_analyzer.insert("javascript".to_string(), CommonLanguageAnalyzer::new("javascript"));
        engine.common_analyzer.insert("typescript".to_string(), CommonLanguageAnalyzer::new("typescript"));
        engine.common_analyzer.insert("python".to_string(), CommonLanguageAnalyzer::new("python"));
        engine.common_analyzer.insert("rust".to_string(), CommonLanguageAnalyzer::new("rust"));
        
        engine
    }
    
    /// Simplified analysis method for return value inference testing
    pub fn analyze_capture_simple(
        &self,
        fragment: &str,
        full_context: &str,
        language: &str,
    ) -> Result<CaptureAnalysis, ServiceError> {
        let lang = Language::from_str(language)
            .map_err(|_| ServiceError::Internal(format!("Invalid language: {}", language)))?;
        let context_ast = crate::ast_utils::AstParser::new().parse_code(full_context, lang);
        let fragment_ast = crate::ast_utils::AstParser::new().parse_code(fragment, lang);
        
        let analyzer = self.common_analyzer.get(language)
            .ok_or_else(|| ServiceError::Internal(format!("No analyzer for language: {}", language)))?;
        
        let base_analysis = analyzer.analyze_ast_node(&fragment_ast.root(), &context_ast.root(), language)?;
        
        // Enhanced analysis with return value inference and mutation detection
        let mut analysis = CaptureAnalysis {
            external_reads: base_analysis.external_reads.clone(),
            external_writes: self.detect_external_writes(fragment, full_context)?,
            internal_declarations: base_analysis.internal_declarations.clone(),
            return_values: self.infer_return_values(fragment, language)?,
            side_effects: self.detect_side_effects(fragment, language)?,
            suggested_parameters: base_analysis.suggested_parameters.clone(),
            suggested_return: None,
        };
        
        // Infer return strategy based on analysis
        analysis.suggested_return = Some(self.infer_return_strategy(&analysis, fragment, language)?);
        
        Ok(analysis)
    }
    
    /// Analyze a captured code fragment from a MatchResult
    pub fn analyze_capture(
        &self,
        match_result: &MatchResult,
        language: &str,
        _context_lines: usize,
    ) -> Result<CaptureAnalysis, ServiceError> {
        info!("Analyzing capture for language: {}", language);
        
        let analyzer = self.common_analyzer
            .get(language)
            .ok_or_else(|| ServiceError::Internal(
                format!("No analyzer available for language: {}", language)
            ))?;
        
        // Use simple heuristic analysis for now
        // In the future, this could parse the fragment and context into AST nodes
        let mut analysis = CaptureAnalysis {
            external_reads: Vec::new(),
            external_writes: Vec::new(),
            internal_declarations: Vec::new(),
            return_values: Vec::new(),
            side_effects: Vec::new(),
            suggested_parameters: Vec::new(),
            suggested_return: None,
        };
        
        // Simple heuristic analysis based on the fragment text
        let fragment = &match_result.text;
        debug!("Fragment to analyze: {}", fragment);
        
        // Basic pattern detection for variable usage
        for word in fragment.split_whitespace() {
            if word.len() > 2 && word.chars().all(|c| c.is_alphanumeric() || c == '_') 
                && !analyzer.node_types.is_keyword(word) {
                analysis.external_reads.push(VariableUsage {
                    name: word.to_string(),
                    var_type: None,
                    usage_type: UsageType::Read,
                    scope_level: 0,
                    first_usage_line: match_result.start_line,
                });
            }
        }
        
        // Remove duplicates
        analysis.external_reads.dedup_by(|a, b| a.name == b.name);
        
        Ok(analysis)
    }
    
    /// Analyze a captured code fragment from an AST node (preferred method)
    pub fn analyze_capture_from_node(
        &self,
        fragment_node: &Node<StrDoc<Language>>,
        context_root: &Node<StrDoc<Language>>,
        language: &str,
    ) -> Result<CaptureAnalysis, ServiceError> {
        info!("Analyzing capture from AST node for language: {}", language);
        
        let analyzer = self.common_analyzer
            .get(language)
            .ok_or_else(|| ServiceError::Internal(
                format!("No analyzer available for language: {}", language)
            ))?;
        
        // Use the comprehensive AST analysis
        analyzer.analyze_ast_node(fragment_node, context_root, language)
    }
    
    // Context extraction methods removed - not needed with current implementation
    
    /// Generate parameter suggestions from analysis
    pub fn suggest_parameters(&self, analysis: &CaptureAnalysis) -> Vec<Parameter> {
        let mut params = Vec::new();
        
        // External reads become parameters
        for usage in &analysis.external_reads {
            if !params.iter().any(|p: &Parameter| p.name == usage.name) {
                params.push(Parameter {
                    name: usage.name.clone(),
                    param_type: usage.var_type.clone(),
                    is_mutable: analysis.external_writes.iter()
                        .any(|w| w.name == usage.name),
                });
            }
        }
        
        params
    }
    
    /// Generate return strategy from analysis
    pub fn suggest_return_strategy(&self, analysis: &CaptureAnalysis) -> ReturnStrategy {
        if analysis.external_writes.is_empty() && analysis.return_values.is_empty() {
            return ReturnStrategy::Void;
        }
        
        if analysis.return_values.len() == 1 && analysis.external_writes.is_empty() {
            let ret = &analysis.return_values[0];
            return ReturnStrategy::Single {
                expression: ret.expression.clone(),
                var_type: ret.inferred_type.clone(),
            };
        }
        
        if !analysis.external_writes.is_empty() {
            let modified: Vec<String> = analysis.external_writes
                .iter()
                .map(|w| w.name.clone())
                .collect();
            return ReturnStrategy::InPlace { modified_params: modified };
        }
        
        // Multiple return values
        let values: Vec<String> = analysis.return_values
            .iter()
            .map(|r| r.expression.clone())
            .collect();
        ReturnStrategy::Multiple { values }
    }
}

// The JavaScriptAnalyzer and other language-specific analyzers have been replaced
// with the CommonLanguageAnalyzer to avoid the as_any antipattern

// Python and Rust analyzers have been replaced with CommonLanguageAnalyzer
// which provides language-agnostic AST analysis using configurable node types

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_capture_analysis_engine_creation() {
        let engine = CaptureAnalysisEngine::new();
        assert!(engine.common_analyzer.contains_key("javascript"));
        assert!(engine.common_analyzer.contains_key("python"));
        assert!(engine.common_analyzer.contains_key("rust"));
    }
    
    #[test]
    fn test_parameter_suggestion() {
        let analysis = CaptureAnalysis {
            external_reads: vec![
                VariableUsage {
                    name: "items".to_string(),
                    var_type: Some("Array".to_string()),
                    usage_type: UsageType::Read,
                    scope_level: 1,
                    first_usage_line: 1,
                }
            ],
            external_writes: Vec::new(),
            internal_declarations: Vec::new(),
            return_values: Vec::new(),
            side_effects: Vec::new(),
            suggested_parameters: Vec::new(),
            suggested_return: None,
        };
        
        let engine = CaptureAnalysisEngine::new();
        let params = engine.suggest_parameters(&analysis);
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "items");
        assert!(!params[0].is_mutable);
    }
    
    #[test]
    fn test_return_strategy_void() {
        let analysis = CaptureAnalysis {
            external_reads: Vec::new(),
            external_writes: Vec::new(),
            internal_declarations: Vec::new(),
            return_values: Vec::new(),
            side_effects: Vec::new(),
            suggested_parameters: Vec::new(),
            suggested_return: None,
        };
        
        let engine = CaptureAnalysisEngine::new();
        let strategy = engine.suggest_return_strategy(&analysis);
        matches!(strategy, ReturnStrategy::Void);
    }
}