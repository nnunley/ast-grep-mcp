//! # Refactoring System
//!
//! This module implements a comprehensive refactoring system for ast-grep,
//! providing pattern-based code transformations with token-efficient responses.

pub mod catalog;
pub mod capture_analysis;
pub mod engine;
pub mod service;
pub mod types;
pub mod validation;

pub use service::{RefactoringService, RefactoringInfo, RefactoringDetails};
pub use types::{RefactoringRequest, RefactoringResponse, ValidateRefactoringRequest, ValidateRefactoringResponse, RefactoringOptions};

/// Initialize the refactoring system with default catalog
pub fn initialize_default_catalog() -> Result<catalog::RefactoringCatalog, Box<dyn std::error::Error>> {
    catalog::RefactoringCatalog::load_default()
}