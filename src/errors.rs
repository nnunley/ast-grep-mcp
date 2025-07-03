use std::fmt;
use rmcp::model::ErrorData;
use std::path::PathBuf;

#[derive(Debug)]
pub enum ServiceError {
    ParserError(String),
    Internal(String),
    Io(std::io::Error),
    WalkDir(walkdir::Error),
    SerdeYaml(serde_yaml::Error),
    SerdeJson(serde_json::Error),
    Regex(regex::Error),
    FileNotFound(PathBuf),
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServiceError::ParserError(msg) => write!(f, "Parser error: {}", msg),
            ServiceError::Internal(msg) => write!(f, "Internal error: {}", msg),
            ServiceError::Io(err) => write!(f, "IO error: {}", err),
            ServiceError::WalkDir(err) => write!(f, "Directory traversal error: {}", err),
            ServiceError::SerdeYaml(err) => write!(f, "YAML parsing error: {}", err),
            ServiceError::SerdeJson(err) => write!(f, "JSON parsing error: {}", err),
            ServiceError::Regex(err) => write!(f, "Regex error: {}", err),
            ServiceError::FileNotFound(path) => write!(f, "File not found: {}", path.display()),
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

impl From<ServiceError> for ErrorData {
    fn from(err: ServiceError) -> Self {
        ErrorData::internal_error(err.to_string().into(), None)
    }
}