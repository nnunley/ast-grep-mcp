//! # Language Injection Detection
//!
//! Provides automatic detection and configuration for embedded languages,
//! mimicking ast-grep CLI's behavior for HTML/JS/CSS.

use crate::types::EmbeddedLanguageConfig;
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
                    Some(InjectionConfig::html_javascript())
                }
                // HTML with embedded CSS
                ("html" | "htm", "css") => Some(InjectionConfig::html_css()),
                // Vue components (HTML by default, can contain JS/CSS)
                ("vue", "javascript" | "js" | "typescript" | "ts") => {
                    Some(InjectionConfig::vue_javascript())
                }
                ("vue", "css") => Some(InjectionConfig::vue_css()),
                // JSX/TSX files might have CSS-in-JS
                ("jsx" | "tsx", "css") => Some(InjectionConfig::jsx_css_in_js()),
                _ => None,
            }
        } else {
            // No file path - used for string search
            // Check if we're searching for JS/CSS patterns (might be in HTML string)
            match pattern_language {
                "javascript" | "js" | "typescript" | "ts" => {
                    Some(InjectionConfig::html_javascript())
                }
                "css" => Some(InjectionConfig::html_css()),
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
    pub embedded_config: EmbeddedLanguageConfig,
    pub is_automatic: bool, // Whether this is a built-in automatic injection
}

impl InjectionConfig {
    /// HTML with embedded JavaScript in <script> tags
    fn html_javascript() -> Self {
        Self {
            embedded_config: EmbeddedLanguageConfig {
                host_language: "html".to_string(),
                embedded_language: "javascript".to_string(),
                // Use a simpler pattern that captures the script content
                extraction_pattern: "<script>$JS_CODE</script>".to_string(),
                selector: None,
                context: None,
            },
            is_automatic: true,
        }
    }

    /// HTML with embedded CSS in <style> tags
    fn html_css() -> Self {
        Self {
            embedded_config: EmbeddedLanguageConfig {
                host_language: "html".to_string(),
                embedded_language: "css".to_string(),
                extraction_pattern: "<style$ATTRS>$CSS_CODE</style>".to_string(),
                selector: Some("style_element".to_string()),
                context: None,
            },
            is_automatic: true,
        }
    }

    /// Vue component with JavaScript in <script> tags
    fn vue_javascript() -> Self {
        Self {
            embedded_config: EmbeddedLanguageConfig {
                host_language: "html".to_string(),
                embedded_language: "javascript".to_string(),
                extraction_pattern: "<script$ATTRS>$JS_CODE</script>".to_string(),
                selector: Some("script_element".to_string()),
                context: None,
            },
            is_automatic: true,
        }
    }

    /// Vue component with CSS in <style> tags
    fn vue_css() -> Self {
        Self {
            embedded_config: EmbeddedLanguageConfig {
                host_language: "html".to_string(),
                embedded_language: "css".to_string(),
                extraction_pattern: "<style$ATTRS>$CSS_CODE</style>".to_string(),
                selector: Some("style_element".to_string()),
                context: None,
            },
            is_automatic: true,
        }
    }

    /// JSX/TSX with CSS-in-JS (styled-components style)
    fn jsx_css_in_js() -> Self {
        Self {
            embedded_config: EmbeddedLanguageConfig {
                host_language: "tsx".to_string(),
                embedded_language: "css".to_string(),
                // Match styled.div`...` or styled(Component)`...`
                extraction_pattern: "styled.$COMPONENT`$CSS_CODE`".to_string(),
                selector: None,
                context: None,
            },
            is_automatic: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_javascript_detection() {
        let config = LanguageInjection::should_use_injection(Some("index.html"), "javascript");
        assert!(config.is_some());
        let config = config.unwrap();
        assert_eq!(config.embedded_config.host_language, "html");
        assert_eq!(config.embedded_config.embedded_language, "javascript");
        assert!(config.is_automatic);
    }

    #[test]
    fn test_html_css_detection() {
        let config = LanguageInjection::should_use_injection(Some("styles.html"), "css");
        assert!(config.is_some());
        let config = config.unwrap();
        assert_eq!(config.embedded_config.host_language, "html");
        assert_eq!(config.embedded_config.embedded_language, "css");
    }

    #[test]
    fn test_no_injection_for_js_file() {
        let config = LanguageInjection::should_use_injection(Some("app.js"), "javascript");
        assert!(config.is_none());
    }

    #[test]
    fn test_vue_component_detection() {
        let config = LanguageInjection::should_use_injection(Some("App.vue"), "javascript");
        assert!(config.is_some());
        let config = config.unwrap();
        assert_eq!(config.embedded_config.embedded_language, "javascript");
    }

    #[test]
    fn test_host_language_detection() {
        assert_eq!(
            LanguageInjection::get_host_language("index.html"),
            Some(Language::Html)
        );
        assert_eq!(
            LanguageInjection::get_host_language("App.vue"),
            Some(Language::Html)
        );
        assert_eq!(
            LanguageInjection::get_host_language("Component.tsx"),
            Some(Language::Tsx)
        );
        assert_eq!(LanguageInjection::get_host_language("script.js"), None);
    }
}
