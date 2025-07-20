//! # Tool Router Module
//!
//! Handles routing of MCP tool calls to appropriate service methods.
//! This module extracts the tool routing logic from the main service
//! to improve modularity and maintainability.

use crate::ast_grep_service::AstGrepService;
use crate::response_formatter::ResponseFormatter;
use crate::rules::*;
use crate::types::*;

use rmcp::model::{CallToolRequestParam, CallToolResult, Content, ErrorData};
use serde::de::DeserializeOwned;
use std::borrow::Cow;

/// Routes tool calls to appropriate service methods
pub struct ToolRouter;

impl ToolRouter {
    /// Helper function to parse request parameters
    fn parse_params<T: DeserializeOwned>(request: &CallToolRequestParam) -> Result<T, ErrorData> {
        serde_json::from_value(serde_json::Value::Object(
            request.arguments.clone().unwrap_or_default(),
        ))
        .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))
    }

    /// Helper function to create JSON response
    fn create_json_response<T: serde::Serialize>(result: T) -> Result<CallToolResult, ErrorData> {
        let json_value = serde_json::to_value(&result)
            .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
        Ok(CallToolResult::success(vec![Content::json(json_value)?]))
    }

    /// Helper function to create formatted response
    fn create_formatted_response<T: serde::Serialize>(
        result: &T,
        summary: String,
    ) -> Result<CallToolResult, ErrorData> {
        ResponseFormatter::create_formatted_response(result, summary)
            .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))
    }
    /// Route a tool call to the appropriate service method
    pub async fn route_tool_call(
        service: &AstGrepService,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult, ErrorData> {
        match request.name.as_ref() {
            // Search operations
            "search" => Self::handle_search(service, request).await,
            "file_search" => Self::handle_file_search(service, request).await,

            // Replace operations
            "replace" => Self::handle_replace(service, request).await,
            "file_replace" => Self::handle_file_replace(service, request).await,

            // Rule operations
            "rule_search" => Self::handle_rule_search(service, request).await,
            "rule_replace" => Self::handle_rule_replace(service, request).await,
            "create_rule" => Self::handle_create_rule(service, request).await,
            "get_rule" => Self::handle_get_rule(service, request).await,
            "list_rules" => Self::handle_list_rules(service, request).await,
            "delete_rule" => Self::handle_delete_rule(service, request).await,
            "rule_validate" => Self::handle_rule_validate(service, request).await,

            // Utility operations
            "generate_ast" => Self::handle_generate_ast(service, request).await,
            "list_languages" => Self::handle_list_languages(service, request).await,

            // Learning operations
            "validate_pattern" => Self::handle_validate_pattern(service, request).await,
            "explore_patterns" => Self::handle_explore_patterns(service, request).await,

            _ => Err(ErrorData::method_not_found::<
                rmcp::model::CallToolRequestMethod,
            >()),
        }
    }

    // Search operations
    async fn handle_search(
        service: &AstGrepService,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult, ErrorData> {
        let param: SearchParam = Self::parse_params(&request)?;

        // Error handling for common LLM misuse patterns
        if param.code.is_empty() {
            return Err(ErrorData::invalid_params(
                Cow::Borrowed(
                    "The 'search' tool requires the 'code' parameter. If you intend to search across files, please use the 'file_search' tool and provide a 'path_pattern'.",
                ),
                None,
            ));
        }
        // Although SearchParam does not have path_pattern, an LLM might mistakenly pass it.
        // We check raw arguments to provide a more helpful error.
        if let Some(args) = &request.arguments {
            if args.contains_key("path_pattern") {
                return Err(ErrorData::invalid_params(
                    Cow::Borrowed(
                        "The 'search' tool operates on code snippets and does not accept 'path_pattern'. If you intend to search across files, please use the 'file_search' tool.",
                    ),
                    None,
                ));
            }
        }

        let result = service.search(param).await.map_err(ErrorData::from)?;
        let summary = ResponseFormatter::format_search_result(&result);
        Self::create_formatted_response(&result, summary)
    }

    async fn handle_file_search(
        service: &AstGrepService,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult, ErrorData> {
        let param: FileSearchParam = Self::parse_params(&request)?;

        // Error handling for common LLM misuse patterns
        if param.path_pattern.is_empty() {
            return Err(ErrorData::invalid_params(
                Cow::Borrowed(
                    "The 'file_search' tool requires the 'path_pattern' parameter. If you intend to search a code snippet, please use the 'search' tool and provide a 'code' parameter.",
                ),
                None,
            ));
        }
        // Although FileSearchParam does not have code, an LLM might mistakenly pass it.
        // We check raw arguments to provide a more helpful error.
        if let Some(args) = &request.arguments {
            if args.contains_key("code") {
                return Err(ErrorData::invalid_params(
                    Cow::Borrowed(
                        "The 'file_search' tool operates on files and does not accept a 'code' parameter. If you intend to search a code snippet, please use the 'search' tool.",
                    ),
                    None,
                ));
            }
        }

        let result = service.file_search(param).await.map_err(ErrorData::from)?;
        let summary = ResponseFormatter::format_file_search_result(&result);
        Self::create_formatted_response(&result, summary)
    }

    // Replace operations
    async fn handle_replace(
        service: &AstGrepService,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult, ErrorData> {
        let param: ReplaceParam = Self::parse_params(&request)?;

        // Error handling for common LLM misuse patterns
        if param.code.is_empty() {
            return Err(ErrorData::invalid_params(
                Cow::Borrowed(
                    "The 'replace' tool requires the 'code' parameter. If you intend to replace across files, please use the 'file_replace' tool and provide a 'path_pattern'.",
                ),
                None,
            ));
        }
        if let Some(args) = &request.arguments {
            if args.contains_key("path_pattern") {
                return Err(ErrorData::invalid_params(
                    Cow::Borrowed(
                        "The 'replace' tool operates on code snippets and does not accept 'path_pattern'. If you intend to replace across files, please use the 'file_replace' tool.",
                    ),
                    None,
                ));
            }
        }

        let result = service.replace(param).await.map_err(ErrorData::from)?;
        let summary = ResponseFormatter::format_replace_result(&result);
        Self::create_formatted_response(&result, summary)
    }

    async fn handle_file_replace(
        service: &AstGrepService,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult, ErrorData> {
        let param: FileReplaceParam = Self::parse_params(&request)?;

        // Error handling for common LLM misuse patterns
        if param.path_pattern.is_empty() {
            return Err(ErrorData::invalid_params(
                Cow::Borrowed(
                    "The 'file_replace' tool requires the 'path_pattern' parameter. If you intend to replace a code snippet, please use the 'replace' tool and provide a 'code' parameter.",
                ),
                None,
            ));
        }
        if let Some(args) = &request.arguments {
            if args.contains_key("code") {
                return Err(ErrorData::invalid_params(
                    Cow::Borrowed(
                        "The 'file_replace' tool operates on files and does not accept a 'code' parameter. If you intend to replace a code snippet, please use the 'replace' tool.",
                    ),
                    None,
                ));
            }
            // Warn if dry_run is not explicitly set
            if !args.contains_key("dry_run") {
                return Err(ErrorData::invalid_params(
                    Cow::Borrowed(
                        "For 'file_replace', it is highly recommended to explicitly set 'dry_run' to true or false to confirm your intent. 'dry_run: true' will show changes without applying them, while 'dry_run: false' will apply changes directly to files.",
                    ),
                    None,
                ));
            }
        }

        let result = service.file_replace(param).await.map_err(ErrorData::from)?;
        let summary = ResponseFormatter::format_file_replace_result(&result);
        Self::create_formatted_response(&result, summary)
    }

    // Rule operations
    async fn handle_rule_search(
        service: &AstGrepService,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult, ErrorData> {
        let param: RuleSearchParam = Self::parse_params(&request)?;
        let result = service.rule_search(param).await.map_err(ErrorData::from)?;
        let summary = ResponseFormatter::format_file_search_result(&result);
        Self::create_formatted_response(&result, summary)
    }

    async fn handle_rule_replace(
        service: &AstGrepService,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult, ErrorData> {
        let param: RuleReplaceParam = Self::parse_params(&request)?;
        let result = service.rule_replace(param).await.map_err(ErrorData::from)?;
        let summary = ResponseFormatter::format_file_replace_result(&result);
        Self::create_formatted_response(&result, summary)
    }

    async fn handle_create_rule(
        service: &AstGrepService,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult, ErrorData> {
        let param: CreateRuleParam = Self::parse_params(&request)?;
        let result = service.create_rule(param).await.map_err(ErrorData::from)?;
        Self::create_json_response(result)
    }

    async fn handle_get_rule(
        service: &AstGrepService,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult, ErrorData> {
        let param: GetRuleParam = Self::parse_params(&request)?;
        let result = service.get_rule(param).await.map_err(ErrorData::from)?;
        Self::create_json_response(result)
    }

    async fn handle_list_rules(
        service: &AstGrepService,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult, ErrorData> {
        let param: ListRulesParam = Self::parse_params(&request)?;
        let result = service.list_rules(param).await.map_err(ErrorData::from)?;
        Self::create_json_response(result)
    }

    async fn handle_delete_rule(
        service: &AstGrepService,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult, ErrorData> {
        let param: DeleteRuleParam = Self::parse_params(&request)?;
        let result = service.delete_rule(param).await.map_err(ErrorData::from)?;
        Self::create_json_response(result)
    }

    async fn handle_rule_validate(
        service: &AstGrepService,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult, ErrorData> {
        let param: RuleValidateParam = Self::parse_params(&request)?;
        let result = service
            .validate_rule(param)
            .await
            .map_err(ErrorData::from)?;
        Self::create_json_response(result)
    }

    // Debug operations
    async fn handle_generate_ast(
        service: &AstGrepService,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult, ErrorData> {
        let param: GenerateAstParam = Self::parse_params(&request)?;
        let result = service.generate_ast(param).await.map_err(ErrorData::from)?;
        let summary = ResponseFormatter::format_generate_ast_result(&result);
        Self::create_formatted_response(&result, summary)
    }

    // Utility operations

    async fn handle_list_languages(
        service: &AstGrepService,
        _request: CallToolRequestParam,
    ) -> Result<CallToolResult, ErrorData> {
        let param = ListLanguagesParam {};
        let result = service
            .list_languages(param)
            .await
            .map_err(ErrorData::from)?;
        Self::create_json_response(result)
    }

    // Learning operations
    async fn handle_validate_pattern(
        service: &AstGrepService,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult, ErrorData> {
        let param: crate::learning::ValidatePatternParam = Self::parse_params(&request)?;
        let result = service
            .validate_pattern(param)
            .await
            .map_err(ErrorData::from)?;
        let summary = format!(
            "Pattern validation {}. Complexity: {:.2}, Compatible languages: {}",
            if result.is_valid { "passed" } else { "failed" },
            result.analysis.complexity_score,
            result.analysis.language_compatibility.join(", ")
        );
        Self::create_formatted_response(&result, summary)
    }

    async fn handle_explore_patterns(
        service: &AstGrepService,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult, ErrorData> {
        let param: crate::learning::ExplorePatternParam = Self::parse_params(&request)?;
        let result = service
            .explore_patterns(param)
            .await
            .map_err(ErrorData::from)?;
        let summary = format!(
            "Found {} patterns (of {} total available). Learning path has {} steps",
            result.patterns.len(),
            result.total_available,
            result.learning_path.len()
        );
        Self::create_formatted_response(&result, summary)
    }
}
