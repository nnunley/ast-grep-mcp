//! # ast-grep MCP Service
//!
//! A Model Context Protocol (MCP) service that provides ast-grep functionality for structural
//! code search and transformation. This crate enables AI assistants to perform sophisticated
//! code analysis and refactoring with token-efficient diff-based responses.
//!
//! ## Key Features
//!
//! - **Structural Search & Replace**: Use ast-grep's powerful AST-based pattern matching
//! - **Multi-Root Directory Support**: Search across multiple directory trees
//! - **Token-Efficient Diffs**: Returns line-by-line changes instead of full file content
//! - **Safe by Default**: Dry-run mode with optional in-place file modification
//! - **Multi-Language Support**: JavaScript, TypeScript, Rust, Python, Java, Go, and more
//!
//! ## Usage
//!
//! This crate is designed to be used as an MCP service, but can also be used as a library:
//!
//! ```rust,no_run
//! use ast_grep_mcp::{SearchParam, search::SearchService, config::ServiceConfig};
//! use std::path::PathBuf;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = ServiceConfig {
//!     root_directories: vec![PathBuf::from("src")],
//!     ..Default::default()
//! };
//!
//! let search_service = SearchService::new(config, Default::default(), Default::default());
//!
//! let param = SearchParam {
//!     code: "function test() { console.log('hello'); }".to_string(),
//!     pattern: "console.log($VAR)".to_string(),
//!     language: "javascript".to_string(),
//!     ..Default::default()
//! };
//!
//! let result = search_service.search(param).await?;
//! println!("Found {} matches", result.matches.len());
//! # Ok(())
//! # }
//! ```
//!
//! ## Important Notes
//!
//! - **Manual Comma Handling**: ast-grep does NOT automatically insert commas. You must include
//!   commas explicitly in your replacement patterns.
//! - **Literal Pattern Matching**: ast-grep performs exact pattern matching and replacement.
//!   It does not infer proper syntax or handle validation.
//! - **Struct Update Syntax**: In Rust, fields must come before `..Default::default()` in
//!   struct literals.

pub mod ast_grep_service;
pub mod ast_utils;
pub mod config;
pub mod context_lines;
pub mod errors;
pub mod path_validation;
pub mod pattern;
pub mod replace;
pub mod response_formatter;
pub mod rules;
pub mod search;
pub mod tools;
pub mod types;

#[cfg(test)]
mod test_context_integration;

// Re-export commonly used types
pub use rules::types::*;
pub use types::*;
