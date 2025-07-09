//! # Error Types
//!
//! Error handling for the ast-grep MCP service.
//! Provides structured error types that can be converted to MCP ErrorData.

use rmcp::model::ErrorData;
use std::fmt;
use std::path::PathBuf;

/// Error types that can occur during ast-grep MCP service operations.
///
/// These errors cover parsing, I/O, and internal service failures.
/// All errors implement conversion to MCP `ErrorData` for proper error reporting.
#[derive(Debug)]
pub enum ServiceError {
    /// Error parsing ast-grep patterns or rules
    ParserError(String),
    /// Internal service error with custom message
    Internal(String),
    /// I/O error reading/writing files
    Io(std::io::Error),
    /// Error walking directory trees during file search
    WalkDir(walkdir::Error),
    /// Error parsing YAML rule configurations
    SerdeYaml(serde_yaml::Error),
    /// Error parsing JSON data
    SerdeJson(serde_json::Error),
    /// Regular expression compilation error
    Regex(regex::Error),
    /// Requested file not found
    FileNotFound(PathBuf),
    /// Glob pattern compilation error
    Glob(globset::Error),
    /// MCP tool not found
    ToolNotFound(String),
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServiceError::ParserError(msg) => write!(f, "Parser error: {msg}"),
            ServiceError::Internal(msg) => write!(f, "Internal error: {msg}"),
            ServiceError::Io(err) => write!(f, "IO error: {err}"),
            ServiceError::WalkDir(err) => write!(f, "Directory traversal error: {err}"),
            ServiceError::SerdeYaml(err) => write!(f, "YAML parsing error: {err}"),
            ServiceError::SerdeJson(err) => write!(f, "JSON parsing error: {err}"),
            ServiceError::Regex(err) => write!(f, "Regex error: {err}"),
            ServiceError::FileNotFound(path) => write!(f, "File not found: {}", path.display()),
            ServiceError::Glob(err) => write!(f, "Glob error: {err}"),
            ServiceError::ToolNotFound(tool) => write!(f, "Tool not found: {tool}"),
        }
    }
}

impl std::error::Error for ServiceError {}

impl From<std::io::Error> for ServiceError {
    fn from(err: std::io::Error) -> Self {
        ServiceError::Io(err)
    }
}

impl From<walkdir::Error> for ServiceError {
    fn from(err: walkdir::Error) -> Self {
        ServiceError::WalkDir(err)
    }
}

impl From<serde_yaml::Error> for ServiceError {
    fn from(err: serde_yaml::Error) -> Self {
        ServiceError::SerdeYaml(err)
    }
}

impl From<serde_json::Error> for ServiceError {
    fn from(err: serde_json::Error) -> Self {
        ServiceError::SerdeJson(err)
    }
}

impl From<regex::Error> for ServiceError {
    fn from(err: regex::Error) -> Self {
        ServiceError::Regex(err)
    }
}

impl From<globset::Error> for ServiceError {
    fn from(err: globset::Error) -> Self {
        ServiceError::Glob(err)
    }
}

impl From<ServiceError> for ErrorData {
    fn from(err: ServiceError) -> Self {
        ErrorData::internal_error(err.to_string(), None)
    }
}
