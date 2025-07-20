//! # Language Injection Detection
//!
//! Provides automatic detection and configuration for embedded languages,
//! mimicking ast-grep CLI's behavior for HTML/JS/CSS.

use ast_grep_language::SupportLang as Language;
use std::path::Path;

/// Built-in language injection configurations
pub struct LanguageInjection;

impl LanguageInjection {
    /// Detect if language injection should be used based on file extension and pattern language
    pub fn should_use_injection(
        file_path: Option<&str>,
        pattern_language: &str,
    ) -> Option<InjectionConfig> {
        if let Some(file_path) = file_path {
            let extension = Path::new(file_path)
                .extension()
                .and_then(|ext| ext.to_str())?;

            match (extension, pattern_language) {
                // HTML with embedded JavaScript
                ("html" | "htm", "javascript" | "js" | "typescript" | "ts") => {
                    Some(InjectionConfig::new(Language::JavaScript))
                }
                // HTML with embedded CSS
                ("html" | "htm", "css") => Some(InjectionConfig::new(Language::Css)),
                // Vue components (HTML by default, can contain JS/CSS)
                ("vue", "javascript" | "js" | "typescript" | "ts") => {
                    Some(InjectionConfig::new(Language::JavaScript))
                }
                ("vue", "css") => Some(InjectionConfig::new(Language::Css)),
                // JSX/TSX files might have CSS-in-JS
                ("jsx" | "tsx", "css") => Some(InjectionConfig::new(Language::Css)),
                _ => None,
            }
        } else {
            // No file path - used for string search
            // Check if we're searching for JS/CSS patterns (might be in HTML string)
            match pattern_language {
                "javascript" | "js" | "typescript" | "ts" => {
                    Some(InjectionConfig::new(Language::JavaScript))
                }
                "css" => Some(InjectionConfig::new(Language::Css)),
                _ => None,
            }
        }
    }

    /// Get the host language for a file based on its extension
    pub fn get_host_language(file_path: &str) -> Option<Language> {
        let extension = Path::new(file_path)
            .extension()
            .and_then(|ext| ext.to_str())?;

        match extension {
            "html" | "htm" => Some(Language::Html),
            "vue" => Some(Language::Html), // Vue files are HTML-based
            "jsx" => Some(Language::Tsx),  // Use TSX parser for JSX
            "tsx" => Some(Language::Tsx),
            _ => None,
        }
    }
}

/// Configuration for a specific language injection scenario
#[derive(Debug, Clone)]
pub struct InjectionConfig {
    pub language: Language,
    pub is_automatic: bool, // Whether this is a built-in automatic injection
}

impl InjectionConfig {
    fn new(language: Language) -> Self {
        Self {
            language,
            is_automatic: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_javascript_injection() {
        let config = LanguageInjection::should_use_injection(Some("index.html"), "javascript");
        assert!(config.is_some());
        assert!(matches!(config.unwrap().language, Language::JavaScript));
    }

    #[test]
    fn test_html_css_injection() {
        let config = LanguageInjection::should_use_injection(Some("styles.html"), "css");
        assert!(config.is_some());
        assert!(matches!(config.unwrap().language, Language::Css));
    }

    #[test]
    fn test_vue_javascript_injection() {
        let config = LanguageInjection::should_use_injection(Some("component.vue"), "javascript");
        assert!(config.is_some());
        assert!(matches!(config.unwrap().language, Language::JavaScript));
    }

    #[test]
    fn test_jsx_css_injection() {
        let config = LanguageInjection::should_use_injection(Some("component.jsx"), "css");
        assert!(config.is_some());
        assert!(matches!(config.unwrap().language, Language::Css));
    }

    #[test]
    fn test_no_injection() {
        // Python file with Python pattern - no injection needed
        let config = LanguageInjection::should_use_injection(Some("script.py"), "python");
        assert!(config.is_none());
    }

    #[test]
    fn test_string_search_injection() {
        // No file path, but searching for JS pattern
        let config = LanguageInjection::should_use_injection(None, "javascript");
        assert!(config.is_some());
        assert!(matches!(config.unwrap().language, Language::JavaScript));
    }

    #[test]
    fn test_get_host_language() {
        assert!(matches!(
            LanguageInjection::get_host_language("index.html"),
            Some(Language::Html)
        ));
        assert!(matches!(
            LanguageInjection::get_host_language("component.vue"),
            Some(Language::Html)
        ));
        assert!(matches!(
            LanguageInjection::get_host_language("app.jsx"),
            Some(Language::Tsx)
        ));
        assert!(LanguageInjection::get_host_language("script.py").is_none());
    }
}
